//! Claim construction and RS256 signing, tested without an application.
//!
//! The key here is a throwaway generated for this suite and registered
//! nowhere; it exists only so the signature can be verified against its own
//! public half. Nothing in this file reaches the network, and nothing needs a
//! GitHub App to exist — which is the entire argument for the pure/effects
//! split (research R5a). Rules that can only be exercised against a live App
//! are rules that are hoped for rather than tested.
//!
//! What is actually being pinned down: the validity window. The assertion is
//! backdated against clock skew, its span never exceeds the host's ceiling,
//! and the arithmetic refuses rather than wraps at the top of the range. All
//! three are properties of a function of `now`, which is why `now` is a
//! parameter and not a call to the system clock.

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use proptest::prelude::*;
use thunderforge_repo_host::jwt::{
    AppJwtClaims, CLOCK_SKEW_BACKDATE_SECS, DEFAULT_JWT_LIFETIME_SECS, MAX_JWT_LIFETIME_SECS,
    build_claims, encoding_key_from_pem, sign_app_jwt, sign_claims,
};
use thunderforge_repo_host::{RepoHost, RepoHostError, github::GitHubApp};

const TEST_KEY: &[u8] = include_bytes!("fixtures/throwaway-test-app-key.pem");
const TEST_PUBLIC_KEY: &[u8] = include_bytes!("fixtures/throwaway-test-app-key.pub.pem");

/// Verify a signed assertion against the fixture's public half.
///
/// `exp` validation is switched off deliberately: these tests sign at chosen
/// instants that are mostly in the past, and the point of the check is that
/// the *signature* is a real RS256 signature over the claims we built — the
/// window is asserted separately, against values rather than against the
/// wall clock.
fn verify(token: &str) -> AppJwtClaims {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = false;
    validation.required_spec_claims.clear();
    decode::<AppJwtClaims>(
        token,
        &DecodingKey::from_rsa_pem(TEST_PUBLIC_KEY).expect("fixture public key parses"),
        &validation,
    )
    .expect("the assertion must verify against the key that signed it")
    .claims
}

#[test]
fn claims_are_backdated_and_bounded() {
    let claims = build_claims("123456", 1_000_000, DEFAULT_JWT_LIFETIME_SECS)
        .expect("the default lifetime is in range");

    assert_eq!(claims.iss, "123456");
    assert_eq!(claims.iat, 1_000_000 - CLOCK_SKEW_BACKDATE_SECS);
    assert_eq!(claims.exp, 1_000_000 + DEFAULT_JWT_LIFETIME_SECS);
    assert!(claims.exp - claims.iat <= MAX_JWT_LIFETIME_SECS);
}

#[test]
fn the_default_lifetime_leaves_headroom_under_the_ceiling() {
    // Sitting exactly at the ceiling would mean any future change to the
    // backdate silently produces assertions the host refuses.
    let span = DEFAULT_JWT_LIFETIME_SECS + CLOCK_SKEW_BACKDATE_SECS;
    assert!(span < MAX_JWT_LIFETIME_SECS, "span {span} has no headroom");
}

#[test]
fn a_clock_near_the_epoch_does_not_underflow() {
    // The backdate saturates rather than wrapping to the far end of `u64`.
    let claims = build_claims("1", 10, 60).expect("in range");
    assert_eq!(claims.iat, 0);
    assert_eq!(claims.exp, 70);
}

#[test]
fn a_lifetime_past_the_ceiling_is_refused_rather_than_clamped() {
    // Quietly using a different number than was asked for is how a
    // configuration mistake survives to production unnoticed.
    let err = build_claims("1", 1_000, MAX_JWT_LIFETIME_SECS).unwrap_err();
    assert!(matches!(
        err,
        RepoHostError::JwtLifetimeOutOfRange {
            limit: MAX_JWT_LIFETIME_SECS,
            ..
        }
    ));
    assert!(matches!(
        build_claims("1", 1_000, 0),
        Err(RepoHostError::JwtLifetimeOutOfRange { .. })
    ));
}

#[test]
fn a_clock_at_the_top_of_the_range_refuses_rather_than_wrapping() {
    assert_eq!(
        build_claims("1", u64::MAX, DEFAULT_JWT_LIFETIME_SECS).unwrap_err(),
        RepoHostError::ClockOutOfRange { now: u64::MAX }
    );
}

#[test]
fn a_blank_application_identifier_is_refused() {
    assert_eq!(
        build_claims("   ", 1_000, 60).unwrap_err(),
        RepoHostError::MissingAppId
    );
}

#[test]
fn a_signed_assertion_verifies_and_says_rs256() {
    let token = sign_app_jwt(TEST_KEY, "123456", 1_700_000_000, DEFAULT_JWT_LIFETIME_SECS)
        .expect("the fixture key signs");

    assert_eq!(
        decode_header(&token).expect("header parses").alg,
        Algorithm::RS256
    );
    let claims = verify(&token);
    assert_eq!(claims.iss, "123456");
    assert_eq!(claims.iat, 1_700_000_000 - CLOCK_SKEW_BACKDATE_SECS);
}

#[test]
fn a_key_that_is_not_a_key_is_refused() {
    assert!(matches!(
        encoding_key_from_pem(b"not a pem file at all"),
        Err(RepoHostError::InvalidPrivateKey(_))
    ));
}

#[test]
fn the_token_exchange_carries_a_verifiable_assertion_and_the_right_url() {
    let app = GitHubApp::new("123456", "thunderforge-test", TEST_KEY).expect("valid registration");
    let (grant, _) = app
        .validate_grant(r#"{"id":99,"repository_selection":"selected","repositories":[{"full_name":"gm/lore","private":true}]}"#)
        .expect("a single-repository grant");

    let exchange = app
        .token_exchange(&grant, 1_700_000_000)
        .expect("the exchange is built from a valid registration");

    assert_eq!(
        exchange.url,
        "https://api.github.com/app/installations/99/access_tokens"
    );
    assert_eq!(verify(&exchange.assertion).iss, "123456");

    // FR-035 again: the assertion must not be reachable through `Debug`.
    let rendered = format!("{exchange:?}");
    assert!(!rendered.contains(&exchange.assertion));
    assert!(rendered.contains("<redacted>"));
}

proptest! {
    /// The window invariants hold for every clock value and every lifetime in
    /// range: the assertion is backdated, it expires after it is issued, and
    /// its span never exceeds the host's ceiling.
    #[test]
    fn the_validity_window_holds_at_every_clock_value(
        now in 0u64..(u64::MAX - MAX_JWT_LIFETIME_SECS),
        lifetime in 1u64..(MAX_JWT_LIFETIME_SECS - CLOCK_SKEW_BACKDATE_SECS),
    ) {
        let claims = build_claims("app", now, lifetime)
            .expect("a lifetime inside the ceiling is always buildable");
        prop_assert!(claims.iat <= now);
        prop_assert!(claims.exp > claims.iat);
        prop_assert!(claims.exp - claims.iat <= MAX_JWT_LIFETIME_SECS);
        prop_assert_eq!(claims.exp, now + lifetime);
    }

    /// Claim construction is total: no clock value and no lifetime panics,
    /// which is the property unguarded `u64` addition does not give.
    #[test]
    fn claim_construction_never_panics(now: u64, lifetime: u64, app_id in ".{0,32}") {
        let _ = build_claims(&app_id, now, lifetime);
    }

    /// Whatever the clock says, a built assertion is a real RS256 signature
    /// over the claims that were built — signing is not quietly skipped for
    /// some inputs.
    #[test]
    fn every_buildable_assertion_verifies(
        now in 0u64..4_102_444_800u64,
        app_id in "[0-9]{1,12}",
    ) {
        let key = encoding_key_from_pem(TEST_KEY).expect("fixture key parses");
        let claims = build_claims(&app_id, now, DEFAULT_JWT_LIFETIME_SECS)
            .expect("the default lifetime is always in range");
        let token = sign_claims(&key, &claims).expect("signing is deterministic here");
        prop_assert_eq!(verify(&token), claims);
    }
}
