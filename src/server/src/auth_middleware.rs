use crate::admin::user_role;
use crate::models::UserSession;
use crate::schema::{user_sessions, users};
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use chrono::Utc;
use diesel::prelude::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tower_cookies::cookie::SameSite;
use tower_cookies::{Cookie, Cookies};

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub is_admin: bool,
    pub role: String,
}

static AUTH_RATE_LIMITER: OnceLock<Mutex<HashMap<String, Vec<i64>>>> = OnceLock::new();

fn limiter_store() -> &'static Mutex<HashMap<String, Vec<i64>>> {
    AUTH_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

pub async fn rate_limit_auth_requests(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if !path.contains("/authentication/") {
        return next.run(request).await;
    }

    let ip = client_ip(request.headers());
    let key = format!("{ip}:{path}");

    let now = Utc::now().timestamp();
    let window_seconds = 60;
    let max_requests = if path.contains("/authentication/basic")
        || path.contains("/authentication/login")
        || path.contains("/authentication/register")
    {
        15
    } else {
        40
    };

    {
        let mut store = match limiter_store().lock() {
            Ok(v) => v,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
        let entry = store.entry(key).or_default();
        entry.retain(|ts| now - *ts < window_seconds);
        if entry.len() >= max_requests {
            return StatusCode::TOO_MANY_REQUESTS.into_response();
        }
        entry.push(now);
    }

    next.run(request).await
}

pub async fn require_csrf_for_session(
    State(state): State<AppState>,
    cookies: Cookies,
    request: Request,
    next: Next,
) -> Response {
    let has_session = cookies.private(&state.key).get("session").is_some();

    if has_session {
        ensure_csrf_cookie(&cookies);
    }

    let method = request.method().clone();
    let needs_csrf = method == Method::POST
        || method == Method::PUT
        || method == Method::PATCH
        || method == Method::DELETE;

    if has_session && needs_csrf {
        let csrf_cookie = cookies
            .get("csrf_token")
            .map(|c| c.value().to_string())
            .unwrap_or_default();
        let csrf_header = request
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();

        if csrf_cookie.is_empty() || !secure_equals(csrf_cookie.as_bytes(), csrf_header.as_bytes())
        {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

fn ensure_csrf_cookie(cookies: &Cookies) {
    if cookies.get("csrf_token").is_some() {
        return;
    }

    let token = uuid::Uuid::now_v7().to_string();
    let mut cookie = Cookie::new("csrf_token", token);
    cookie.set_path("/");
    cookie.set_http_only(false);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_secure(true);
    cookies.add(cookie);
}

fn client_ip(headers: &HeaderMap) -> String {
    if let Some(forwarded) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok())
        && let Some(first) = forwarded.split(',').next()
    {
        let ip = first.trim();
        if !ip.is_empty() {
            return ip.to_string();
        }
    }

    headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn secure_equals(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    let max_len = std::cmp::max(a.len(), b.len());
    for i in 0..max_len {
        let av = *a.get(i).unwrap_or(&0);
        let bv = *b.get(i).unwrap_or(&0);
        diff |= (av ^ bv) as usize;
    }
    diff == 0
}

pub async fn require_authenticated_user(
    State(state): State<AppState>,
    cookies: Cookies,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let authenticated_user = resolve_authenticated_user(&state, &cookies).await?;

    request.extensions_mut().insert(authenticated_user);

    Ok(next.run(request).await)
}

pub async fn require_admin_user(
    State(state): State<AppState>,
    cookies: Cookies,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let authenticated_user = resolve_authenticated_user(&state, &cookies).await?;
    if !authenticated_user.is_admin {
        return Err(StatusCode::FORBIDDEN);
    }

    request.extensions_mut().insert(authenticated_user);
    Ok(next.run(request).await)
}

pub async fn resolve_authenticated_user(
    state: &AppState,
    cookies: &Cookies,
) -> Result<AuthenticatedUser, StatusCode> {
    let Some(session_cookie) = cookies.private(&state.key).get("session") else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let session_id = match uuid::Uuid::parse_str(session_cookie.value()) {
        Ok(v) => v,
        Err(_) => {
            cookies
                .private(&state.key)
                .remove(Cookie::new("session", ""));
            return Err(StatusCode::UNAUTHORIZED);
        }
    };
    let now = Utc::now().naive_utc();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session = tokio::task::spawn_blocking(move || {
        user_sessions::table
            .inner_join(users::table.on(users::id.eq(user_sessions::user_id)))
            .filter(user_sessions::id.eq(session_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .select((UserSession::as_select(), users::is_admin))
            .first::<(UserSession, bool)>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some((session, is_admin)) = session else {
        cookies
            .private(&state.key)
            .remove(Cookie::new("session", ""));
        return Err(StatusCode::UNAUTHORIZED);
    };

    Ok(AuthenticatedUser {
        user_id: session.user_id,
        session_id: session.id,
        expires_at: session.expires_at,
        is_admin,
        role: user_role(is_admin).to_string(),
    })
}
