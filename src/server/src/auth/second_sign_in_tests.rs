//! Spec 036 T015: what removing the login-time eviction did *not* change.
//!
//! ADR-073 took a statement out of session creation — the one that revoked
//! every live session on each sign-in. The risk in a removal like that is
//! never the thing removed; it is a second behaviour that was leaning on it.
//! The one worth pinning is the second factor: a sign-in that is challenged
//! the first time must be challenged the second time too, and it must be
//! challenged *before* a session exists, not after. If the eviction had been
//! load-bearing there, the second sign-in would sail past the challenge — and
//! the spec's own Edge Cases section asks for exactly this case.
//!
//! `oauth.rs` and `admin_setup.rs` are the other two callers T015 names.
//! Neither branches on how many sessions the account holds — they call
//! `issue_session_cookie` and nothing else — so there is nothing there for a
//! test to distinguish; the eviction bound is asserted where it lives, in
//! `sessions.rs`'s `the_session_bound_ends_the_least_recently_used`.

use chrono::Utc;
use diesel::prelude::*;
use tower_cookies::Cookies;
use uuid::Uuid;

use super::*;
use crate::auth::sessions::{authenticate_password_login, hash_password};
use crate::schema::{user_sessions, users};
use crate::test_support::{insert_test_user, test_app_state};

const PASSWORD: &str = "correct horse battery staple";

/// A user who can actually sign in, with a live second factor.
fn signable_user(state: &AppState) -> (Uuid, String) {
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    let username: String = users::table
        .filter(users::id.eq(user_id))
        .select(users::username)
        .first(&mut conn)
        .expect("the user exists");

    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::password_hash.eq(hash_password(PASSWORD).expect("hashed")),
            users::two_factor_enabled.eq(true),
            users::two_factor_secret_encrypted.eq(Some("ciphertext".to_string())),
            users::two_factor_confirmed_at.eq(Some(Utc::now().naive_utc())),
        ))
        .execute(&mut conn)
        .expect("enrolled");

    (user_id, username)
}

fn live_session_count(state: &AppState, user_id: Uuid) -> i64 {
    let mut conn = state.db_pool.get().expect("conn");
    user_sessions::table
        .filter(user_sessions::user_id.eq(user_id))
        .filter(user_sessions::revoked_at.is_null())
        .count()
        .get_result(&mut conn)
        .expect("count sessions")
}

/// The Edge Case spec 036 names: a second sign-in must still be challenged.
#[tokio::test]
async fn a_second_sign_in_is_still_challenged_for_a_second_factor() {
    let state = test_app_state();
    let (user_id, username) = signable_user(&state);

    for attempt in ["the first", "the second"] {
        let (status, response) = authenticate_password_login(
            &state,
            &Cookies::default(),
            &username,
            PASSWORD,
            None,
            ClientDescription::unknown(),
        )
        .await;

        assert_eq!(
            status,
            axum::http::StatusCode::UNAUTHORIZED,
            "{attempt} sign-in must stop at the second factor",
        );
        assert_eq!(response.status, "two_factor_required", "{attempt} sign-in");
        assert!(
            response.login_two_factor_challenge_id.is_some(),
            "{attempt} sign-in must hand back a challenge to answer",
        );
        assert!(
            response.session.is_none(),
            "{attempt} sign-in must not return a session before the factor is met",
        );
    }

    // And the important half: two challenged sign-ins created no sessions at
    // all. A challenge that issued a session and then asked for a code would
    // be a second factor in appearance only.
    assert_eq!(
        live_session_count(&state, user_id),
        0,
        "a challenged sign-in must not leave a live session behind",
    );
}

/// The other direction, so the test above cannot pass by the account being
/// unable to sign in at all: with no factor enrolled, two sign-ins produce
/// two live sessions rather than one replacing the other (FR-001).
#[tokio::test]
async fn without_a_second_factor_two_sign_ins_leave_two_sessions() {
    let state = test_app_state();
    let (user_id, username) = signable_user(&state);
    {
        let mut conn = state.db_pool.get().expect("conn");
        diesel::update(users::table.filter(users::id.eq(user_id)))
            .set((
                users::two_factor_enabled.eq(false),
                users::two_factor_secret_encrypted.eq(None::<String>),
                users::two_factor_confirmed_at.eq(None::<chrono::NaiveDateTime>),
            ))
            .execute(&mut conn)
            .expect("factor removed");
    }

    for _ in 0..2 {
        let (status, _) = authenticate_password_login(
            &state,
            &Cookies::default(),
            &username,
            PASSWORD,
            None,
            ClientDescription::unknown(),
        )
        .await;
        assert_eq!(status, axum::http::StatusCode::OK, "the sign-in succeeds");
    }

    assert_eq!(
        live_session_count(&state, user_id),
        2,
        "signing in twice must leave both sessions live — this is the \
         eviction ADR-073 removed",
    );
}
