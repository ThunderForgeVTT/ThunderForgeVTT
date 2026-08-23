//! Registration/bootstrap identity concerns split out of `auth/mod.rs`:
//! validating new-account input, gating registration until initial admin
//! setup is complete, and deriving usernames for auto-provisioned accounts
//! (manual registration and OAuth auto-provisioning alike, ADR-011).

use crate::schema::users;
use crate::state::AppState;
use diesel::prelude::*;

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

pub(super) fn validate_registration_input(
    username: &str,
    email: &str,
    password: &str,
) -> Result<(), String> {
    if username.is_empty() || email.is_empty() || password.is_empty() {
        return Err("Username, email, and password are required".to_string());
    }

    if username.len() < 3 || username.len() > 32 {
        return Err("Username must be between 3 and 32 characters".to_string());
    }

    if !username
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err("Username may only contain letters, numbers, '-', '_' and '.'".to_string());
    }

    if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err("Email address is invalid".to_string());
    }

    if password.len() < 12 {
        return Err("Password must be at least 12 characters long".to_string());
    }

    Ok(())
}

pub(super) enum RegisterUserError {
    UsernameTaken,
    EmailTaken,
    Storage,
}

pub(super) fn derive_bootstrap_username(
    desired_username: Option<String>,
    provider_email: &str,
) -> String {
    let candidate = desired_username
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            provider_email
                .split('@')
                .next()
                .unwrap_or("admin")
                .trim()
                .to_string()
        });

    candidate
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect::<String>()
        .to_lowercase()
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
    let base = if base.is_empty() { "user".to_string() } else { base };

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

pub(super) fn random_setup_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 12];
    let mut rng = rand::rng();
    rand::RngExt::fill(&mut rng, &mut bytes);

    let token = bytes
        .iter()
        .map(|byte| CHARSET[*byte as usize % CHARSET.len()] as char)
        .collect::<String>();

    format!("{}-{}-{}", &token[0..4], &token[4..8], &token[8..12])
}

#[cfg(test)]
mod tests {
    use super::{
        derive_bootstrap_username, random_setup_code, unique_username_from_email_sync,
        validate_registration_input,
    };
    use crate::schema::users;
    use crate::test_support::test_app_state;
    use diesel::prelude::*;

    #[test]
    fn registration_validation_rejects_short_password() {
        let result = validate_registration_input("wizard", "wizard@thunderforge.dev", "short");

        assert_eq!(
            result,
            Err("Password must be at least 12 characters long".to_string())
        );
    }

    #[test]
    fn registration_validation_rejects_invalid_username() {
        let result = validate_registration_input(
            "bad name",
            "wizard@thunderforge.dev",
            "very-secure-password",
        );

        assert_eq!(
            result,
            Err("Username may only contain letters, numbers, '-', '_' and '.'".to_string())
        );
    }

    #[test]
    fn registration_validation_accepts_valid_input() {
        let result = validate_registration_input(
            "archmage.1",
            "wizard@thunderforge.dev",
            "very-secure-password",
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn bootstrap_username_falls_back_to_email_local_part() {
        let username = derive_bootstrap_username(None, "Grand.Magister+Admin@thunderforge.dev");

        assert_eq!(username, "grand.magisteradmin");
    }

    #[test]
    fn setup_code_uses_expected_fantasy_friendly_format() {
        let code = random_setup_code();

        assert_eq!(code.len(), 14);
        assert!(
            code.chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-')
        );
        assert_eq!(code.chars().filter(|ch| *ch == '-').count(), 2);
    }

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
