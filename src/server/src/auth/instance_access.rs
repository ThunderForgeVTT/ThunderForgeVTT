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

/// Consumes one use of an invitation, or refuses uniformly (FR-011, FR-019a,
/// FR-020a, SC-006).
///
/// # The order of these steps is the design
///
/// 0. **Rate limit, before the code is looked up.** An unguessable code is
///    unguessable only while the number of guesses is bounded, and the account
///    requirement that used to bound them is exactly what this feature
///    removes. Reuses the limiter already guarding the anonymous share reads.
/// 1. **The conditional UPDATE**, carrying the whole validity predicate in its
///    WHERE clause. Not a read, an in-memory check, and a write-back: that
///    sequence loses updates, and spec 027 shipped it once — two redeemers
///    racing the last use both read `used_count = N`, both computed `N + 1`,
///    and both wrote it. Zero rows updated means unusable, and *which* of the
///    four reasons is never distinguished.
///
/// The already-a-user check is **not** here. It belongs to the caller, before
/// this is ever reached, because it must not consume a use — see
/// [`super::sessions`] and [`super::oauth`] (FR-020a).
pub(crate) async fn consume_invitation_use(
    state: &AppState,
    code: &str,
    route: &AdmissionRoute,
) -> Result<super::registration::Admission, super::registration::AdmissionRefused> {
    use super::registration::{Admission, AdmissionRefused};

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

    // Step 1. One statement: the check and the increment are indivisible.
    let updated = tokio::task::spawn_blocking(move || {
        let now = chrono::Utc::now().naive_utc();
        diesel::update(
            crate::schema::instance_invitations::table
                .filter(crate::schema::instance_invitations::invite_code.eq(&code))
                .filter(crate::schema::instance_invitations::revoked.eq(false))
                .filter(
                    crate::schema::instance_invitations::expires_at
                        .is_null()
                        .or(crate::schema::instance_invitations::expires_at.gt(now)),
                )
                .filter(
                    crate::schema::instance_invitations::used_count
                        .lt(crate::schema::instance_invitations::max_uses),
                ),
        )
        .set((
            crate::schema::instance_invitations::used_count
                .eq(crate::schema::instance_invitations::used_count + 1),
            crate::schema::instance_invitations::updated_at.eq(now),
        ))
        .returning(crate::schema::instance_invitations::id)
        .get_result::<Uuid>(&mut conn)
        .optional()
    })
    .await
    .map_err(|_| AdmissionRefused::Unavailable("Failed to spawn blocking task".to_string()))?
    .map_err(|_| AdmissionRefused::Unavailable("Failed to redeem the invitation".to_string()))?;

    match updated {
        Some(id) => Ok(Admission::AllowedByInvitation(id)),
        // Revoked, expired, exhausted and never-existed all arrive here, and
        // all leave as one refusal. FR-011.
        None => Err(AdmissionRefused::InvitationUnusable),
    }
}

/// Releases a use taken by [`consume_invitation_use`] when the account it was
/// taken for was never created.
///
/// A failed signup must not burn a use. The consume and the account insert
/// cannot share one transaction — the account write lives in the auth flow and
/// the consume must be visible to concurrent redeemers the instant it happens,
/// which is what makes SC-006 hold — so the compensation is explicit.
///
/// # The window this leaves, and why it is the right way round
///
/// Between the consume and the release, a concurrent redeemer of the last use
/// is refused as though the invitation were exhausted, and then it is not.
/// That is a transient over-refusal, and it errs in the safe direction: the
/// count is never *under*-stated, so SC-006's "at most N" holds at every
/// instant. The alternative — holding the row locked across the account
/// insert — would serialise redemption behind a password hash, and trade a
/// rare spurious refusal for a guaranteed slow one.
pub(crate) async fn release_invitation_use(state: &AppState, invitation_id: Uuid) {
    let Ok(mut conn) = state.db_pool.get() else {
        return;
    };
    let _ = tokio::task::spawn_blocking(move || {
        diesel::update(
            crate::schema::instance_invitations::table
                .filter(crate::schema::instance_invitations::id.eq(invitation_id))
                .filter(crate::schema::instance_invitations::used_count.gt(0)),
        )
        .set(
            crate::schema::instance_invitations::used_count
                .eq(crate::schema::instance_invitations::used_count - 1),
        )
        .execute(&mut conn)
    })
    .await;
}

/// Records that an account was admitted by an invitation (FR-018).
pub(crate) async fn record_redemption(
    state: &AppState,
    invitation_id: Uuid,
    user_id: Uuid,
    route: &AdmissionRoute,
) -> Result<(), String> {
    let row = crate::models::NewInstanceInvitationRedemption {
        id: Uuid::now_v7(),
        invitation_id,
        user_id,
        route: route.as_db_str(),
    };
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(crate::schema::instance_invitation_redemptions::table)
            .values(&row)
            .execute(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to record the redemption".to_string())?;
    Ok(())
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
mod tests {
    use super::*;
    use crate::auth::registration::{Admission, AdmissionRefused, ensure_admission_allowed};
    use crate::schema::users;
    use crate::test_support::{
        insert_test_instance_invitation, insert_test_user, set_instance_access_policy,
        test_app_state,
    };

    /// The policy must be set explicitly rather than inherited from the
    /// migration's seed: the shared test database has users in it, so it
    /// seeded `open`, and a test that assumed otherwise would pass for the
    /// wrong reason.
    fn arrange(policy: &str) -> (crate::state::AppState, uuid::Uuid) {
        dotenvy::dotenv().ok();
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        // FR-010's exemption is unconditional and first, so every test that
        // wants a policy honoured needs an administrator to exist.
        let admin_exists = users::table
            .filter(users::is_admin.eq(true))
            .select(users::id)
            .first::<uuid::Uuid>(&mut conn)
            .optional()
            .unwrap();
        let admin_id = match admin_exists {
            Some(id) => id,
            None => {
                let id = insert_test_user(&mut conn);
                diesel::update(users::table.filter(users::id.eq(id)))
                    .set(users::is_admin.eq(true))
                    .execute(&mut conn)
                    .unwrap();
                id
            }
        };
        set_instance_access_policy(&mut conn, policy);
        drop(conn);
        (state, admin_id)
    }

    /// Fail shut. A stored policy this build does not understand must not be
    /// read as an invitation to let everyone in.
    #[test]
    fn an_unknown_stored_policy_degrades_to_closed() {
        assert_eq!(
            InstanceAccessPolicy::from_db_str("open"),
            InstanceAccessPolicy::Open
        );
        assert_eq!(
            InstanceAccessPolicy::from_db_str("invite_only"),
            InstanceAccessPolicy::InviteOnly
        );
        for nonsense in ["", "OPEN", "anything", "opened", "invite-only"] {
            assert_eq!(
                InstanceAccessPolicy::from_db_str(nonsense),
                InstanceAccessPolicy::Closed,
                "{nonsense:?} must fail shut"
            );
        }
    }

    /// FR-001 and the contract's decision table, all three states.
    #[tokio::test]
    async fn the_policy_decides_admission_on_every_route() {
        let (state, admin_id) = arrange("open");
        let route = AdmissionRoute::Local;

        assert!(
            matches!(
                ensure_admission_allowed(&state, &route, None).await,
                Ok(Admission::Allowed)
            ),
            "an open instance admits with no invitation"
        );

        {
            let mut conn = state.db_pool.get().unwrap();
            set_instance_access_policy(&mut conn, "invite_only");
        }
        assert!(
            matches!(
                ensure_admission_allowed(&state, &route, None).await,
                Err(AdmissionRefused::Policy(_))
            ),
            "invite-only refuses without an invitation"
        );

        let code = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_instance_invitation(&mut conn, admin_id, 1, None, false).1
        };
        assert!(
            matches!(
                ensure_admission_allowed(&state, &route, Some(&code)).await,
                Ok(Admission::AllowedByInvitation(_))
            ),
            "invite-only admits with a valid invitation"
        );

        // FR-001: closed refuses even a valid invitation. This is the case a
        // reader is most likely to "fix" by adding an exception — it is not
        // an omission.
        {
            let mut conn = state.db_pool.get().unwrap();
            set_instance_access_policy(&mut conn, "closed");
        }
        let fresh = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_instance_invitation(&mut conn, admin_id, 1, None, false).1
        };
        assert!(
            matches!(
                ensure_admission_allowed(&state, &route, Some(&fresh)).await,
                Err(AdmissionRefused::Policy(_))
            ),
            "a closed instance admits nobody, invitation or not"
        );
    }

    /// FR-011: revoked, expired, exhausted and never-existed are one refusal.
    /// Distinguishing them tells a stranger holding a guessed code whether
    /// they guessed a real one.
    #[tokio::test]
    async fn every_unusable_invitation_refuses_identically() {
        let (state, admin_id) = arrange("invite_only");
        let route = AdmissionRoute::Local;

        let (revoked, expired, exhausted) = {
            let mut conn = state.db_pool.get().unwrap();
            let revoked = insert_test_instance_invitation(&mut conn, admin_id, 1, None, true).1;
            let past = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
            let expired =
                insert_test_instance_invitation(&mut conn, admin_id, 1, Some(past), false).1;
            let (id, code) = insert_test_instance_invitation(&mut conn, admin_id, 1, None, false);
            diesel::update(
                crate::schema::instance_invitations::table
                    .filter(crate::schema::instance_invitations::id.eq(id)),
            )
            .set(crate::schema::instance_invitations::used_count.eq(1))
            .execute(&mut conn)
            .unwrap();
            (revoked, expired, code)
        };

        for code in [
            revoked.as_str(),
            expired.as_str(),
            exhausted.as_str(),
            "NOTAREALCODEATALL0",
        ] {
            let result = ensure_admission_allowed(&state, &route, Some(code)).await;
            assert!(
                matches!(result, Err(AdmissionRefused::InvitationUnusable)),
                "{code} must refuse as merely unusable, saying nothing about why"
            );
        }
    }

    /// FR-020a: an operator testing the link they issued must not destroy it.
    /// The use is consumed only when an account is actually created, so a
    /// consume followed by a release leaves the count where it started.
    #[tokio::test]
    async fn a_released_use_returns_to_the_invitation() {
        let (state, admin_id) = arrange("invite_only");
        let (id, code) = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_instance_invitation(&mut conn, admin_id, 1, None, false)
        };

        let Ok(Admission::AllowedByInvitation(invitation_id)) =
            ensure_admission_allowed(&state, &AdmissionRoute::Local, Some(&code)).await
        else {
            panic!("a valid invitation must be consumable");
        };
        assert_eq!(invitation_id, id);

        release_invitation_use(&state, invitation_id).await;

        let mut conn = state.db_pool.get().unwrap();
        let used = crate::schema::instance_invitations::table
            .filter(crate::schema::instance_invitations::id.eq(id))
            .select(crate::schema::instance_invitations::used_count)
            .first::<i32>(&mut conn)
            .unwrap();
        assert_eq!(used, 0, "a use released must be a use available again");
    }

    /// SC-006: an N-use invitation admits at most N, including when
    /// redemptions race. This is the test that catches a regression from the
    /// conditional UPDATE back to read-then-write.
    #[tokio::test]
    async fn an_n_use_invitation_admits_exactly_n_under_concurrency() {
        let (state, admin_id) = arrange("invite_only");
        const N: i32 = 3;
        let attempts = (N + 2) as usize;

        let (id, code) = {
            let mut conn = state.db_pool.get().unwrap();
            insert_test_instance_invitation(&mut conn, admin_id, N, None, false)
        };

        let mut handles = Vec::new();
        for _ in 0..attempts {
            let state = state.clone();
            let code = code.clone();
            handles.push(tokio::spawn(async move {
                matches!(
                    ensure_admission_allowed(&state, &AdmissionRoute::Local, Some(&code)).await,
                    Ok(Admission::AllowedByInvitation(_))
                )
            }));
        }

        let mut admitted = 0;
        for h in handles {
            if h.await.unwrap() {
                admitted += 1;
            }
        }

        assert_eq!(
            admitted, N,
            "{attempts} concurrent redemptions of a {N}-use invitation must admit exactly {N}"
        );

        let mut conn = state.db_pool.get().unwrap();
        let used = crate::schema::instance_invitations::table
            .filter(crate::schema::instance_invitations::id.eq(id))
            .select(crate::schema::instance_invitations::used_count)
            .first::<i32>(&mut conn)
            .unwrap();
        assert_eq!(used, N, "the stored count must never exceed the cap");
    }

    /// FR-004 and FR-012: both event kinds are written, and neither carries an
    /// address. The struct makes that structural, and this asserts it stays so.
    #[tokio::test]
    async fn access_events_record_the_act_and_never_the_person() {
        let (state, admin_id) = arrange("closed");

        record_refusal(&state, &AdmissionRoute::OAuth("google".to_string())).await;
        crate::admin::update_instance_access_policy(
            &state,
            admin_id,
            InstanceAccessPolicy::InviteOnly,
        )
        .await
        .expect("an administrator may change the policy");

        let mut conn = state.db_pool.get().unwrap();
        let rows = crate::schema::instance_access_events::table
            .order(crate::schema::instance_access_events::occurred_at.desc())
            .select(crate::models::InstanceAccessEvent::as_select())
            .limit(10)
            .load::<crate::models::InstanceAccessEvent>(&mut conn)
            .unwrap();

        let refusal = rows
            .iter()
            .find(|r| r.event_type == "admission_refused")
            .expect("a refusal must be recorded");
        assert_eq!(refusal.attempted_route.as_deref(), Some("oauth:google"));
        assert_eq!(refusal.policy_at_attempt.as_deref(), Some("closed"));

        let change = rows
            .iter()
            .find(|r| r.event_type == "policy_changed")
            .expect("a policy change must be recorded");
        assert_eq!(change.actor_user_id, Some(admin_id));
        assert_eq!(change.previous_policy.as_deref(), Some("closed"));
        assert_eq!(change.new_policy.as_deref(), Some("invite_only"));

        // The rendered row must contain nothing that identifies a person
        // beyond the administrator who acted.
        for row in &rows {
            let rendered = format!("{row:?}");
            assert!(
                !rendered.contains('@'),
                "an access event must never carry an address: {rendered}"
            );
        }
    }
}
