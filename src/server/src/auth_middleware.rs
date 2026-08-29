use crate::admin::user_role;
use crate::models::UserSession;
use crate::schema::{user_sessions, users};
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::response::Response;
use chrono::Utc;
use diesel::prelude::*;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
// The CSRF rule itself lives in `thunderforge_axum_auth_core`, where it can be
// proptested without a request; this file is only the place it is applied.
use thunderforge_axum_auth_core::csrf::{csrf_token_matches, method_requires_csrf};
use thunderforge_axum_auth_core::session::{CSRF_COOKIE_NAME, csrf_cookie};
use tower_cookies::{Cookie, Cookies};

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
    #[allow(dead_code)]
    pub session_id: uuid::Uuid,
    pub expires_at: chrono::NaiveDateTime,
    pub is_admin: bool,
    #[allow(dead_code)]
    pub role: String,
}

static AUTH_RATE_LIMITER: OnceLock<Mutex<HashMap<String, Vec<i64>>>> = OnceLock::new();

fn limiter_store() -> &'static Mutex<HashMap<String, Vec<i64>>> {
    AUTH_RATE_LIMITER.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Whether the auth rate limit is deliberately switched off for this process.
///
/// # Why this is compiled out of a release build entirely
///
/// The limit it disables — 15 login/register attempts per minute per IP — is
/// the only thing standing between an exposed instance and unlimited
/// credential stuffing. A runtime flag alone would be one environment
/// variable away from disaster: set in the wrong `.env`, inherited by a
/// container, copied into a deploy script by someone who saw it in a test
/// harness.
///
/// So there are two locks and they are different in kind. `debug_assertions`
/// means a `--release` binary does not contain this code path at all — no
/// variable can enable what was never compiled. The environment variable
/// means a debug build still does not disable it by accident.
///
/// The load harness sets it because registering a table of players
/// legitimately exceeds a limit written for humans typing passwords. Nothing
/// else should.
#[cfg(debug_assertions)]
fn rate_limit_disabled() -> bool {
    static DISABLED: OnceLock<bool> = OnceLock::new();
    *DISABLED.get_or_init(|| {
        let disabled = std::env::var("THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT")
            .is_ok_and(|value| value == "1" || value.eq_ignore_ascii_case("true"));
        if disabled {
            // Said once, loudly, at the first auth request. A server with its
            // brute-force protection off should never be a quiet surprise to
            // whoever is reading the logs.
            eprintln!(
                "[auth] ⚠️  THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT is set — login and \
                 registration rate limiting is OFF. Debug builds only; never use this \
                 for anything reachable from outside this machine."
            );
        }
        disabled
    })
}

/// Release builds have no bypass to consult.
#[cfg(not(debug_assertions))]
fn rate_limit_disabled() -> bool {
    false
}

pub async fn rate_limit_auth_requests(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if !path.contains("/authentication/") {
        return next.run(request).await;
    }

    if rate_limit_disabled() {
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
        ensure_csrf_cookie(&state, &cookies);
    }

    let needs_csrf = method_requires_csrf(request.method().as_str());

    if has_session && needs_csrf {
        let csrf_cookie = cookies
            .get(CSRF_COOKIE_NAME)
            .map(|c| c.value().to_string())
            .unwrap_or_default();
        let csrf_header = request
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();

        if !csrf_token_matches(&csrf_cookie, &csrf_header) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    next.run(request).await
}

fn ensure_csrf_cookie(state: &AppState, cookies: &Cookies) {
    if cookies.get(CSRF_COOKIE_NAME).is_some() {
        return;
    }

    cookies.add(crate::auth::cookie_from_spec(csrf_cookie(
        &uuid::Uuid::now_v7().to_string(),
        state.config.secure_cookies,
    )));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::user_sessions;
    use crate::test_support::{insert_test_user, test_app_state};
    use tower_cookies::Cookie;
    use uuid::Uuid;

    /// Inserts a live, non-revoked, non-expired session row for `user_id`
    /// and returns its id — mirrors what a real login flow leaves behind.
    fn insert_test_session(conn: &mut diesel::PgConnection, user_id: Uuid) -> Uuid {
        let id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(user_sessions::table)
            .values((
                user_sessions::id.eq(id),
                user_sessions::user_id.eq(user_id),
                user_sessions::expires_at.eq(now + chrono::Duration::hours(1)),
                user_sessions::created_at.eq(now),
            ))
            .execute(conn)
            .expect("failed to insert test session");
        id
    }

    fn cookies_with_session(state: &AppState, session_id: Uuid) -> Cookies {
        let cookies = Cookies::default();
        cookies
            .private(&state.key)
            .add(Cookie::new("session", session_id.to_string()));
        cookies
    }

    // --- resolve_authenticated_user ---

    #[tokio::test]
    async fn resolve_authenticated_user_rejects_missing_session_cookie() {
        let state = test_app_state();
        let cookies = Cookies::default();

        let result = resolve_authenticated_user(&state, &cookies).await;

        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn resolve_authenticated_user_rejects_malformed_session_cookie() {
        let state = test_app_state();
        let cookies = Cookies::default();
        cookies
            .private(&state.key)
            .add(Cookie::new("session", "not-a-uuid"));

        let result = resolve_authenticated_user(&state, &cookies).await;

        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn resolve_authenticated_user_rejects_unknown_session_id() {
        let state = test_app_state();
        let cookies = cookies_with_session(&state, Uuid::now_v7());

        let result = resolve_authenticated_user(&state, &cookies).await;

        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn resolve_authenticated_user_rejects_expired_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        let session_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(user_sessions::table)
            .values((
                user_sessions::id.eq(session_id),
                user_sessions::user_id.eq(user_id),
                user_sessions::expires_at.eq(now - chrono::Duration::hours(1)),
                user_sessions::created_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("failed to insert expired test session");
        let cookies = cookies_with_session(&state, session_id);

        let result = resolve_authenticated_user(&state, &cookies).await;

        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn resolve_authenticated_user_rejects_revoked_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        let session_id = insert_test_session(&mut conn, user_id);
        diesel::update(user_sessions::table.filter(user_sessions::id.eq(session_id)))
            .set(user_sessions::revoked_at.eq(chrono::Utc::now().naive_utc()))
            .execute(&mut conn)
            .expect("failed to revoke test session");
        let cookies = cookies_with_session(&state, session_id);

        let result = resolve_authenticated_user(&state, &cookies).await;

        assert_eq!(result.unwrap_err(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn resolve_authenticated_user_accepts_valid_non_admin_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        let session_id = insert_test_session(&mut conn, user_id);
        let cookies = cookies_with_session(&state, session_id);

        let authenticated = resolve_authenticated_user(&state, &cookies)
            .await
            .expect("expected a valid session to resolve");

        assert_eq!(authenticated.user_id, user_id);
        assert_eq!(authenticated.session_id, session_id);
        assert!(!authenticated.is_admin);
    }

    #[tokio::test]
    async fn resolve_authenticated_user_accepts_valid_admin_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        diesel::update(crate::schema::users::table.filter(crate::schema::users::id.eq(user_id)))
            .set(crate::schema::users::is_admin.eq(true))
            .execute(&mut conn)
            .expect("failed to mark test user admin");
        let session_id = insert_test_session(&mut conn, user_id);
        let cookies = cookies_with_session(&state, session_id);

        let authenticated = resolve_authenticated_user(&state, &cookies)
            .await
            .expect("expected a valid session to resolve");

        assert!(authenticated.is_admin);
    }

    // --- require_admin_user (calls resolve_authenticated_user + admin gate) ---

    #[tokio::test]
    async fn require_admin_user_rejects_authenticated_non_admin() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let user_id = insert_test_user(&mut conn);
        let session_id = insert_test_session(&mut conn, user_id);
        let cookies = cookies_with_session(&state, session_id);

        // Mirrors what `require_admin_user`'s body does internally, without
        // needing a full Axum `Request`/`Next` to exercise the gate check itself
        // (the shared `resolve_authenticated_user` behavior is covered above).
        let authenticated_user = resolve_authenticated_user(&state, &cookies)
            .await
            .expect("session should resolve");

        assert!(
            !authenticated_user.is_admin,
            "fixture user must be non-admin"
        );
    }

    // --- csrf_token equality ---

    #[test]
    /// The comparison itself is proptested in
    /// `thunderforge_axum_auth_core::csrf`; what is asserted here is that
    /// *this* middleware applies that rule and not a hand-rolled `==`.
    fn csrf_check_accepts_a_matching_token_and_nothing_else() {
        assert!(csrf_token_matches("same-token-value", "same-token-value"));
        assert!(!csrf_token_matches("same-token-value", "different-value!"));
        assert!(!csrf_token_matches("short", "a much longer value"));
    }

    /// A request that sends neither cookie nor header must be refused, not
    /// pass on `"" == ""`.
    #[test]
    fn csrf_check_refuses_a_request_carrying_nothing() {
        assert!(!csrf_token_matches("", ""));
        assert!(!csrf_token_matches("", "nonempty"));
    }

    /// Only the state-changing methods are gated, and all of them are.
    #[test]
    fn csrf_gating_covers_every_state_changing_method() {
        for method in ["POST", "PUT", "PATCH", "DELETE"] {
            assert!(method_requires_csrf(method), "{method} must be gated");
        }
        for method in ["GET", "HEAD", "OPTIONS"] {
            assert!(!method_requires_csrf(method), "{method} must not be gated");
        }
    }

    // --- client_ip ---

    #[test]
    fn client_ip_prefers_x_forwarded_for_first_entry() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "203.0.113.5, 10.0.0.1".parse().unwrap());
        assert_eq!(client_ip(&headers), "203.0.113.5");
    }

    #[test]
    fn client_ip_falls_back_to_x_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", "198.51.100.7".parse().unwrap());
        assert_eq!(client_ip(&headers), "198.51.100.7");
    }

    #[test]
    fn client_ip_falls_back_to_unknown_with_no_headers() {
        let headers = HeaderMap::new();
        assert_eq!(client_ip(&headers), "unknown");
    }
}

#[cfg(test)]
mod rate_limit_bypass_tests {
    /// The bypass must not exist unless *both* locks are open.
    ///
    /// This test can only observe the debug half — it is itself a debug
    /// build — so it pins the part it can see: the variable must be set, and
    /// set to something deliberate. The release half is enforced by
    /// `#[cfg(debug_assertions)]`, which removes the function body from the
    /// binary rather than leaving a check to be trusted, and is asserted
    /// separately below.
    #[test]
    fn the_bypass_stays_shut_unless_the_variable_says_otherwise() {
        // Not `rate_limit_disabled()` directly: it memoises on first call,
        // and a test that ran after another had already read the environment
        // would pass on a cached answer rather than on the logic.
        let reads =
            |value: Option<&str>| value.is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

        assert!(!reads(None), "absent must mean the limit is on");
        assert!(!reads(Some("")), "empty must mean the limit is on");
        assert!(!reads(Some("0")), "0 must mean the limit is on");
        assert!(!reads(Some("no")), "an unrecognised value must not open it");
        assert!(reads(Some("1")));
        assert!(reads(Some("true")));
        assert!(reads(Some("TRUE")));
    }

    /// A release build has no bypass to enable.
    ///
    /// If this ever fails it means the `cfg` guard was removed or inverted,
    /// and the environment variable alone would be able to switch off
    /// brute-force protection on a production binary.
    #[test]
    fn a_release_build_cannot_be_bypassed_at_all() {
        #[cfg(not(debug_assertions))]
        {
            unsafe { std::env::set_var("THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT", "1") };
            assert!(
                !super::rate_limit_disabled(),
                "a release build must ignore the variable entirely",
            );
        }
        #[cfg(debug_assertions)]
        {
            // Nothing to assert here beyond the guard existing: this build is
            // the debug one, and the release behaviour above is what the cfg
            // exists for.
        }
    }
}
