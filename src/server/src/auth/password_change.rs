//! Changing the password on an account that has one.
//!
//! # Why this did not exist
//!
//! It simply was never built. `password_hash` was written at registration, at
//! the admin bootstrap and by OAuth auto-provisioning, and never again — so a
//! person who thought their password had been seen had exactly one remedy,
//! which was to stop using the account. Spec 036 FR-008 ("a password change
//! MUST end every session for that account except the one that made the
//! change") had nothing to attach to, and the `password_changed` value in the
//! `ended_reason` constraint was written by nothing.
//!
//! # What it costs
//!
//! The current password. Not the second factor: unlike turning a factor
//! *off*, changing a password does not make any future sign-in cheaper, and
//! demanding possession here would mean somebody who has lost their
//! authenticator cannot rotate a password they believe is compromised —
//! trading the urgent risk for the smaller one.
//!
//! # Why the sessions go
//!
//! FR-008, and the reason behind it: the usual cause of changing a password
//! is believing somebody else has it, and somebody else who has it is signed
//! in. Leaving those sessions alive would make the change cosmetic. The
//! session that asked survives, because signing the person out of the screen
//! they just used would read as a failure.

use axum::{Json, extract::State, http::StatusCode};
use diesel::prelude::*;
use tower_cookies::Cookies;

use crate::auth::sessions::{ended_reason, hash_password};
use crate::auth_middleware::resolve_authenticated_user;
use crate::schema::{user_sessions, users};
use crate::state::AppState;

#[derive(Debug, serde::Deserialize)]
pub(crate) struct PasswordChangeRequest {
    pub(crate) current_password: String,
    pub(crate) new_password: String,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct PasswordChangeResponse {
    pub(crate) status: &'static str,
    pub(crate) message: String,
    /// How many other sessions this ended, so the screen can say so rather
    /// than claim it in the abstract.
    pub(crate) sessions_ended: i64,
}

fn refuse(
    code: StatusCode,
    status: &'static str,
    message: &str,
) -> (StatusCode, Json<PasswordChangeResponse>) {
    (
        code,
        Json(PasswordChangeResponse {
            status,
            message: message.to_string(),
            sessions_ended: 0,
        }),
    )
}

/// Whether a proposed new password may replace the current one.
///
/// Pure, so the rule can be asserted without a database. Length is the only
/// composition rule this product has — see
/// `thunderforge_axum_auth_core::password` for why — and "not the one you
/// already have" is the only addition, because a change that changes nothing
/// ends every other session for no gain and reads as success.
pub(crate) fn validate_new_password(current: &str, proposed: &str) -> Result<(), String> {
    if proposed.len() < thunderforge_axum_auth_core::password::PASSWORD_MIN_LEN {
        return Err(format!(
            "Password must be at least {} characters long",
            thunderforge_axum_auth_core::password::PASSWORD_MIN_LEN
        ));
    }
    if proposed == current {
        return Err("That is the password you already have.".to_string());
    }
    Ok(())
}

/// Write the new hash and end every other session, together.
///
/// One transaction. A hash written whose sessions were not ended is the
/// cosmetic change this route exists to avoid, and sessions ended whose hash
/// was not written would sign somebody out for nothing.
fn apply_change_sync(
    conn: &mut PgConnection,
    user_id: uuid::Uuid,
    keep_session: uuid::Uuid,
    new_hash: String,
) -> Result<usize, diesel::result::Error> {
    let now = chrono::Utc::now().naive_utc();
    conn.transaction(|conn| {
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::password_hash.eq(new_hash))
            .execute(conn)?;

        diesel::update(
            user_sessions::table
                .filter(user_sessions::user_id.eq(user_id))
                .filter(user_sessions::id.ne(keep_session))
                .filter(user_sessions::revoked_at.is_null()),
        )
        .set((
            user_sessions::revoked_at.eq(Some(now)),
            user_sessions::ended_reason.eq(Some(ended_reason::PASSWORD_CHANGED)),
        ))
        .execute(conn)
    })
}

/// `POST /authentication/password`.
pub(crate) async fn change_password(
    cookies: Cookies,
    State(state): State<AppState>,
    Json(request): Json<PasswordChangeRequest>,
) -> (StatusCode, Json<PasswordChangeResponse>) {
    let Ok(authenticated) = resolve_authenticated_user(&state, &cookies).await else {
        return refuse(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Authentication required",
        );
    };
    let user_id = authenticated.user_id;
    let session_id = authenticated.session_id;

    if let Err(message) = validate_new_password(&request.current_password, &request.new_password) {
        return refuse(StatusCode::BAD_REQUEST, "invalid_password", &message);
    }

    let Ok(mut conn) = state.db_pool.get() else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_error",
            "Failed to get DB connection",
        );
    };

    let stored = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::id.eq(user_id))
            .select(users::password_hash)
            .first::<String>(&mut conn)
    })
    .await;
    let Ok(Ok(stored)) = stored else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_error",
            "That account could not be read",
        );
    };

    if !thunderforge_axum_auth_core::hashing::verify(&request.current_password, &stored) {
        // Deliberately the same shape as any other wrong-password answer: the
        // caller holds a session, so this is not an account-existence oracle,
        // but there is no reason to distinguish "wrong" from anything else.
        return refuse(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "That is not your current password.",
        );
    }

    let Ok(new_hash) = hash_password(&request.new_password) else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_error",
            "The new password could not be stored",
        );
    };

    let Ok(mut conn) = state.db_pool.get() else {
        return refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_error",
            "Failed to get DB connection",
        );
    };
    let applied = tokio::task::spawn_blocking(move || {
        apply_change_sync(&mut conn, user_id, session_id, new_hash)
    })
    .await;

    let ended = match applied {
        Ok(Ok(count)) => count,
        _ => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                "password_error",
                "The password could not be changed",
            );
        }
    };

    (
        StatusCode::OK,
        Json(PasswordChangeResponse {
            status: "success",
            message: if ended == 0 {
                "Your password has been changed.".to_string()
            } else {
                format!(
                    "Your password has been changed, and {ended} other session{} \
                     signed out.",
                    if ended == 1 { " was" } else { "s were" }
                )
            },
            sessions_ended: i64::try_from(ended).unwrap_or(i64::MAX),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::ClientDescription;
    use crate::auth::session_registry::live_sessions;
    use crate::auth::sessions::issue_session_cookie;
    use crate::test_support::{insert_test_user, test_app_state};
    use tower_cookies::Cookies;

    const CURRENT: &str = "correct horse battery staple";

    #[test]
    fn a_short_password_is_refused_by_the_rule_the_product_already_has() {
        assert!(validate_new_password(CURRENT, "short").is_err());
        assert!(validate_new_password(CURRENT, &"x".repeat(12)).is_ok());
    }

    /// A "change" to the same password would end every other session and
    /// leave the account exactly as compromised as it was — success in
    /// wording only.
    #[test]
    fn the_same_password_is_not_a_change() {
        assert_eq!(
            validate_new_password(CURRENT, CURRENT),
            Err("That is the password you already have.".to_string()),
        );
    }

    /// FR-008. The session that asked survives; every other one ends, and
    /// ends with a reason that says why.
    #[tokio::test]
    async fn changing_the_password_ends_every_other_session() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        let user_id = insert_test_user(&mut conn);
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::password_hash.eq(hash_password(CURRENT).expect("hashed")))
            .execute(&mut conn)
            .expect("password set");
        drop(conn);

        let mut ids = Vec::new();
        for _ in 0..3 {
            ids.push(
                issue_session_cookie(
                    &state,
                    &Cookies::default(),
                    user_id,
                    ClientDescription::unknown(),
                )
                .await
                .expect("a session")
                .id,
            );
        }
        let asking = ids[1];

        let mut conn = state.db_pool.get().expect("conn");
        let ended = apply_change_sync(
            &mut conn,
            user_id,
            asking,
            hash_password("a different long password").expect("hashed"),
        )
        .expect("the change applies");
        drop(conn);

        assert_eq!(ended, 2, "the two sessions that did not ask were ended");

        let remaining: Vec<uuid::Uuid> = live_sessions(&state, user_id, asking)
            .await
            .expect("listed")
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(
            remaining,
            vec![asking],
            "only the session that made the change is still live",
        );

        // The reason is the point: `ended_reason` is how somebody reading the
        // row later can tell a password change from a sign-out, and this
        // value had never been written by anything.
        let mut conn = state.db_pool.get().expect("conn");
        let reasons: Vec<Option<String>> = user_sessions::table
            .filter(user_sessions::user_id.eq(user_id))
            .filter(user_sessions::id.ne(asking))
            .select(user_sessions::ended_reason)
            .load(&mut conn)
            .expect("read reasons");
        assert_eq!(
            reasons,
            vec![
                Some(ended_reason::PASSWORD_CHANGED.to_string()),
                Some(ended_reason::PASSWORD_CHANGED.to_string()),
            ],
        );
    }

    /// The transaction's other half: the new password is the one stored, and
    /// the old one no longer verifies.
    #[tokio::test]
    async fn the_new_password_replaces_the_old_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("conn");
        let user_id = insert_test_user(&mut conn);
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::password_hash.eq(hash_password(CURRENT).expect("hashed")))
            .execute(&mut conn)
            .expect("password set");

        apply_change_sync(
            &mut conn,
            user_id,
            uuid::Uuid::now_v7(),
            hash_password("a different long password").expect("hashed"),
        )
        .expect("the change applies");

        let stored: String = users::table
            .filter(users::id.eq(user_id))
            .select(users::password_hash)
            .first(&mut conn)
            .expect("read back");

        assert!(thunderforge_axum_auth_core::hashing::verify(
            "a different long password",
            &stored
        ));
        assert!(
            !thunderforge_axum_auth_core::hashing::verify(CURRENT, &stored),
            "the old password must stop working — this is the whole point",
        );
    }
}
