//! Spec 041 US5 (FR-019, FR-022): what a sign-in does about an account the
//! rule requires and that has not enrolled.
//!
//! # The lockout this is the fence around
//!
//! Turning the instance-wide requirement on used to refuse every account that
//! had not already enrolled — permanently, and with no way to fix it from the
//! screen where they were refused. The expression that decided
//! (`global || admin_required || enabled`) produced a *verification* challenge
//! for an account with nothing to verify, and `verify_two_factor_for_user`
//! answers `false` for an account with no stored secret, so the only answer
//! that challenge could ever get was "no".
//!
//! FR-019 says the answer is enrolment, never a refusal. These drive the login
//! handler to prove it, because the arithmetic in `requirement.rs` being right
//! and the handler acting on it are two different claims.

use diesel::prelude::*;
use tower_cookies::Cookies;
use uuid::Uuid;

use crate::auth::client_hint::ClientDescription;
use crate::auth::sessions::authenticate_password_login;
use crate::schema::users;
use crate::state::AppState;
use crate::test_support::{insert_test_user, test_app_state};

const PASSWORD: &str = "correct horse battery staple";

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

/// FR-019, through the per-account requirement rather than the instance-wide
/// switch.
///
/// Deliberately not the global setting: it is a single row shared by every
/// test in the binary, and flipping it makes every other test that reads it
/// non-deterministic. The per-account flag is the same term of the same
/// expression and reaches the same branch.
#[tokio::test]
async fn a_required_account_that_has_not_enrolled_is_sent_to_enrol_not_refused() {
    let state = test_app_state();
    let (user_id, username) = user_with_a_password(&state);

    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::two_factor_admin_required.eq(true))
        .execute(&mut conn)
        .expect("require");
    drop(conn);

    let (status, body) = authenticate_password_login(
        &state,
        &Cookies::default(),
        &username,
        PASSWORD,
        None,
        ClientDescription::unknown(),
    )
    .await;

    assert_eq!(
        body.0.status, "two_factor_enrolment_required",
        "a required account that has never enrolled must be taken through \
         enrolment, not told no",
    );
    assert_eq!(status, axum::http::StatusCode::UNAUTHORIZED);
    assert!(
        body.0.login_two_factor_challenge_id.is_some(),
        "and it must be handed the ticket that authorises the enrolment, or \
         the instruction is one it cannot act on",
    );
    assert!(
        body.0.session.is_none(),
        "the sign-in is not finished until the factor is",
    );
}

/// FR-027 at the login handler, which is where an upgraded instance meets it
/// (FR-031).
///
/// An administrator who has never enrolled is required by the role alone —
/// no switch on, nothing written to their row — and the first thing that
/// happens to them is an enrolment, not a refusal and not an ordinary
/// sign-in. This is the assertion that would have failed until the `is_admin`
/// term reached `second_factor_required`.
#[tokio::test]
async fn an_administrator_who_has_never_enrolled_is_taken_through_enrolment() {
    let state = test_app_state();
    let (user_id, username) = user_with_a_password(&state);

    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::is_admin.eq(true))
        .execute(&mut conn)
        .expect("promote");
    drop(conn);

    let (_, body) = authenticate_password_login(
        &state,
        &Cookies::default(),
        &username,
        PASSWORD,
        None,
        ClientDescription::unknown(),
    )
    .await;

    assert_eq!(
        body.0.status, "two_factor_enrolment_required",
        "promoting an account to administrator must require a factor at its \
         next sign-in, with no migration and nothing written to its row",
    );
}

/// FR-032, the other direction: taking the role away writes nothing, and the
/// account signs in with a password alone again.
///
/// The rule is computed, so giving up the role is the whole of giving up the
/// requirement — there is no second column to remember to clear.
#[tokio::test]
async fn taking_the_role_away_leaves_an_ordinary_account_behind() {
    let state = test_app_state();
    let (user_id, username) = user_with_a_password(&state);

    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::is_admin.eq(true))
        .execute(&mut conn)
        .expect("promote");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::is_admin.eq(false))
        .execute(&mut conn)
        .expect("demote");

    let (enabled, required): (bool, bool) = users::table
        .filter(users::id.eq(user_id))
        .select((users::two_factor_enabled, users::two_factor_admin_required))
        .first(&mut conn)
        .expect("read");
    assert!(
        !enabled && !required,
        "granting and removing the role must write no two-factor column",
    );
    drop(conn);

    let (status, body) = authenticate_password_login(
        &state,
        &Cookies::default(),
        &username,
        PASSWORD,
        None,
        ClientDescription::unknown(),
    )
    .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{}", body.0.message);
    assert!(
        body.0.session.is_some(),
        "with the role gone, nothing requires a factor and the sign-in finishes",
    );
}
