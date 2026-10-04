//! Spec 041 FR-016: a code is not accepted twice, including within its window.
//!
//! `totp.rs` tests the *rule* — which step a code matched, and whether that
//! step may be spent — over a clock it controls. This tests the **join**: that
//! the rule is actually applied by the path a sign-in takes, against a real
//! row, with a real encrypted secret, through a real conditional UPDATE.
//!
//! That join is the whole defect. Before this, `verify_totp_code` returned a
//! bare `bool` and the step `totp-rs` handed back was dropped one line later,
//! with a comment in `totp.rs` saying exactly what the discarded value was
//! for. Every unit test either side of that seam passed.

use uuid::Uuid;

use super::*;
use crate::crypto::{encrypt_secret, encryption_key_from_config_secret};
use crate::schema::users;
use crate::test_support::{insert_test_user, test_app_state};
use thunderforge_axum_auth_core::totp::{STEP_SECONDS, generate_code_at};

/// A real base32 secret, the length RFC 4226 recommends.
const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

/// An account with a confirmed factor whose secret this test knows, so it can
/// produce codes the server will genuinely accept.
fn enrolled_user(state: &crate::state::AppState) -> (Uuid, String) {
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    let username: String = users::table
        .filter(users::id.eq(user_id))
        .select(users::username)
        .first(&mut conn)
        .expect("the user exists");

    let key = encryption_key_from_config_secret(&state.config.secret).expect("key");
    let encrypted = encrypt_secret(SECRET, &key).expect("encrypted");

    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set((
            users::two_factor_enabled.eq(true),
            users::two_factor_secret_encrypted.eq(Some(encrypted)),
            users::two_factor_confirmed_at.eq(Some(chrono::Utc::now().naive_utc())),
        ))
        .execute(&mut conn)
        .expect("enrolled");

    (user_id, username)
}

fn spent_step(state: &crate::state::AppState, user_id: Uuid) -> Option<i64> {
    let mut conn = state.db_pool.get().expect("conn");
    users::table
        .filter(users::id.eq(user_id))
        .select(users::two_factor_last_used_step)
        .first(&mut conn)
        .expect("read the account")
}

/// The headline: the same code, offered twice, works once.
#[tokio::test]
async fn a_code_that_signed_in_once_is_refused_the_second_time() {
    let state = test_app_state();
    let (user_id, username) = enrolled_user(&state);

    let now = chrono::Utc::now().timestamp() as u64;
    let code = generate_code_at(&username, SECRET, now).expect("a code");

    assert!(
        verify_two_factor_for_user(&state, user_id, &code)
            .await
            .expect("verification runs"),
        "a fresh code must be accepted",
    );
    assert_eq!(
        spent_step(&state, user_id),
        Some((now / STEP_SECONDS) as i64),
        "accepting a code must record the step it spent",
    );

    // The same code, moments later, still inside its own window — which is
    // precisely what makes it dangerous, and precisely what the bare `bool`
    // verifier could not refuse.
    assert!(
        !verify_two_factor_for_user(&state, user_id, &code)
            .await
            .expect("verification runs"),
        "FR-016: a code must not be accepted twice",
    );
}

/// The refusal must not be distinguishable from a wrong code. Telling them
/// apart would confirm to an attacker that an intercepted code was genuine.
#[tokio::test]
async fn a_replayed_code_is_refused_exactly_as_a_wrong_one_is() {
    let state = test_app_state();
    let (user_id, username) = enrolled_user(&state);

    let now = chrono::Utc::now().timestamp() as u64;
    let code = generate_code_at(&username, SECRET, now).expect("a code");
    verify_two_factor_for_user(&state, user_id, &code)
        .await
        .expect("first use succeeds");

    let replayed = verify_two_factor_for_user(&state, user_id, &code).await;
    let nonsense = verify_two_factor_for_user(&state, user_id, "000000").await;

    assert_eq!(
        replayed.expect("runs"),
        nonsense.expect("runs"),
        "both are plain false; neither is an error the caller could tell apart",
    );
}

/// The next step still works. A guard that locked the account out after one
/// code would satisfy "not twice" and be useless.
#[tokio::test]
async fn the_next_code_is_still_accepted() {
    let state = test_app_state();
    let (user_id, username) = enrolled_user(&state);

    let now = chrono::Utc::now().timestamp() as u64;
    let first = generate_code_at(&username, SECRET, now).expect("a code");
    verify_two_factor_for_user(&state, user_id, &first)
        .await
        .expect("first use succeeds");

    // One step on. Generated rather than waited for — the point of an explicit
    // clock is not having to spend 30 seconds proving this.
    let next = generate_code_at(&username, SECRET, now + STEP_SECONDS).expect("a code");
    assert!(
        verify_two_factor_for_user(&state, user_id, &next)
            .await
            .expect("verification runs"),
        "a later step must remain spendable, or the factor is a lockout",
    );
}

/// The backwards case, through the real path. With a skew of one the previous
/// step's code is still cryptographically valid, and it carries a *different*
/// step number — so a guard comparing for inequality would admit it.
#[tokio::test]
async fn the_previous_steps_code_cannot_be_used_after_the_current_one() {
    let state = test_app_state();
    let (user_id, username) = enrolled_user(&state);

    let now = chrono::Utc::now().timestamp() as u64;
    let current = generate_code_at(&username, SECRET, now).expect("a code");
    let previous = generate_code_at(&username, SECRET, now - STEP_SECONDS).expect("a code");

    verify_two_factor_for_user(&state, user_id, &current)
        .await
        .expect("the current code signs in");

    assert!(
        !verify_two_factor_for_user(&state, user_id, &previous)
            .await
            .expect("verification runs"),
        "the previous step is inside the skew window and must be dead once a \
         later step has been spent",
    );
}

/// Concurrency, which is what the conditional UPDATE is for. Two requests
/// carrying the same code is not a hypothetical — it is what a replay looks
/// like when the attacker is quick.
///
/// # Why real threads and the sync function
///
/// The race being tested lives in one SQL statement, so this calls that
/// statement directly from two OS threads rather than going through the async
/// path. The async version — two `verify_two_factor_for_user` futures under
/// `tokio::join!` — asserted the same thing and then **segfaulted the test
/// binary on exit**, after printing `test result: ok`. It did so under both a
/// current-thread and a multi-thread runtime, so it is not the runtime flavour;
/// something in the libpq teardown does not survive that shape. A green result
/// with exit code 101 is the worst failure to leave lying around, and this
/// tests the actual contended statement more directly anyway.
#[test]
fn two_simultaneous_uses_of_one_code_admit_exactly_one() {
    let state = test_app_state();
    let (user_id, username) = enrolled_user(&state);

    let now = chrono::Utc::now().timestamp() as u64;
    let code = generate_code_at(&username, SECRET, now).expect("a code");
    let step = i64::try_from(
        thunderforge_axum_auth_core::totp::matched_step_at(&username, SECRET, &code, now)
            .expect("verified")
            .expect("matches"),
    )
    .expect("in range");

    // A barrier so both threads are inside the statement at the same time.
    // Without it the first would usually finish before the second began, and
    // the test would pass without ever contending.
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let pool = state.db_pool.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            std::thread::spawn(move || {
                let mut conn = pool.get().expect("conn");
                barrier.wait();
                claim_totp_step_sync(&mut conn, user_id, step).expect("the claim runs")
            })
        })
        .collect();

    let admitted = handles
        .into_iter()
        .filter(|_| true)
        .map(|h| h.join().expect("thread"))
        .filter(|ok| *ok)
        .count();

    assert_eq!(
        admitted, 1,
        "exactly one of two racing claims on one step may succeed",
    );
    assert_eq!(
        spent_step(&state, user_id),
        Some(step),
        "and the step is recorded once",
    );
}
