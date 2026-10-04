//! What an operator may see and do about second factors (spec 041 FR-021,
//! FR-023, FR-024).
//!
//! Lifted out of `admin.rs` when that file crossed the 1000-line gate. The cut
//! is by subject rather than by convenience: everything here answers "how much
//! of this instance holds a second factor, and which one account am I about to
//! act on", which is a different question from OAuth providers, disk usage or
//! the realm manifest.
//!
//! Two rules carry across all of it and are worth stating once:
//!
//! - **Counts, never a roster.** A list of accounts without a second factor is
//!   a list of accounts a stolen password is sufficient for, and it would be
//!   handed to whoever takes over an operator's session.
//! - **Nothing an operator could sign in with.** No password hash, no secret,
//!   no recovery code, no session. That is the property that makes an operator
//!   surface for other people's second factors safe to have at all.

use diesel::prelude::*;

use crate::state::AppState;

use super::load_auth_security_settings;

/// Spec 041 FR-021: three counts, computed in one pass.
///
/// # Why `required_not_enrolled` is computed here rather than counted
///
/// It is `required(user) AND NOT enrolled`, and `required(user)` includes
/// `is_admin` — a term that exists nowhere as a column (ADR-094). So this is a
/// query over the three inputs rather than a count of a flag, which is the
/// same reason the rule itself is a function: there is no stored answer to
/// count, and an instance that upgrades into the administrator rule needs no
/// backfill for this figure to be right.
///
/// The instance-wide switch is read once and applied to every row, because it
/// is a property of the instance rather than of an account.
pub struct TwoFactorCoverage {
    pub enrolled: i64,
    pub not_enrolled: i64,
    pub required_not_enrolled: i64,
}

/// One account, as an operator needs to see it to act on its second factor.
///
/// Spec 041 US6/US7. Deliberately **not** a roster and deliberately not a
/// search: an exact username or email, or nothing. The same reasoning that
/// makes `two_factor_coverage` three integers applies here — a browsable list
/// of who has no second factor is a target list, and an operator with a
/// genuine reason to act on one account already knows which account it is,
/// because somebody has just asked them for help.
///
/// It carries no password hash, no secret, no recovery code and no session.
/// There is nothing here an operator could sign in with, which is the property
/// that makes an operator surface for other people's second factors safe to
/// have at all.
pub struct AdminAccountView {
    pub id: uuid::Uuid,
    pub username: String,
    pub email: String,
    pub is_admin: bool,
    pub two_factor_enabled: bool,
    pub two_factor_confirmed_at: Option<chrono::NaiveDateTime>,
    pub two_factor_admin_required: bool,
}

pub async fn find_account_for_admin(
    state: &AppState,
    identifier: &str,
) -> Result<Option<AdminAccountView>, String> {
    use crate::schema::users;

    let identifier = identifier.trim().to_string();
    if identifier.is_empty() {
        return Ok(None);
    }
    // Email is matched case-insensitively because that is how people type one;
    // a username is matched exactly because it is an identifier this product
    // chose, not one a mail provider folded.
    let email_candidate = identifier.to_lowercase();

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        users::table
            .filter(
                users::username
                    .eq(&identifier)
                    .or(users::email.eq(&email_candidate)),
            )
            .select((
                users::id,
                users::username,
                users::email,
                users::is_admin,
                users::two_factor_enabled,
                users::two_factor_confirmed_at,
                users::two_factor_admin_required,
            ))
            .first::<(
                uuid::Uuid,
                String,
                String,
                bool,
                bool,
                Option<chrono::NaiveDateTime>,
                bool,
            )>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to look up that account".to_string())
    .map(|row| {
        row.map(
            |(
                id,
                username,
                email,
                is_admin,
                two_factor_enabled,
                two_factor_confirmed_at,
                two_factor_admin_required,
            )| AdminAccountView {
                id,
                username,
                email,
                is_admin,
                two_factor_enabled,
                two_factor_confirmed_at,
                two_factor_admin_required,
            },
        )
    })
}

pub async fn load_two_factor_coverage(state: &AppState) -> Result<TwoFactorCoverage, String> {
    let instance_required = load_auth_security_settings(state)
        .await?
        .two_factor_required_for_all_users;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::users;

        // One snapshot, not three. Under the default read-committed level each
        // statement sees a different moment, so an account created between the
        // enrolled count and the total makes `not_enrolled = total - enrolled`
        // arrive one too high — and an account *deleted* between them makes it
        // negative. Neither is a big number on a real instance, and both are a
        // panel that contradicts itself for no reason a reader could work out.
        // Repeatable-read costs nothing here: three counts, read-only, no
        // contention to serialise against.
        conn.build_transaction()
            .repeatable_read()
            .read_only()
            .run(|conn| {
                let enrolled: i64 = users::table
                    .filter(users::two_factor_enabled.eq(true))
                    .count()
                    .get_result(conn)?;
                let total: i64 = users::table.count().get_result(conn)?;

                // Not enrolled *and* required. With the instance-wide switch on that
                // is every unenrolled account; with it off it is the administrators
                // plus the individually-required, which is exactly
                // `required(user)` minus the term already known to be false.
                let unenrolled_and_required: i64 = if instance_required {
                    total - enrolled
                } else {
                    users::table
                        .filter(users::two_factor_enabled.eq(false))
                        .filter(
                            users::is_admin
                                .eq(true)
                                .or(users::two_factor_admin_required.eq(true)),
                        )
                        .count()
                        .get_result(conn)?
                };

                Ok(TwoFactorCoverage {
                    enrolled,
                    not_enrolled: total - enrolled,
                    required_not_enrolled: unenrolled_and_required,
                })
            })
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_: diesel::result::Error| "Failed to count two-factor coverage".to_string())
}
