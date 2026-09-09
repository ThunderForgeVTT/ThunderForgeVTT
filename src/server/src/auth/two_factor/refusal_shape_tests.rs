//! Spec 041 FR-018, `contracts/verification.md` § Refusal shapes: **one
//! message and one status** for every refusal that is about a credential.
//!
//! # What a second message would give away
//!
//! Each of the alternatives is a fact about somebody else's account, handed to
//! a caller who has not proved they hold it:
//!
//! - "this account has no recovery codes" says which credential is worth
//!   spending the remaining attempts on;
//! - "that code was correct but already used" confirms an intercepted code was
//!   genuine, which is exactly what an attacker replaying one wants to know;
//! - "this account has no second factor" answers, for any username, whether
//!   the password alone is enough.
//!
//! So they are the same refusal, and this drives the handler rather than
//! reading the source, because a constant used in four places and a *response*
//! that differs in a fifth are not the same guarantee.

use uuid::Uuid;

use super::verification::CREDENTIAL_REFUSED;
use super::*;
use crate::auth::client_hint::ClientDescription;
use crate::auth::types::TwoFactorVerifyRequest;
use crate::schema::{login_two_factor_challenges, users};
use crate::test_support::{insert_test_user, test_app_state};

/// A live challenge for `user_id`, as the login path would have minted it.
async fn challenge_for(state: &AppState, user_id: Uuid) -> Uuid {
    create_login_two_factor_challenge(state, user_id)
        .await
        .expect("a challenge")
}

async fn verify(
    state: &AppState,
    challenge_id: Uuid,
    code: Option<&str>,
    recovery_code: Option<&str>,
) -> (StatusCode, String, String) {
    let (status, body) = two_factor_verify(
        Cookies::default(),
        ClientDescription::unknown(),
        State(state.clone()),
        Json(TwoFactorVerifyRequest {
            challenge_id,
            code: code.map(str::to_string),
            recovery_code: recovery_code.map(str::to_string),
        }),
    )
    .await;
    (status, body.0.status.to_string(), body.0.message.clone())
}

/// The seven rows of the contract's list, as far as they can be reached
/// through one handler, asserted to be indistinguishable.
///
/// Every one of these is a different *cause* inside the server and must be the
/// same *answer* outside it — including the recovery-code cases, which is
/// where the disclosure would actually hurt: an account with no codes and an
/// account with a wrong code must not be tellable apart.
#[tokio::test]
async fn every_credential_refusal_is_the_same_refusal() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    // Holds no second factor at all, and no recovery codes: two of the rows
    // for free, and the state a probe against an arbitrary username finds.
    let bare = insert_test_user(&mut conn);
    let enrolled = insert_test_user(&mut conn);
    // A *real* encrypted secret, not a placeholder. A placeholder fails to
    // decrypt, which is a 500 rather than a refusal — and this test would then
    // have been asserting that an internal error looks like a wrong code,
    // which is not the guarantee and is not even true.
    let key = crate::crypto::encryption_key_from_config_secret(&state.config.secret)
        .expect("an encryption key");
    // 32 base32 characters, so 20 bytes: `totp-rs` refuses a secret shorter
    // than RFC 6238 asks for, and a refusal there is an error rather than a
    // wrong code.
    let secret =
        crate::crypto::encrypt_secret("JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP", &key).expect("a secret");
    diesel::update(users::table.filter(users::id.eq(enrolled)))
        .set((
            users::two_factor_enabled.eq(true),
            users::two_factor_secret_encrypted.eq(Some(secret)),
        ))
        .execute(&mut conn)
        .expect("enrol");
    drop(conn);

    let mut answers = Vec::new();
    for (label, user, code, recovery) in [
        ("no second factor at all", bare, Some("000000"), None),
        ("no recovery codes at all", bare, None, Some("AAAA-BBBB")),
        ("a wrong authenticator code", enrolled, Some("000000"), None),
        ("a wrong recovery code", enrolled, None, Some("AAAA-BBBB")),
        ("neither field supplied", enrolled, None, None),
        (
            "both fields supplied",
            enrolled,
            Some("000000"),
            Some("AAAA-BBBB"),
        ),
    ] {
        let challenge = challenge_for(&state, user).await;
        let answer = verify(&state, challenge, code, recovery).await;
        assert_eq!(
            answer.2, CREDENTIAL_REFUSED,
            "`{label}` must be refused with the one credential message",
        );
        answers.push((label, answer));
    }

    let first = &answers[0].1;
    for (label, answer) in &answers[1..] {
        assert_eq!(
            answer, first,
            "`{label}` is distinguishable from `{}` — status, code or message \
             differs, and any difference is a fact about somebody else's \
             account",
            answers[0].0,
        );
    }
    assert_eq!(
        first.0,
        StatusCode::UNAUTHORIZED,
        "a credential refusal is a 401, whatever went wrong behind it",
    );
}

/// The one refusal that is deliberately *not* the same, and why that is safe.
///
/// A challenge that is unknown, expired or consumed is a fact about a request
/// rather than about an account, and the person has to do something different
/// about it — sign in again — which no amount of retyping a code will reach.
/// Giving them the credential message would send them round a loop with no
/// exit.
#[tokio::test]
async fn an_expired_sign_in_is_told_apart_because_the_person_must_act_differently() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let user = insert_test_user(&mut conn);
    drop(conn);

    let unknown = verify(&state, Uuid::now_v7(), Some("000000"), None).await;
    assert_eq!(unknown.0, StatusCode::BAD_REQUEST);
    assert_ne!(unknown.2, CREDENTIAL_REFUSED);

    let live = challenge_for(&state, user).await;
    let mut conn = state.db_pool.get().expect("conn");
    diesel::update(
        login_two_factor_challenges::table.filter(login_two_factor_challenges::id.eq(live)),
    )
    .set(login_two_factor_challenges::consumed_at.eq(Some(chrono::Utc::now().naive_utc())))
    .execute(&mut conn)
    .expect("consume");
    drop(conn);

    let consumed = verify(&state, live, Some("000000"), None).await;
    assert_eq!(
        consumed, unknown,
        "unknown, expired and consumed are one answer between them — which of \
         the three it was is not the caller's business",
    );
}
