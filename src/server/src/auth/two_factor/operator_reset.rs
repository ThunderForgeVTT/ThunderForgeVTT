//! Spec 041 FR-024/FR-025: an operator resets somebody's second factor.
//!
//! # The case this is for
//!
//! Everything else in this feature serves the person who still holds
//! *something*: the authenticator, or one of the ten recovery codes. This is
//! for the person who holds neither — the phone is gone and the codes went with
//! the laptop — and without it that account is simply lost.
//!
//! FR-024 says the path must be a defined one, "without editing the database".
//! Until this route there was no path at all, which meant the only way to help
//! somebody was the database edit FR-024 forbids: unaudited, unnotified, and
//! performed with a text editor against a live table by somebody working from
//! memory at the worst possible moment.
//!
//! # Why it is not `disable` with an admin check
//!
//! Because they are different acts and the record must say which. Removal is
//! an account holder deciding about their own account, and costs them password
//! **and** possession. A reset is an operator deciding about somebody else's,
//! and possession is precisely what is missing — demanding it would make the
//! route useless for the only situation it exists for. What replaces it is
//! that an operator is doing it, in the open, with their name on the row.
//!
//! # What it deliberately does not do
//!
//! It does not enrol a new factor, and it does not hand out fresh recovery
//! codes. It puts the account back to having no second factor, which is a
//! state the product already understands; the person then enrols from their own
//! screen like anybody else, with a secret only they ever see. An operator who
//! could *issue* a factor could sign in as the account holder.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use tower_cookies::Cookies;

use super::disable::clear_second_factor_sync;
use super::events;
use crate::auth::admin_setup::verify_admin_request;
use crate::auth::types::OAuthResponse;
use crate::auth_middleware::resolve_authenticated_user;
use crate::state::AppState;

fn refuse(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<OAuthResponse>) {
    (
        code,
        Json(OAuthResponse {
            status,
            message: message.to_string(),
            challenge_id: None,
            login_two_factor_challenge_id: None,
        }),
    )
}

/// `POST /authentication/admin/users/{user_id}/2fa/reset`.
pub(crate) async fn reset_second_factor(
    cookies: Cookies,
    Path(user_id): Path<uuid::Uuid>,
    State(state): State<AppState>,
) -> (StatusCode, Json<OAuthResponse>) {
    if let Err(resp) = verify_admin_request(&state, &cookies).await {
        return resp;
    }

    // FR-025 wants *who*, and a reset with no actor recorded is the audit hole
    // this route exists to close. `verify_admin_request` proves the caller is
    // an administrator without saying which one, so the identity is resolved
    // separately rather than assumed.
    let actor_user_id = resolve_authenticated_user(&state, &cookies)
        .await
        .ok()
        .map(|user| user.user_id);

    let Ok(mut conn) = state.db_pool.get() else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            "Failed to get DB connection",
        );
    };

    let cleared = tokio::task::spawn_blocking(move || {
        clear_second_factor_sync(
            &mut conn,
            user_id,
            actor_user_id,
            events::event_type::RESET_BY_OPERATOR,
        )
    })
    .await;

    match cleared {
        Ok(Ok(())) => {
            // FR-015/FR-025, and the most important of the three notices: the
            // account holder did not do this and may not know it happened.
            // After the commit, and it cannot fail the reset — an instance
            // with no mail configured is the ordinary case for exactly the
            // small self-hosted instances that need this route most.
            super::notify::tell(&state, user_id, super::notify::Change::ResetByOperator).await;
            (
                StatusCode::OK,
                Json(OAuthResponse {
                    status: "success",
                    // Says what happened *and* what has to happen next: the account
                    // can now sign in with a password alone, which is a weaker
                    // state than it was in a moment ago, and whoever asked for this
                    // should be told to go and enrol again rather than left to
                    // discover it.
                    message: "Second factor reset. That account can sign in with \
                          its password, and should enrol a new factor."
                        .to_string(),
                    challenge_id: None,
                    login_two_factor_challenge_id: None,
                }),
            )
        }
        _ => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            "The second factor could not be reset",
        ),
    }
}

#[cfg(test)]
#[path = "operator_reset_tests.rs"]
mod operator_reset_tests;
