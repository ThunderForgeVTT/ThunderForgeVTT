//! FR-001b and FR-015: the notice goes out, and an instance that cannot send
//! one is a normal instance.

use super::*;
use crate::test_support::{insert_test_user, test_app_state};

/// SC-010, and the reason this is a seam at all: a fresh instance has no mail
/// configured, and every one of these acts still has to complete.
///
/// `tell` returns nothing, so "it did not fail the caller" is the assertion —
/// and it is a real one, because the path underneath it reaches the outbox,
/// which resolves the settings and finds no transport.
#[tokio::test]
async fn telling_somebody_on_an_instance_with_no_mail_is_not_a_failure() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user_id = insert_test_user(&mut conn);
    drop(conn);

    // Would panic or hang if the seam consulted a transport it must not need.
    tell(&state, user_id, Change::ResetByOperator).await;
    tell(&state, user_id, Change::Removed).await;
    tell(&state, user_id, Change::RecoveryCodesReissued).await;
}

/// An account that is not there is not a reason to fail either. An operator
/// reset races a deletion about as often as never, but "never" is not a thing
/// this function is allowed to assume about a call made after a commit.
#[tokio::test]
async fn telling_nobody_is_quiet_rather_than_loud() {
    let state = test_app_state();
    tell(&state, uuid::Uuid::now_v7(), Change::Removed).await;
}

/// What the messages may say, asserted rather than trusted to review.
///
/// A security notice that quoted a code, a secret or a count would be a
/// credential in an inbox — and the operator's name in a reset notice turns a
/// notice into something to forward.
#[tokio::test]
async fn no_message_carries_a_credential_or_names_an_operator() {
    for change in [
        Change::Removed,
        Change::ResetByOperator,
        Change::RecoveryCodesReissued,
    ] {
        let text = format!("{} {}", change.subject(), change.body()).to_lowercase();
        for forbidden in ["code:", "secret", "otpauth", "remaining", "administrator "] {
            if forbidden == "administrator " && change == Change::ResetByOperator {
                // "An administrator ... reset" is the fact. What must not
                // appear is a *name*, and there is nowhere for one to come
                // from: `tell` is given a user id and a variant, never an
                // actor.
                continue;
            }
            assert!(
                !text.contains(forbidden),
                "`{change:?}` says `{forbidden}`:\n{text}",
            );
        }
        assert!(
            text.contains("if this was not you") || text.contains("if you did not ask"),
            "`{change:?}` must tell somebody what to do if it was not them",
        );
    }
}
