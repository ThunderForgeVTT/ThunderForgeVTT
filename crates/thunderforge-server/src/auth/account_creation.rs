//! The two places a user account row is written outside first-run setup:
//! the local signup form and OAuth auto-provisioning (ADR-042, ADR-072).
//!
//! Both claim the invitation use that admitted the account in the same
//! transaction as the account insert (FR-016, FR-018, SC-006). Admission was
//! judged earlier, read-only, by
//! [`super::registration::ensure_admission_allowed`]; nothing is burned until
//! an account exists, and an account never exists without the use that
//! admitted it. Owner's rule, 2026-10-09: bots, repeated clicks, refreshes,
//! and failed or abandoned signups never burn a use.

use super::*;
use crate::models::NewUserOAuthAccount;
use crate::schema::user_oauth_accounts;

/// Validates a registration and creates the account, or says why not.
///
/// When admission rested on an invitation, its use is claimed and the
/// redemption recorded in the same transaction as the account insert (FR-016,
/// FR-018, SC-006). A vtt-dev invitation once read 7 of 7 used with three
/// accounts behind it, because the use was taken before validation and only
/// some refusals gave it back; now nothing is taken until the account exists.
pub(super) async fn create_local_account(
    state: &AppState,
    request: &RegisterRequest,
    invitation_id: Option<uuid::Uuid>,
    route: &AdmissionRoute,
) -> Result<uuid::Uuid, Box<(StatusCode, Json<AuthSessionResponse>)>> {
    let username = request.username.trim().to_string();
    let email = request.email.trim().to_lowercase();

    validate_registration_input(&username, &email, &request.password).map_err(|message| {
        auth_session_error(StatusCode::BAD_REQUEST, "invalid_request", message.as_str())
    })?;

    let password_hash = hash_password(&request.password).map_err(|message| {
        auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "password_hash_failed",
            message.as_str(),
        )
    })?;

    let registration_failed = || {
        auth_session_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "registration_failed",
            "Failed to create account",
        )
    };

    let now = Utc::now().naive_utc();
    let route = route.clone();
    let mut conn = state.db_pool.get().map_err(|_| registration_failed())?;
    let create_result =
        tokio::task::spawn_blocking(move || -> Result<uuid::Uuid, RegisterUserError> {
            conn.transaction::<_, RegisterUserError, _>(|conn| {
                let username_exists = users::table
                    .filter(users::username.eq(&username))
                    .select(users::id)
                    .first::<uuid::Uuid>(conn)
                    .optional()
                    .map_err(|_| RegisterUserError::Storage)?;
                if username_exists.is_some() {
                    return Err(RegisterUserError::UsernameTaken);
                }

                let email_exists = users::table
                    .filter(users::email.eq(&email))
                    .select(users::id)
                    .first::<uuid::Uuid>(conn)
                    .optional()
                    .map_err(|_| RegisterUserError::Storage)?;
                if email_exists.is_some() {
                    return Err(RegisterUserError::EmailTaken);
                }

                let user_id = uuid::Uuid::now_v7();
                diesel::insert_into(users::table)
                    .values((
                        users::id.eq(user_id),
                        users::username.eq(username),
                        users::email.eq(email),
                        users::is_admin.eq(false),
                        users::password_hash.eq(password_hash),
                        users::created_at.eq(now),
                        users::updated_at.eq(now),
                        users::two_factor_enabled.eq(false),
                        users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                        users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                        users::two_factor_admin_required.eq(false),
                    ))
                    .execute(conn)
                    .map_err(|_| RegisterUserError::Storage)?;

                // The use is burned here, with the account, or not at all.
                if let Some(invitation_id) = invitation_id {
                    let redeemed = crate::auth::instance_access::redeem_for_account_sync(
                        conn,
                        invitation_id,
                        user_id,
                        &route,
                    )?;
                    if !redeemed {
                        // Returning an error rolls the account insert back.
                        return Err(RegisterUserError::InvitationUnusable);
                    }
                }

                Ok(user_id)
            })
        })
        .await
        .map_err(|_| registration_failed())?;

    let refusal = match create_result {
        Ok(value) => return Ok(value),
        Err(RegisterUserError::UsernameTaken) => auth_session_error(
            StatusCode::CONFLICT,
            "username_taken",
            "Username is already in use",
        ),
        Err(RegisterUserError::EmailTaken) => auth_session_error(
            StatusCode::CONFLICT,
            "email_taken",
            "Email is already in use",
        ),
        Err(RegisterUserError::InvitationUnusable) => {
            record_refusal(state, &AdmissionRoute::Local).await;
            auth_session_error(
                StatusCode::CONFLICT,
                "invitation_unusable",
                "This invitation is no longer valid",
            )
        }
        Err(RegisterUserError::Storage) => registration_failed(),
    };
    Err(Box::new(refusal))
}

/// Writes an auto-provisioned OAuth account and its identity link in one
/// transaction, claiming the invitation use with them when admission rested
/// on one.
///
/// Returns `Ok(false)` when the invitation was usable at the gate and is not
/// now (its last use went to a concurrent signup, or it was revoked or
/// expired in between). Everything is rolled back and the caller refuses as
/// for an exhausted invitation. Before this, the use was taken ahead of the
/// inserts and the two inserts ran without a transaction, so a failed link
/// insert left an account no one could sign in to.
pub(super) fn provision_oauth_account_sync(
    conn: &mut diesel::PgConnection,
    username: &str,
    email: &str,
    password_hash: String,
    link: &NewUserOAuthAccount,
    invitation: Option<(uuid::Uuid, &AdmissionRoute)>,
) -> Result<bool, diesel::result::Error> {
    let now = link.created_at;
    let outcome = conn.transaction::<(), diesel::result::Error, _>(|conn| {
        diesel::insert_into(users::table)
            .values((
                users::id.eq(link.user_id),
                users::username.eq(username),
                users::email.eq(email),
                users::is_admin.eq(false),
                users::password_hash.eq(password_hash),
                users::created_at.eq(now),
                users::updated_at.eq(now),
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq::<Option<String>>(None),
                users::two_factor_confirmed_at.eq::<Option<chrono::NaiveDateTime>>(None),
                users::two_factor_admin_required.eq(false),
            ))
            .execute(conn)?;

        diesel::insert_into(user_oauth_accounts::table)
            .values(link)
            .execute(conn)?;

        if let Some((invitation_id, route)) = invitation
            && !crate::auth::instance_access::redeem_for_account_sync(
                conn,
                invitation_id,
                link.user_id,
                route,
            )?
        {
            return Err(diesel::result::Error::RollbackTransaction);
        }
        Ok(())
    });
    match outcome {
        Ok(()) => Ok(true),
        Err(diesel::result::Error::RollbackTransaction) => Ok(false),
        Err(error) => Err(error),
    }
}
