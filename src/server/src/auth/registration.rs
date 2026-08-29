//! Registration/bootstrap identity concerns split out of `auth/mod.rs`:
//! validating new-account input, gating registration until initial admin
//! setup is complete, and deriving usernames for auto-provisioned accounts
//! (manual registration and OAuth auto-provisioning alike, ADR-011).

use crate::schema::users;
use crate::state::AppState;
use diesel::prelude::*;
// The rules with no database behind them live in
// `thunderforge_axum_auth_core`, where they are proptested. What is left here
// is the part that genuinely needs a connection: gating on admin setup, and
// resolving a username against rows that already exist.
pub(super) use thunderforge_axum_auth_core::password::{
    derive_bootstrap_username, validate_registration_input,
};
pub(super) use thunderforge_axum_auth_core::random::random_setup_code;

pub(super) async fn ensure_registration_allowed(state: &AppState) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let admin_exists = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query admin setup state".to_string())?;

    if admin_exists.is_some() {
        Ok(())
    } else {
        Err("Registration is unavailable until the initial admin setup is complete".to_string())
    }
}

pub(super) enum RegisterUserError {
    UsernameTaken,
    EmailTaken,
    Storage,
}

/// Derives a username from an auto-provisioned OAuth user's email (ADR-011),
/// appending a short random suffix on collision. Bounded retries: a
/// collision on every attempt (astronomically unlikely) falls back to the
/// email-local-part-plus-full-UUID form, which is unique by construction.
pub(super) fn unique_username_from_email_sync(
    conn: &mut diesel::PgConnection,
    email: &str,
) -> Result<String, diesel::result::Error> {
    let base = derive_bootstrap_username(None, email);
    let base = if base.is_empty() {
        "user".to_string()
    } else {
        base
    };

    let is_taken = |conn: &mut diesel::PgConnection, candidate: &str| {
        users::table
            .filter(users::username.eq(candidate))
            .select(users::id)
            .first::<uuid::Uuid>(conn)
            .optional()
            .map(|row| row.is_some())
    };

    if !is_taken(conn, &base)? {
        return Ok(base);
    }

    for _ in 0..5 {
        let candidate = format!("{base}-{}", super::random_urlsafe(4).to_lowercase());
        if !is_taken(conn, &candidate)? {
            return Ok(candidate);
        }
    }

    Ok(format!("{base}-{}", uuid::Uuid::now_v7()))
}

#[cfg(test)]
mod tests {
    use super::unique_username_from_email_sync;
    use crate::schema::users;
    use crate::test_support::test_app_state;
    use diesel::prelude::*;

    /// ADR-011: auto-provisioned OAuth usernames derive from the email
    /// local part when it isn't already taken.
    #[test]
    fn auto_provision_username_uses_email_local_part_when_free() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("failed to get DB connection");
        let email = format!("auto-provision-{}@example.invalid", uuid::Uuid::now_v7());

        let username = unique_username_from_email_sync(&mut conn, &email)
            .expect("username derivation should succeed");

        assert_eq!(username, email.split('@').next().unwrap());
    }

    /// ADR-011: a collision on the email-derived base username falls back
    /// to a suffixed variant rather than erroring, since auto-provisioning
    /// has no user to ask for a different name.
    #[test]
    fn auto_provision_username_avoids_collision_with_existing_username() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().expect("failed to get DB connection");
        let suffix = uuid::Uuid::now_v7().simple().to_string();
        let base_username = format!("collide{suffix}");
        let email = format!("{base_username}@example.invalid");

        diesel::insert_into(users::table)
            .values((
                users::id.eq(uuid::Uuid::now_v7()),
                users::username.eq(&base_username),
                users::password_hash.eq("not-a-real-hash"),
                users::email.eq(format!("someone-else-{suffix}@example.invalid")),
            ))
            .execute(&mut conn)
            .expect("failed to insert colliding user");

        let username = unique_username_from_email_sync(&mut conn, &email)
            .expect("username derivation should succeed despite collision");

        assert_ne!(username, base_username);
        assert!(username.starts_with(&format!("{base_username}-")));
    }
}
