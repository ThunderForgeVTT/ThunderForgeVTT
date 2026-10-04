//! Spec 041 US1 (FR-001c, FR-004) and US4 (FR-013): what an enrolment in
//! progress may and may not touch.
//!
//! # The bug these are the fence around
//!
//! `setup/start` used to write the new secret **over** the live one and set
//! `two_factor_enabled = false` in the same statement, on a password alone.
//! Beginning an enrolment was therefore a way to remove a confirmed second
//! factor without ever proving possession of it, and an enrolment somebody
//! merely abandoned left the account weaker than it started. ADR-081 says a
//! confirmed factor is replaced, never disarmed; these assert it against the
//! database rather than against the intention.

use uuid::Uuid;

use super::*;
use crate::auth::client_hint::ClientDescription;
use crate::auth::types::{TwoFactorSetupConfirmRequest, TwoFactorSetupStartRequest};
use crate::schema::users;
use crate::test_support::{insert_test_user, test_app_state};

const PASSWORD: &str = "correct horse battery staple";

/// A user with a password that actually verifies, because `insert_test_user`
/// stores a placeholder and every entrance here goes through a real Argon2
/// check.
fn user_with_a_password(state: &AppState) -> (Uuid, String) {
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    let hash = thunderforge_axum_auth_core::hashing::hash(PASSWORD).expect("hash");
    let username: String = diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::password_hash.eq(hash))
        .returning(users::username)
        .get_result(&mut conn)
        .expect("set password");
    (user_id, username)
}

/// Every column on `users` that says anything about a second factor.
type FactorColumns = (
    bool,
    Option<String>,
    Option<chrono::NaiveDateTime>,
    Option<String>,
);

fn factor_columns(state: &AppState, user_id: Uuid) -> FactorColumns {
    let mut conn = state.db_pool.get().expect("conn");
    users::table
        .filter(users::id.eq(user_id))
        .select((
            users::two_factor_enabled,
            users::two_factor_secret_encrypted,
            users::two_factor_confirmed_at,
            users::two_factor_pending_secret_encrypted,
        ))
        .first(&mut conn)
        .expect("read the account")
}

fn enrol(state: &AppState, user_id: Uuid) {
    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::two_factor_enabled.eq(true),
            users::two_factor_secret_encrypted.eq(Some("the-live-secret".to_string())),
            users::two_factor_confirmed_at.eq(Some(chrono::Utc::now().naive_utc())),
        ))
        .execute(&mut conn)
        .expect("enrol");
}

async fn start(state: &AppState, username: &str) -> StatusCode {
    two_factor_setup_start(
        State(state.clone()),
        Json(TwoFactorSetupStartRequest {
            username: Some(username.to_string()),
            password: Some(PASSWORD.to_string()),
            challenge_id: None,
        }),
    )
    .await
    .0
}

async fn confirm(state: &AppState, username: &str, code: &str) -> StatusCode {
    two_factor_setup_confirm(
        Cookies::default(),
        ClientDescription::unknown(),
        State(state.clone()),
        Json(TwoFactorSetupConfirmRequest {
            username: Some(username.to_string()),
            password: Some(PASSWORD.to_string()),
            challenge_id: None,
            code: code.to_string(),
        }),
    )
    .await
    .0
}

/// FR-004 and FR-013 together, and the assertion the whole of ADR-081 is
/// about: starting an enrolment writes the **pending** columns and nothing
/// else.
///
/// Stated as "the live columns are byte-for-byte what they were", not as
/// "`two_factor_enabled` is still true", because the failure that happened
/// cleared the secret as well — a check on the flag alone would have passed
/// while the factor it names had already been destroyed.
#[tokio::test]
async fn starting_an_enrolment_writes_no_live_column() {
    let state = test_app_state();
    let (user_id, username) = user_with_a_password(&state);
    enrol(&state, user_id);

    let (enabled_before, secret_before, confirmed_before, _) = factor_columns(&state, user_id);

    assert_eq!(start(&state, &username).await, StatusCode::OK);

    let (enabled, secret, confirmed, pending) = factor_columns(&state, user_id);
    assert_eq!(
        (enabled, secret, confirmed),
        (enabled_before, secret_before, confirmed_before),
        "starting an enrolment must leave every live column exactly as it was",
    );
    assert!(
        pending.is_some(),
        "the new secret must be written, and written to the pending column",
    );
}

/// FR-013. The abandoned case, which is the one that actually happened to
/// people: somebody opens the enrolment screen, does not finish, and closes
/// the tab.
///
/// This is `apps/web/e2e/two-factor.spec.ts`'s `test.fail()` case at the unit
/// level (T036). Nothing here confirms anything, which is the point — the
/// factor must survive an enrolment that simply stops.
#[tokio::test]
async fn an_abandoned_enrolment_leaves_the_confirmed_factor_in_force() {
    let state = test_app_state();
    let (user_id, username) = user_with_a_password(&state);
    enrol(&state, user_id);

    assert_eq!(start(&state, &username).await, StatusCode::OK);
    // …and then nothing. No confirm, no code, no second request.

    let (enabled, secret, confirmed, _) = factor_columns(&state, user_id);
    assert!(
        enabled,
        "an enrolment nobody finished must not turn the factor off"
    );
    assert_eq!(
        secret,
        Some("the-live-secret".to_string()),
        "the live secret must still be the one the person's authenticator holds",
    );
    assert!(confirmed.is_some(), "the factor is still a confirmed one");
}

/// FR-001c. A mistyped code is the ordinary case, not an attack, and it must
/// cost the person nothing but the retry.
///
/// The pending secret survives, so the QR code already scanned into their
/// authenticator is still the right one. A confirmation that cleared it would
/// send somebody back to the beginning for one wrong digit — and, worse, would
/// leave the authenticator holding a secret the server had thrown away.
#[tokio::test]
async fn a_wrong_confirmation_code_leaves_the_pending_enrolment_retryable() {
    let state = test_app_state();
    let (user_id, username) = user_with_a_password(&state);

    assert_eq!(start(&state, &username).await, StatusCode::OK);
    let (_, _, _, pending_before) = factor_columns(&state, user_id);
    assert!(pending_before.is_some(), "the enrolment started");

    let refused = confirm(&state, &username, "000000").await;
    assert_ne!(
        refused,
        StatusCode::OK,
        "a wrong code must not confirm an enrolment"
    );

    let (enabled, _, _, pending_after) = factor_columns(&state, user_id);
    assert!(
        !enabled,
        "a wrong code must not enable the factor it failed to prove"
    );
    assert_eq!(
        pending_after, pending_before,
        "the pending secret must survive a wrong code, or the person has to rescan",
    );
}

/// `contracts/enrolment.md`: exactly one proof, never two and never none.
///
/// Refused before either is evaluated, for the same reason `two_factor_verify`
/// refuses a request carrying two proofs — one request is one attempt at one
/// thing, and a request offering both is a client that does not know which of
/// them it believes.
#[tokio::test]
async fn enrolment_refuses_a_request_carrying_both_proofs_or_neither() {
    let state = test_app_state();
    let (_, username) = user_with_a_password(&state);

    let neither = authorise_enrolment(&state, None, None, None).await;
    assert!(
        neither.is_err(),
        "a request proving nothing must not authorise an enrolment"
    );
    assert_eq!(
        neither.err().map(|e| e.code),
        Some(StatusCode::BAD_REQUEST),
        "and it is the request that is wrong, not the credentials",
    );

    let both = authorise_enrolment(
        &state,
        Some(&username),
        Some(PASSWORD),
        Some(uuid::Uuid::now_v7()),
    )
    .await;
    assert!(
        both.is_err(),
        "a correct password must not rescue a request that also carries a ticket",
    );
    assert_eq!(
        both.err().map(|e| e.code),
        Some(StatusCode::BAD_REQUEST),
        "refused for its shape, before either proof is looked at",
    );
}
