use crate::models::UserSession;
use crate::schema::user_sessions;
use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::Response;
use chrono::Utc;
use diesel::prelude::*;
use tower_cookies::Cookies;

#[derive(Clone, Debug)]
pub struct AuthenticatedUser {
    pub user_id: uuid::Uuid,
    pub session_id: uuid::Uuid,
}

pub async fn require_authenticated_user(
    State(state): State<AppState>,
    cookies: Cookies,
    mut request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let Some(session_cookie) = cookies.private(&state.key).get("session") else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    let session_id =
        uuid::Uuid::parse_str(session_cookie.value()).map_err(|_| StatusCode::UNAUTHORIZED)?;
    let now = Utc::now().naive_utc();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let session = tokio::task::spawn_blocking(move || {
        user_sessions::table
            .filter(user_sessions::id.eq(session_id))
            .filter(user_sessions::revoked_at.is_null())
            .filter(user_sessions::expires_at.gt(now))
            .select(UserSession::as_select())
            .first::<UserSession>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let Some(session) = session else {
        return Err(StatusCode::UNAUTHORIZED);
    };

    request.extensions_mut().insert(AuthenticatedUser {
        user_id: session.user_id,
        session_id: session.id,
    });

    Ok(next.run(request).await)
}
