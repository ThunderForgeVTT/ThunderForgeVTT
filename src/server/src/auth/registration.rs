//! Registration/bootstrap identity concerns split out of `auth/mod.rs`:
//! validating new-account input, gating registration until initial admin
//! setup is complete, and deriving usernames for auto-provisioned accounts
//! (manual registration and OAuth auto-provisioning alike, ADR-042).

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

/// The one sentence a refused admission produces (FR-011).
///
/// A constant rather than a literal at each refusal site, so the local and
/// OAuth paths cannot drift into saying different things — which is how a
/// closed instance would start telling an outsider which of the two doors it
/// was, or worse, whether the address they used already exists.
pub(crate) const NOT_ACCEPTING: &str = "This instance is not accepting new accounts";

/// What the gate decided.
pub(crate) enum Admission {
    /// Admit, consuming nothing.
    Allowed,
    /// Admit, and consume one use of this invitation. Carries the row id so
    /// the caller can write the redemption without looking the code up twice.
    AllowedByInvitation(uuid::Uuid),
}

/// Why the gate refused. The caller turns this into a `409` or a redirect;
/// the distinction never reaches the person, who sees one sentence.
pub(crate) enum AdmissionRefused {
    /// The policy admits nobody on this route.
    Policy(String),
    /// An invitation was presented and is unusable — revoked, expired,
    /// exhausted, or never existed. Deliberately one variant: distinguishing
    /// them tells a stranger holding a guessed code whether they guessed a
    /// real one (FR-011).
    InvitationUnusable,
    /// The caller is being throttled (FR-019a). Distinct from
    /// `InvitationUnusable` on purpose: a legitimate recipient retrying must
    /// not be told their invitation is bad.
    RateLimited(String),
    /// Something went wrong reading the policy. Fails shut.
    Unavailable(String),
}

/// **The instance's answer to "may this request create an account?"**
///
/// # Every account-creating path calls this
///
/// ADR-072. There are exactly two — [`super::sessions::register`] and
/// [`super::oauth::resolve_oauth_login`] — and a third added later inherits
/// the requirement. A path that creates an account without calling this is the
/// defect spec 035 exists to prevent, not an oversight to fix afterwards.
///
/// # The first-run branch comes first, and stays
///
/// FR-010: an instance with no administrator must be able to make its first
/// one whatever the policy says, or a conservative default would brick a fresh
/// install. That check was already here answering "has setup happened?"; it is
/// kept as the first branch rather than re-derived somewhere else.
///
/// # `closed` refuses even a valid invitation
///
/// FR-001 — "no account may be created by any means". Reopening to
/// `invite_only` makes an issued invitation work again; the policy switch
/// never alters the invitation's own validity (FR-020).
pub(crate) async fn ensure_admission_allowed(
    state: &AppState,
    route: &crate::auth::instance_access::AdmissionRoute,
    invitation: Option<&str>,
) -> Result<Admission, AdmissionRefused> {
    use crate::auth::instance_access::{InstanceAccessPolicy, load_policy};

    // FR-010, first and unconditionally: before there is an administrator,
    // there is nothing for a policy to protect and no one to change it.
    if !admin_exists(state)
        .await
        .map_err(AdmissionRefused::Unavailable)?
    {
        return Ok(Admission::Allowed);
    }

    let policy = load_policy(state)
        .await
        .map_err(AdmissionRefused::Unavailable)?;

    match policy {
        InstanceAccessPolicy::Open => match invitation {
            // An invitation presented on an open instance is still consumed:
            // the operator issued it deliberately, and silently ignoring it
            // would make the redemption list lie about how someone arrived.
            Some(code) => consume_invitation(state, code, route).await,
            None => Ok(Admission::Allowed),
        },
        InstanceAccessPolicy::InviteOnly => match invitation {
            Some(code) => consume_invitation(state, code, route).await,
            None => Err(AdmissionRefused::Policy(NOT_ACCEPTING.to_string())),
        },
        // No exception for a valid invitation. FR-001.
        InstanceAccessPolicy::Closed => Err(AdmissionRefused::Policy(NOT_ACCEPTING.to_string())),
    }
}

/// Consumes one use of an invitation, or refuses uniformly.
///
/// Delegates to [`crate::auth::instance_access::consume_invitation_use`],
/// which carries the whole validity predicate in a single conditional UPDATE
/// so that check-and-increment is indivisible (SC-006).
async fn consume_invitation(
    state: &AppState,
    code: &str,
    route: &crate::auth::instance_access::AdmissionRoute,
) -> Result<Admission, AdmissionRefused> {
    crate::auth::instance_access::consume_invitation_use(state, code, route).await
}

async fn admin_exists(state: &AppState) -> Result<bool, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let found = tokio::task::spawn_blocking(move || {
        users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query admin setup state".to_string())?;

    Ok(found.is_some())
}

pub(super) enum RegisterUserError {
    UsernameTaken,
    EmailTaken,
    Storage,
}

/// Derives a username from an auto-provisioned OAuth user's email (ADR-042),
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

    /// ADR-042: auto-provisioned OAuth usernames derive from the email
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

    /// ADR-042: a collision on the email-derived base username falls back
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
