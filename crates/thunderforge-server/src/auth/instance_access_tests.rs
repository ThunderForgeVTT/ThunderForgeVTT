use super::*;
use crate::auth::registration::{Admission, AdmissionRefused, ensure_admission_allowed};
use crate::schema::users;
use crate::test_support::{
    insert_test_instance_invitation, insert_test_user, set_instance_access_policy, test_app_state,
};

/// The instance's access policy is one row, and the events table is
/// global. So these tests cannot be isolated by giving each its own data
/// the way the rest of the suite is: whatever policy one sets, the next
/// one reads. They are serialised instead — `arrange` takes the lock and
/// hands it to the test, which holds it until it returns.
///
/// Without this they pass alone and fail together, which is the worst way
/// for a test to be wrong: a `--test-threads=1` rerun says "green" and the
/// default command says "broken gate". `repo_host_tests.rs` serialises
/// process-global environment variables behind the same idiom.
static POLICY_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Holds the lock, and puts the instance back to `open` when it is
/// dropped.
///
/// # Why the restore matters outside this file
///
/// `instance_access_settings` is one global row, and it is the *same* row
/// the local dev instance reads. Whichever of these tests ran last used to
/// leave its policy behind — so a suite that finished on the `closed` case
/// left a closed instance, and the e2e suite running against the same
/// database then failed **every spec that registers an account**, with a
/// timeout on `#register-username` and nothing anywhere near the cause. It
/// looked exactly like a broken registration page.
///
/// `open` is the right thing to restore to: it is what the migration's
/// conditional seed chooses for a database that already has users in it,
/// which every database this runs against does.
struct PolicyGuard {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    pool: crate::state::DbPool,
}

impl Drop for PolicyGuard {
    fn drop(&mut self) {
        // Best-effort: a test that has already failed must not be reported
        // as a panic in the cleanup instead.
        if let Ok(mut conn) = self.pool.get() {
            set_instance_access_policy(&mut conn, "open");
        }
    }
}

/// The policy must be set explicitly rather than inherited from the
/// migration's seed: the shared test database has users in it, so it
/// seeded `open`, and a test that assumed otherwise would pass for the
/// wrong reason.
async fn arrange(policy: &str) -> (PolicyGuard, crate::state::AppState, uuid::Uuid) {
    let lock = POLICY_LOCK.lock().await;
    crate::test_support::load_dotenv();
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
    let guard = PolicyGuard {
        _lock: lock,
        pool: state.db_pool.clone(),
    };
    (guard, state, admin_id)
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
    let (_policy, state, admin_id) = arrange("open").await;
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
    let (_policy, state, admin_id) = arrange("invite_only").await;
    let route = AdmissionRoute::Local;

    let (revoked, expired, exhausted) = {
        let mut conn = state.db_pool.get().unwrap();
        let revoked = insert_test_instance_invitation(&mut conn, admin_id, 1, None, true).1;
        let past = chrono::Utc::now().naive_utc() - chrono::Duration::hours(1);
        let expired = insert_test_instance_invitation(&mut conn, admin_id, 1, Some(past), false).1;
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

fn used_count(state: &crate::state::AppState, id: Uuid) -> i32 {
    let mut conn = state.db_pool.get().unwrap();
    crate::schema::instance_invitations::table
        .filter(crate::schema::instance_invitations::id.eq(id))
        .select(crate::schema::instance_invitations::used_count)
        .first::<i32>(&mut conn)
        .unwrap()
}

fn redemptions_of(state: &crate::state::AppState, id: Uuid) -> Vec<Uuid> {
    let mut conn = state.db_pool.get().unwrap();
    crate::schema::instance_invitation_redemptions::table
        .filter(crate::schema::instance_invitation_redemptions::invitation_id.eq(id))
        .select(crate::schema::instance_invitation_redemptions::user_id)
        .load::<Uuid>(&mut conn)
        .unwrap()
}

fn user_with_email(state: &crate::state::AppState, email: &str) -> Option<Uuid> {
    let mut conn = state.db_pool.get().unwrap();
    users::table
        .filter(users::email.eq(email))
        .select(users::id)
        .first::<Uuid>(&mut conn)
        .optional()
        .unwrap()
}

async fn sign_up(
    state: &crate::state::AppState,
    code: &str,
    username: String,
    email: String,
    password: &str,
) -> (
    axum::http::StatusCode,
    axum::Json<super::super::AuthSessionResponse>,
) {
    super::super::sessions::register(
        tower_cookies::Cookies::default(),
        super::super::ClientDescription::unknown(),
        axum::extract::State(state.clone()),
        axum::Json(super::super::RegisterRequest {
            username,
            email,
            password: password.to_string(),
            invitation_code: Some(code.to_string()),
        }),
    )
    .await
}

/// The owner's decision: a use is burned only when an account is created.
/// Asking whether an invitation would admit — which is all a bot, a crawler,
/// a refresh or a second click ever reaches — must leave the count alone,
/// however many times it is asked.
#[tokio::test]
async fn judging_admission_burns_no_use() {
    let (_policy, state, admin_id) = arrange("invite_only").await;
    let (id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_instance_invitation(&mut conn, admin_id, 1, None, false)
    };

    for attempt in 0..3 {
        let Ok(Admission::AllowedByInvitation(invitation_id)) =
            ensure_admission_allowed(&state, &AdmissionRoute::Local, Some(&code)).await
        else {
            panic!("attempt {attempt}: a valid invitation must admit");
        };
        assert_eq!(invitation_id, id);
    }
    assert_eq!(
        used_count(&state, id),
        0,
        "admission alone must burn nothing"
    );
}

/// FR-016: a signup the server refuses must not burn a use. A vtt-dev
/// invitation once read 7 of 7 used with three accounts made through it,
/// because a too-short password returned after the use was taken. Nothing is
/// taken now until the account row is written, in the same transaction.
#[tokio::test]
async fn a_refused_signup_burns_no_use() {
    let (_policy, state, admin_id) = arrange("invite_only").await;
    let (id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_instance_invitation(&mut conn, admin_id, 7, None, false)
    };
    let suffix = &Uuid::now_v7().simple().to_string()[..12];
    // `insert_test_user` names its rows past the username length limit, so
    // the taken name is written here, short enough to pass validation.
    let taken = format!("t{suffix}");
    {
        let mut conn = state.db_pool.get().unwrap();
        diesel::insert_into(users::table)
            .values((
                users::id.eq(Uuid::now_v7()),
                users::username.eq(&taken),
                users::password_hash.eq("not-a-real-hash"),
                users::email.eq(format!("taken-{suffix}@example.invalid")),
            ))
            .execute(&mut conn)
            .unwrap();
    }

    let refused = [
        (
            "a too-short password",
            format!("u{suffix}"),
            "short",
            axum::http::StatusCode::BAD_REQUEST,
        ),
        (
            "a username with a space",
            format!("u {suffix}"),
            "a-long-enough-password",
            axum::http::StatusCode::BAD_REQUEST,
        ),
        (
            "a username already taken",
            taken,
            "a-long-enough-password",
            axum::http::StatusCode::CONFLICT,
        ),
    ];
    for (why, username, password, expected) in refused {
        let (status, _) = sign_up(
            &state,
            &code,
            username,
            format!("{suffix}@example.test"),
            password,
        )
        .await;
        assert_eq!(status, expected, "{why}");
        assert_eq!(
            used_count(&state, id),
            0,
            "{why} must not burn a use of the invitation"
        );
    }
    assert!(redemptions_of(&state, id).is_empty());
}

/// A signup that creates an account burns exactly one use, and records the
/// redemption against the account it made (FR-018).
#[tokio::test]
async fn a_successful_signup_burns_exactly_one_use() {
    let (_policy, state, admin_id) = arrange("invite_only").await;
    let (id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_instance_invitation(&mut conn, admin_id, 5, None, false)
    };
    let suffix = &Uuid::now_v7().simple().to_string()[..12];
    let email = format!("{suffix}@example.test");

    let (status, _) = sign_up(
        &state,
        &code,
        format!("u{suffix}"),
        email.clone(),
        "a-long-enough-password",
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::CREATED);

    let user_id = user_with_email(&state, &email).expect("the account must exist");
    assert_eq!(used_count(&state, id), 1, "one account, one use");
    assert_eq!(redemptions_of(&state, id), vec![user_id]);
}

/// SC-006: signups racing for the last use admit exactly one. The losers are
/// refused as an exhausted invitation would be, and leave no account behind.
#[tokio::test]
async fn signups_racing_for_the_last_use_admit_exactly_one() {
    let (_policy, state, admin_id) = arrange("invite_only").await;
    let (id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_instance_invitation(&mut conn, admin_id, 1, None, false)
    };

    let mut handles = Vec::new();
    for _ in 0..2 {
        let state = state.clone();
        let code = code.clone();
        let suffix = Uuid::now_v7().simple().to_string()[20..].to_string();
        let email = format!("{suffix}@example.test");
        handles.push(tokio::spawn(async move {
            let (status, body) = sign_up(
                &state,
                &code,
                format!("u{suffix}"),
                email.clone(),
                "a-long-enough-password",
            )
            .await;
            (status, body.0.status, email)
        }));
    }

    let mut created = 0;
    for h in handles {
        let (status, code, email) = h.await.unwrap();
        if status == axum::http::StatusCode::CREATED {
            created += 1;
        } else {
            assert_eq!(status, axum::http::StatusCode::CONFLICT);
            assert_eq!(
                code, "invitation_unusable",
                "the loser is refused as an exhausted invitation is"
            );
            assert!(
                user_with_email(&state, &email).is_none(),
                "a refused signup must leave no account behind"
            );
        }
    }
    assert_eq!(created, 1, "two signups for the last use admit exactly one");
    assert_eq!(
        used_count(&state, id),
        1,
        "the count must never exceed the cap"
    );
    assert_eq!(redemptions_of(&state, id).len(), 1);
}

/// SC-006 at a larger cap: N uses, N + 2 racing signups, exactly N accounts.
#[tokio::test]
async fn an_n_use_invitation_admits_exactly_n_under_concurrency() {
    let (_policy, state, admin_id) = arrange("invite_only").await;
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
        let suffix = Uuid::now_v7().simple().to_string()[20..].to_string();
        handles.push(tokio::spawn(async move {
            sign_up(
                &state,
                &code,
                format!("u{suffix}"),
                format!("{suffix}@example.test"),
                "a-long-enough-password",
            )
            .await
            .0
        }));
    }

    let mut admitted = 0;
    for h in handles {
        if h.await.unwrap() == axum::http::StatusCode::CREATED {
            admitted += 1;
        }
    }
    assert_eq!(
        admitted, N,
        "{attempts} concurrent signups through a {N}-use invitation must admit exactly {N}"
    );
    assert_eq!(
        used_count(&state, id),
        N,
        "the stored count must never exceed the cap"
    );
}

/// Inserts an enabled provider nobody else uses, so the OAuth tests own it.
fn insert_test_provider(state: &crate::state::AppState) -> String {
    let key = format!("test-{}", &Uuid::now_v7().simple().to_string()[20..]);
    let now = chrono::Utc::now().naive_utc();
    let mut conn = state.db_pool.get().unwrap();
    diesel::insert_into(crate::schema::oauth_providers::table)
        .values(&crate::models::NewOAuthProvider {
            id: Uuid::now_v7(),
            provider_key: key.clone(),
            display_name: "Test".to_string(),
            authorization_url: "https://idp.example.invalid/authorize".to_string(),
            token_url: "https://idp.example.invalid/token".to_string(),
            userinfo_url: None,
            issuer_url: None,
            scopes: vec![Some("openid".to_string())],
            oauth_client_id: Some("client".to_string()),
            oauth_client_secret: Some("secret".to_string()),
            configured: true,
            enabled: true,
            created_at: now,
            updated_at: now,
            config_source: "test".to_string(),
        })
        .execute(&mut conn)
        .unwrap();
    key
}

/// The OAuth path: two first-time identities racing for the last use. One
/// account is provisioned and linked; the other is refused, and leaves no
/// orphan account behind — the user insert, the identity link and the use are
/// one transaction.
#[tokio::test]
async fn oauth_signups_racing_for_the_last_use_admit_exactly_one() {
    let (_policy, state, admin_id) = arrange("invite_only").await;
    let provider_key = insert_test_provider(&state);
    let (id, code) = {
        let mut conn = state.db_pool.get().unwrap();
        insert_test_instance_invitation(&mut conn, admin_id, 1, None, false)
    };

    let mut handles = Vec::new();
    for _ in 0..2 {
        let state = state.clone();
        let code = code.clone();
        let provider_key = provider_key.clone();
        let suffix = Uuid::now_v7().simple().to_string()[20..].to_string();
        let email = format!("{suffix}@example.test");
        handles.push(tokio::spawn(async move {
            let (status, _) = super::super::oauth::resolve_oauth_login(
                state.clone(),
                tower_cookies::Cookies::default(),
                super::super::ClientDescription::unknown(),
                super::super::OAuthResolveRequest {
                    invitation_code: Some(code),
                    sign_in_only: false,
                    provider_key,
                    provider_user_id: suffix.clone(),
                    provider_email: Some(email.clone()),
                    access_token: None,
                    refresh_token: None,
                    token_expires_at: None,
                },
            )
            .await;
            (status, email)
        }));
    }

    let mut provisioned = 0;
    for h in handles {
        let (status, email) = h.await.unwrap();
        if status == axum::http::StatusCode::CONFLICT {
            assert!(
                user_with_email(&state, &email).is_none(),
                "a refused OAuth signup must leave no orphan account"
            );
        } else {
            assert!(
                user_with_email(&state, &email).is_some(),
                "an admitted OAuth signup ({status}) must have made the account"
            );
            provisioned += 1;
        }
    }
    assert_eq!(
        provisioned, 1,
        "two identities for the last use admit exactly one"
    );
    assert_eq!(used_count(&state, id), 1);
    assert_eq!(redemptions_of(&state, id).len(), 1);
}

/// FR-004 and FR-012: both event kinds are written, and neither carries an
/// address. The struct makes that structural, and this asserts it stays so.
#[tokio::test]
async fn access_events_record_the_act_and_never_the_person() {
    let (_policy, state, admin_id) = arrange("closed").await;

    // The events table is global, and this test asserts on the rows *it*
    // wrote. `POLICY_LOCK` stops another test writing beside it, but
    // earlier runs against the same database have left their own rows
    // behind — so the ids already present are subtracted afterwards
    // rather than trusting that the newest few are ours. A set difference
    // is used in preference to a timestamp or id boundary because both
    // order ambiguously for anything written in the same millisecond.
    let existing: std::collections::HashSet<Uuid> = {
        let mut conn = state.db_pool.get().unwrap();
        crate::schema::instance_access_events::table
            .select(crate::schema::instance_access_events::id)
            .load::<Uuid>(&mut conn)
            .unwrap()
            .into_iter()
            .collect()
    };

    record_refusal(&state, &AdmissionRoute::OAuth("google".to_string())).await;
    crate::admin::update_instance_access_policy(
        &state,
        Some(admin_id),
        InstanceAccessPolicy::InviteOnly,
    )
    .await
    .expect("an administrator may change the policy");

    let mut conn = state.db_pool.get().unwrap();
    let rows = crate::schema::instance_access_events::table
        .order(crate::schema::instance_access_events::occurred_at.desc())
        .select(crate::models::InstanceAccessEvent::as_select())
        .load::<crate::models::InstanceAccessEvent>(&mut conn)
        .unwrap()
        .into_iter()
        .filter(|r| !existing.contains(&r.id))
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        2,
        "this test writes exactly one refusal and one policy change"
    );

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

fn world_link_resolve(
    provider_key: &str,
    subject: &str,
    email: &str,
    invitation_code: Option<String>,
    sign_in_only: bool,
) -> super::super::OAuthResolveRequest {
    super::super::OAuthResolveRequest {
        invitation_code,
        sign_in_only,
        provider_key: provider_key.to_string(),
        provider_user_id: subject.to_string(),
        provider_email: Some(email.to_string()),
        access_token: None,
        refresh_token: None,
        token_expires_at: None,
    }
}

/// Spec 088 (FR-010, SC-002): a provider sign-in that began on a join page
/// never creates an account, in any access mode, and holding an instance
/// invitation does not change that. An identity that already has an account
/// still signs in.
#[tokio::test]
async fn a_world_link_sign_in_never_creates_an_account_in_any_mode() {
    use super::super::oauth::resolve_oauth_login;
    use super::super::world_link_sign_in::WORLD_LINK_OAUTH_MESSAGE;
    use axum::http::StatusCode;

    for policy in ["open", "invite_only", "closed"] {
        let (_policy, state, admin_id) = arrange("open").await;
        let provider_key = insert_test_provider(&state);

        // Someone who already has an account, made while the door was open.
        let known = Uuid::now_v7().simple().to_string()[20..].to_string();
        let known_email = format!("{known}@example.test");
        let (status, _) = resolve_oauth_login(
            state.clone(),
            tower_cookies::Cookies::default(),
            super::super::ClientDescription::unknown(),
            world_link_resolve(&provider_key, &known, &known_email, None, false),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "the existing account is made");

        let invitation = {
            let mut conn = state.db_pool.get().unwrap();
            set_instance_access_policy(&mut conn, policy);
            insert_test_instance_invitation(&mut conn, admin_id, 5, None, false)
        };

        let stranger = Uuid::now_v7().simple().to_string()[20..].to_string();
        let stranger_email = format!("{stranger}@example.test");
        let (status, axum::Json(body)) = resolve_oauth_login(
            state.clone(),
            tower_cookies::Cookies::default(),
            super::super::ClientDescription::unknown(),
            world_link_resolve(
                &provider_key,
                &stranger,
                &stranger_email,
                Some(invitation.1.clone()),
                true,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{policy}: refused");
        assert_eq!(body.status, "world_link_sign_in_only");
        assert_eq!(body.message, WORLD_LINK_OAUTH_MESSAGE);
        assert!(
            user_with_email(&state, &stranger_email).is_none(),
            "{policy}: no account is made"
        );
        assert_eq!(
            used_count(&state, invitation.0),
            0,
            "{policy}: no use burned"
        );

        let (status, _) = resolve_oauth_login(
            state.clone(),
            tower_cookies::Cookies::default(),
            super::super::ClientDescription::unknown(),
            world_link_resolve(&provider_key, &known, &known_email, None, true),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::OK,
            "{policy}: an existing account still signs in from a link"
        );
    }
}
