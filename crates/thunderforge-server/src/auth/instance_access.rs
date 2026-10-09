//! Spec 035 / ADR-072: the instance's admission policy, and the one gate that
//! consults it.
//!
//! # Why the gate is a single function
//!
//! There are exactly two paths that bring a new user account into existence
//! outside first-run bootstrap: [`super::sessions::register`] and
//! [`super::oauth::resolve_oauth_login`]. Before this module, only the first
//! was gated at all — and by a function answering a different question, "has
//! setup happened?".
//!
//! That asymmetry is the defect this feature exists to close. An "allow
//! signups" switch governing only the local form is worse than useless: the
//! operator watches the registration form disappear, concludes the instance is
//! shut, and keeps admitting every stranger who signs in with a provider.
//! Nothing reports it until an unknown account appears in the user list.
//!
//! So admission is decided in one place, [`super::registration::ensure_admission_allowed`],
//! and this module holds the policy it reads.
//!
//! # What this does NOT decide
//!
//! How an admitted account is created. That is still ADR-042's: the derived
//! username, the unusable password hash, the immediate identity link, and the
//! untouched password-confirmation rule for linking to an account that already
//! exists. This module answers *whether*, and stops there.

use diesel::prelude::*;
use uuid::Uuid;

use crate::models::{InstanceAccessSetting, NewInstanceAccessEvent};
use crate::schema::{instance_access_events, instance_access_settings};
use crate::state::AppState;

/// The instance's three admission states (FR-001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceAccessPolicy {
    /// Anyone may create an account. Behaviour is exactly as it was before
    /// this feature existed.
    Open,
    /// An account may be created only by redeeming a valid invitation.
    InviteOnly,
    /// No account may be created by any means — a valid invitation included.
    Closed,
}

impl InstanceAccessPolicy {
    pub fn as_db_str(self) -> &'static str {
        match self {
            InstanceAccessPolicy::Open => "open",
            InstanceAccessPolicy::InviteOnly => "invite_only",
            InstanceAccessPolicy::Closed => "closed",
        }
    }

    /// Parses a stored policy, **failing shut**.
    ///
    /// An unrecognised value degrades to [`Closed`](Self::Closed), never to
    /// `Open`. The surrounding code parses `ActorPermissionLevel` the same way
    /// and degrades to `Viewer` — its floor. This enum's floor, in the sense
    /// that matters, is the state that admits nobody: a database that has
    /// somehow acquired a policy this build does not understand must not be
    /// read as an invitation to let everyone in.
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "open" => InstanceAccessPolicy::Open,
            "invite_only" => InstanceAccessPolicy::InviteOnly,
            _ => InstanceAccessPolicy::Closed,
        }
    }
}

/// Which door an admission attempt came through. Recorded on a refusal
/// (FR-012) and on a redemption.
#[derive(Debug, Clone)]
pub enum AdmissionRoute {
    Local,
    OAuth(String),
}

impl AdmissionRoute {
    pub fn as_db_str(&self) -> String {
        match self {
            AdmissionRoute::Local => "local".to_string(),
            AdmissionRoute::OAuth(provider) => format!("oauth:{provider}"),
        }
    }
}

/// Reads the policy, creating the singleton row if it is somehow absent.
///
/// The row is normally seeded by the migration, which is the only place that
/// can tell a fresh instance from an upgraded one (FR-013, FR-013a). This
/// fallback exists so a missing row cannot take the instance down, and it
/// chooses `closed` for the same fail-shut reason [`InstanceAccessPolicy::from_db_str`]
/// does — a policy nobody set should not be an open door.
pub async fn load_policy(state: &AppState) -> Result<InstanceAccessPolicy, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let row = tokio::task::spawn_blocking(move || {
        instance_access_settings::table
            .filter(instance_access_settings::id.eq(1))
            .select(InstanceAccessSetting::as_select())
            .first::<InstanceAccessSetting>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query the instance access policy".to_string())?;

    Ok(row
        .map(|r| InstanceAccessPolicy::from_db_str(&r.access_policy))
        .unwrap_or(InstanceAccessPolicy::Closed))
}

/// Writes one append-only access event (FR-004, FR-012).
///
/// Note the parameters: there is no way to pass an email address or any other
/// submitted identifier, and none may be added. FR-012 requires the time, the
/// route and the provider — not the identity of whoever was refused. A
/// signature that cannot express the violation is a stronger guarantee than a
/// comment asking callers not to commit it.
pub async fn record_access_event(
    state: &AppState,
    event_type: &str,
    actor_user_id: Option<Uuid>,
    previous_policy: Option<InstanceAccessPolicy>,
    new_policy: Option<InstanceAccessPolicy>,
    attempted_route: Option<&AdmissionRoute>,
    policy_at_attempt: Option<InstanceAccessPolicy>,
) -> Result<(), String> {
    let row = NewInstanceAccessEvent {
        id: Uuid::now_v7(),
        event_type: event_type.to_string(),
        actor_user_id,
        previous_policy: previous_policy.map(|p| p.as_db_str().to_string()),
        new_policy: new_policy.map(|p| p.as_db_str().to_string()),
        attempted_route: attempted_route.map(|r| r.as_db_str()),
        policy_at_attempt: policy_at_attempt.map(|p| p.as_db_str().to_string()),
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        diesel::insert_into(instance_access_events::table)
            .values(&row)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to record the access event".to_string())?;

    Ok(())
}

/// Judges whether an invitation could admit someone, burning nothing
/// (FR-011, FR-016, FR-019a, FR-020a).
///
/// # Judging and burning are two steps, on purpose
///
/// A use is burned only when the account it admits is written, in the same
/// transaction as that write — see [`claim_invitation_use_sync`]. This step
/// only answers "is this code usable right now?", so that a request refused
/// later (a short password, a taken username, an abandoned OAuth flow, a bot
/// replaying the link) leaves the count where it was. The owner's rule
/// (2026-10-09): nothing but a created account counts.
///
/// 0. **Rate limit, before the code is looked up.** An unguessable code is
///    unguessable only while the number of guesses is bounded, and the account
///    requirement that used to bound them is exactly what this feature
///    removes. Reuses the limiter already guarding the anonymous share reads.
/// 1. **A read carrying the same validity predicate as the claim.** Revoked,
///    expired, exhausted and never-existed all answer "unusable", and which of
///    the four is never distinguished (FR-011). Admission is still judged
///    here, before the caller's uniqueness probes, so a stranger holding a
///    bad code learns nothing about which usernames exist.
///
/// A usable answer is not a reservation. Two people may both be told the
/// last use is available; the claim decides which of them gets it, and the
/// other is refused exactly as an exhausted invitation is (SC-006).
pub(crate) async fn check_invitation_usable(
    state: &AppState,
    code: &str,
    route: &AdmissionRoute,
) -> Result<super::registration::Admission, super::registration::AdmissionRefused> {
    use super::registration::{Admission, AdmissionRefused};
    use crate::schema::instance_invitations as inv;

    // Step 0. FR-019a.
    let caller = route.as_db_str();
    if !crate::graphql::share_rate_limit::allow_request(&caller) {
        return Err(AdmissionRefused::RateLimited(
            crate::graphql::share_rate_limit::rate_limited_message().to_string(),
        ));
    }

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| AdmissionRefused::Unavailable("Failed to get DB connection".to_string()))?;
    let code = code.to_string();

    // Step 1. Read-only: nothing is written until an account is.
    let found = tokio::task::spawn_blocking(move || {
        let now = chrono::Utc::now().naive_utc();
        inv::table
            .filter(inv::invite_code.eq(&code))
            .filter(inv::revoked.eq(false))
            .filter(inv::expires_at.is_null().or(inv::expires_at.gt(now)))
            .filter(inv::used_count.lt(inv::max_uses))
            .select(inv::id)
            .first::<Uuid>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| AdmissionRefused::Unavailable("Failed to spawn blocking task".to_string()))?
    .map_err(|_| AdmissionRefused::Unavailable("Failed to read the invitation".to_string()))?;

    match found {
        Some(id) => Ok(Admission::AllowedByInvitation(id)),
        // FR-011: every unusable reason leaves as one refusal.
        None => Err(AdmissionRefused::InvitationUnusable),
    }
}

/// Burns one use of an invitation, inside the caller's transaction.
///
/// Call it in the same transaction that writes the account the use admits,
/// and roll that transaction back when it returns `false`: the invitation
/// became unusable between [`check_invitation_usable`] and now (exhausted by
/// a concurrent signup, revoked, or expired), and the caller refuses exactly
/// as it would for an exhausted invitation.
///
/// # Why one conditional UPDATE
///
/// The whole validity predicate sits in the WHERE clause. Not a read, an
/// in-memory check, and a write-back: that sequence loses updates, and spec
/// 027 shipped it once — two redeemers racing the last use both read
/// `used_count = N`, both computed `N + 1`, and both wrote it. Here the second
/// racer waits on the first one's row lock, re-reads the row when it commits,
/// finds `used_count = max_uses`, and matches nothing (SC-006). Because the
/// claim and the account insert commit together, a signup that fails after
/// the claim takes its use back with it, and an account never exists without
/// the use that admitted it.
pub(crate) fn claim_invitation_use_sync(
    conn: &mut diesel::PgConnection,
    invitation_id: Uuid,
) -> QueryResult<bool> {
    use crate::schema::instance_invitations as inv;
    let now = chrono::Utc::now().naive_utc();
    let updated = diesel::update(
        inv::table
            .filter(inv::id.eq(invitation_id))
            .filter(inv::revoked.eq(false))
            .filter(inv::expires_at.is_null().or(inv::expires_at.gt(now)))
            .filter(inv::used_count.lt(inv::max_uses)),
    )
    .set((
        inv::used_count.eq(inv::used_count + 1),
        inv::updated_at.eq(now),
    ))
    .execute(conn)?;
    Ok(updated == 1)
}

/// Records that an account was admitted by an invitation (FR-018), inside the
/// same transaction as the claim, so a recorded redemption always has a burned
/// use and an account behind it.
pub(crate) fn record_redemption_sync(
    conn: &mut diesel::PgConnection,
    invitation_id: Uuid,
    user_id: Uuid,
    route: &AdmissionRoute,
) -> QueryResult<()> {
    let row = crate::models::NewInstanceInvitationRedemption {
        id: Uuid::now_v7(),
        invitation_id,
        user_id,
        route: route.as_db_str(),
    };
    diesel::insert_into(crate::schema::instance_invitation_redemptions::table)
        .values(&row)
        .execute(conn)?;
    Ok(())
}

/// Claims a use and records the redemption for an account just written in
/// this transaction. `Ok(false)` means the invitation is no longer usable and
/// the caller must roll back and refuse (FR-011).
pub(crate) fn redeem_for_account_sync(
    conn: &mut diesel::PgConnection,
    invitation_id: Uuid,
    user_id: Uuid,
    route: &AdmissionRoute,
) -> QueryResult<bool> {
    if !claim_invitation_use_sync(conn, invitation_id)? {
        return Ok(false);
    }
    record_redemption_sync(conn, invitation_id, user_id, route)?;
    Ok(true)
}

/// Records a refused admission (FR-012), best-effort.
///
/// Best-effort on purpose: an audit write that fails must not turn a clean
/// refusal into a 500, because the refusal is the security-relevant outcome
/// and it has already been decided. The policy at the time is captured here
/// rather than passed in, so the row explains itself when read months later.
pub(crate) async fn record_refusal(state: &AppState, route: &AdmissionRoute) {
    let policy = load_policy(state).await.ok();
    let _ = record_access_event(
        state,
        "admission_refused",
        None,
        None,
        None,
        Some(route),
        policy,
    )
    .await;
}

#[cfg(test)]
#[path = "instance_access_tests.rs"]
mod tests;
