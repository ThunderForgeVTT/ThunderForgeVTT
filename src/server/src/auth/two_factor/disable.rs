//! Turning a second factor off, deliberately.
//!
//! Spec 041 US4 (FR-012, FR-014), `contracts/removal-and-reset.md`.
//!
//! # Why this route had to exist
//!
//! There was no way off. `setup/start`'s side effect had been doing the job —
//! beginning a new enrolment cleared the old factor — which meant the only
//! route out of two-factor ran through a route whose name says it turns it
//! *on*. An account that started an enrolment and abandoned it was left
//! without the factor it had before, and nobody asked it for anything to get
//! there.
//!
//! # The price is the price of adding one
//!
//! Password **and** possession. The session is not enough on its own: it
//! proves the password was held at sign-in, which may have been days ago on a
//! machine that has since changed hands, and this is the one action that makes
//! every future sign-in cheaper. Either a current authenticator code or an
//! unspent recovery code satisfies possession — the same two proofs
//! `regenerate_recovery_codes` accepts, through the same helper, because two
//! implementations of "prove you still hold it" is one more than this product
//! should have.
//!
//! # Why a refusal here names its reason
//!
//! Every other refusal on the verification path is deliberately vague, because
//! the caller might be an attacker and the shape of the failure is
//! information. Not this one: by the time it is reached the caller has proved
//! the password *and* possession of the factor. They are the account holder.
//! Telling them "an administrator requires this" is telling them something
//! they are entitled to know and can act on; telling them "no" would just make
//! them try again.

use axum::{Json, extract::State, http::StatusCode};
use diesel::prelude::*;
use tower_cookies::Cookies;

use super::recovery::{delete_recovery_codes_sync, verify_second_factor_proof};
use super::requirement::second_factor_required;
use crate::auth::types::TwoFactorDisableRequest;
use crate::auth_middleware::resolve_authenticated_user;
use crate::schema::users;
use crate::state::AppState;

/// Why removal is refused, when it is.
///
/// A closed set rather than a string, so the test can be exhaustive and the
/// handler cannot invent a fourth reason with different wording.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisableRefusal {
    /// FR-027. The role carries the requirement; giving up the role is the
    /// honest ordering.
    AdminRole,
    /// FR-019. Every account on this instance.
    InstancePolicy,
    /// FR-023. This account, by an administrator's decision.
    AdminRequired,
}

impl DisableRefusal {
    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::AdminRole => {
                "An administrator account must keep a second factor. Give up administrator \
                 access first, and then it can be turned off."
            }
            Self::InstancePolicy => {
                "This instance requires a second factor on every account, so it cannot be \
                 turned off here."
            }
            Self::AdminRequired => {
                "An administrator requires a second factor on this account, so it cannot be \
                 turned off here."
            }
        }
    }
}

/// Whether this account may turn its factor off, and why not.
///
/// Pure, and ordered: the role is checked before the policies because it is
/// the reason the caller can act on most directly. An administrator told "the
/// instance requires it" would go looking for a setting to change, when what
/// they actually have to do is stop being an administrator.
pub(crate) fn refusal_for(
    is_admin: bool,
    instance_required: bool,
    admin_required: bool,
) -> Option<DisableRefusal> {
    if is_admin {
        Some(DisableRefusal::AdminRole)
    } else if instance_required {
        Some(DisableRefusal::InstancePolicy)
    } else if second_factor_required(is_admin, instance_required, admin_required) {
        Some(DisableRefusal::AdminRequired)
    } else {
        None
    }
}

/// Clear the factor, its pending enrolment and its recovery codes, together.
///
/// One transaction, because a factor cleared while its codes survive is an
/// account that can still be "verified" by a credential nothing now protects,
/// and a pending enrolment left behind would let the next `setup/confirm`
/// re-arm a factor this request just paid to remove.
///
/// The `removed` event is written **inside** that transaction (FR-015): a
/// removal nobody recorded did not happen, and this is a caller that can
/// record without risking the action, because it is still holding the
/// transaction that performs it.
///
/// `event_type` distinguishes the two ways a factor goes: an account holder
/// turning their own off (`removed`), and an operator resetting somebody
/// else's (`reset_by_operator`). The clearing is identical and the *record* is
/// not, which is the whole reason `two_factor_events` carries two user
/// columns — collapsing them here would lose the distinction at the only
/// moment it exists.
pub(crate) fn clear_second_factor_sync(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    actor_user_id: Option<uuid::Uuid>,
    event_type: &str,
) -> Result<(), diesel::result::Error> {
    conn.transaction::<_, diesel::result::Error, _>(|conn| {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq(None::<String>),
                users::two_factor_confirmed_at.eq(None::<chrono::NaiveDateTime>),
                users::two_factor_pending_secret_encrypted.eq(None::<String>),
                users::two_factor_pending_started_at.eq(None::<chrono::NaiveDateTime>),
            ))
            .execute(conn)?;
        delete_recovery_codes_sync(conn, user_id)?;
        super::events::record_sync(conn, user_id, actor_user_id, event_type)?;

        // FR-017's counter goes with it. An account whose factor an operator
        // has just reset must not still be serving a cooling-off from the
        // guesses that led somebody to ask for the reset.
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_failed_attempts.eq(0),
                users::two_factor_locked_until.eq(None::<chrono::NaiveDateTime>),
            ))
            .execute(conn)?;
        Ok(())
    })
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct TwoFactorDisableResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
}

fn refuse(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<TwoFactorDisableResponse>) {
    (
        code,
        Json(TwoFactorDisableResponse {
            status,
            message: message.to_string(),
        }),
    )
}

/// `POST /authentication/2fa/disable`.
pub(crate) async fn two_factor_disable(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<TwoFactorDisableRequest>,
) -> (StatusCode, Json<TwoFactorDisableResponse>) {
    let Ok(authenticated) = resolve_authenticated_user(&state, &cookies).await else {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "failure",
            "Authentication required",
        );
    };
    let user_id = authenticated.user_id;

    // Read the account's own state first. Refusing early for a policy reason
    // costs the caller nothing and spends no recovery code — a person told
    // "your administrator requires this" should not also have burned one of
    // their ten codes finding out.
    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                "Failed to get DB connection",
            );
        }
    };
    let row = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select((
                users::password_hash,
                users::two_factor_enabled,
                users::two_factor_admin_required,
                users::is_admin,
            ))
            .first::<(String, bool, bool, bool)>(&mut conn)
    })
    .await;

    let (password_hash, enabled, admin_required, is_admin) = match row {
        Ok(Ok(row)) => row,
        _ => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                "That account could not be read",
            );
        }
    };

    if !enabled {
        // Idempotent rather than an error: the end state the caller asked for
        // is the state they are in.
        return (
            StatusCode::OK,
            Json(TwoFactorDisableResponse {
                status: "success",
                message: "Two-factor authentication is off for this account.".to_string(),
            }),
        );
    }

    // FR-019 / FR-023 / FR-027, and each names itself.
    let instance_required =
        match crate::auth::two_factor::load_global_two_factor_requirement(&state).await {
            Ok(value) => value,
            Err(_) => {
                return refuse(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "two_factor_error",
                    "The instance policy could not be read",
                );
            }
        };
    if let Some(refusal) = refusal_for(is_admin, instance_required, admin_required) {
        return refuse(
            StatusCode::FORBIDDEN,
            "two_factor_required",
            refusal.message(),
        );
    }

    // The password. Verified before possession so that a wrong password does
    // not spend a recovery code.
    if !thunderforge_axum_auth_core::hashing::verify(&request.password, &password_hash) {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "That password is not correct.",
        );
    }

    match verify_second_factor_proof(
        &state,
        user_id,
        request.code.as_deref(),
        request.recovery_code.as_deref(),
    )
    .await
    {
        Ok(true) => {}
        Ok(false) => {
            return refuse(
                StatusCode::UNAUTHORIZED,
                "two_factor_invalid",
                crate::auth::two_factor::verification::CREDENTIAL_REFUSED,
            );
        }
        Err(message) => {
            let bad_request = message.starts_with("Provide exactly one");
            return refuse(
                if bad_request {
                    StatusCode::BAD_REQUEST
                } else {
                    StatusCode::INTERNAL_SERVER_ERROR
                },
                if bad_request {
                    "two_factor_invalid"
                } else {
                    "two_factor_error"
                },
                &message,
            );
        }
    }

    // One transaction. A factor cleared while its recovery codes survive is an
    // account that can still be "verified" by a code nothing now protects.
    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(_) => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "two_factor_error",
                "Failed to get DB connection",
            );
        }
    };
    let cleared = tokio::task::spawn_blocking(move || {
        // `removed`, not `reset_by_operator`: this is the account holder
        // turning their own factor off, having paid password *and* possession
        // for it. The operator path is `operator_reset`, and the record has to
        // be able to tell them apart.
        clear_second_factor_sync(
            &mut conn,
            user_id,
            Some(user_id),
            super::events::event_type::REMOVED,
        )
    })
    .await;

    match cleared {
        Ok(Ok(())) => {
            // FR-015, after the commit and unable to fail this request: the
            // person who just weakened their own account is told, because the
            // case that matters is the one where it was not them.
            super::notify::tell(&state, user_id, super::notify::Change::Removed).await;
            (
                StatusCode::OK,
                Json(TwoFactorDisableResponse {
                    status: "success",
                    message: "Two-factor authentication is off for this account.".to_string(),
                }),
            )
        }
        _ => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "two_factor_error",
            "The second factor could not be turned off",
        ),
    }
}

#[cfg(test)]
#[path = "disable_tests.rs"]
mod tests;
