//! PKCE (RFC 7636): proving the client that redeems the code is the client
//! that asked for it.
//!
//! Without this, an attacker who intercepts the authorization code — from a
//! redirect logged by a proxy, or a malicious app registered on the same
//! custom URI scheme — can redeem it themselves. The verifier never leaves
//! our server; only its hash rides in the authorization URL.

use base64::Engine as _;
use sha2::{Digest, Sha256};
use thunderforge_axum_auth_core::constant_time::secure_equals;
use thunderforge_axum_auth_core::random::random_urlsafe;

/// The only challenge method we send. RFC 7636 also defines `plain`, which
/// puts the verifier itself in the redirect URL and therefore protects
/// against nothing; it exists for clients that cannot compute SHA-256, which
/// a server can.
pub const CODE_CHALLENGE_METHOD: &str = "S256";

/// Bytes of entropy in a generated verifier. 48 raw bytes encode to 64
/// base64url characters, comfortably inside RFC 7636's 43-128 character
/// range and far above its 32-byte entropy floor.
pub const VERIFIER_ENTROPY_BYTES: usize = 48;

/// A fresh `code_verifier`, to be stored server-side and never sent to the
/// browser.
pub fn generate_code_verifier() -> String {
    random_urlsafe(VERIFIER_ENTROPY_BYTES)
}

/// `BASE64URL(SHA256(ASCII(code_verifier)))`, unpadded.
///
/// The unpadded encoding is required, not stylistic: RFC 7636 §4.2 specifies
/// base64url without padding, and a provider comparing against a padded
/// string would reject every exchange.
pub fn code_challenge_from_verifier(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Does `verifier` redeem `challenge`?
///
/// Constant-time, because a provider-side equivalent of this comparison is
/// what an attacker attacks; ours is here so our own bootstrap flow, which
/// verifies locally, does not become the weaker of the two.
pub fn verify_code_challenge(verifier: &str, challenge: &str) -> bool {
    secure_equals(
        code_challenge_from_verifier(verifier).as_bytes(),
        challenge.as_bytes(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// RFC 7636 appendix B's worked example. If our encoding ever drifts
    /// this is the test that says so in terms a provider would agree with.
    #[test]
    fn matches_the_rfc_7636_worked_example() {
        assert_eq!(
            code_challenge_from_verifier("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    proptest! {
        /// A verifier always redeems its own challenge. This is the property
        /// the whole flow rests on: get it wrong and every login fails.
        #[test]
        fn a_verifier_always_validates_its_own_challenge(verifier in ".{0,256}") {
            let challenge = code_challenge_from_verifier(&verifier);
            prop_assert!(verify_code_challenge(&verifier, &challenge));
        }

        /// And never redeems anyone else's. Get *this* wrong and the
        /// interception PKCE exists to prevent goes through.
        #[test]
        fn a_different_verifier_never_validates(
            verifier in ".{0,256}",
            other in ".{0,256}",
        ) {
            prop_assume!(verifier != other);
            let challenge = code_challenge_from_verifier(&verifier);
            prop_assert!(!verify_code_challenge(&other, &challenge));
        }

        /// The challenge is URL-safe and unpadded for every input, so it can
        /// go into a query string untouched.
        #[test]
        fn the_challenge_is_always_url_safe_and_unpadded(verifier in ".{0,256}") {
            let challenge = code_challenge_from_verifier(&verifier);
            prop_assert_eq!(challenge.len(), 43); // 32 bytes, unpadded base64
            prop_assert!(
                challenge
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            );
        }

        /// Arbitrary bytes claiming to be a challenge are answered, not
        /// panicked on.
        #[test]
        fn verification_is_total(verifier: String, challenge: String) {
            let _ = verify_code_challenge(&verifier, &challenge);
        }
    }

    #[test]
    fn generated_verifiers_sit_inside_the_rfc_length_range() {
        for _ in 0..64 {
            let verifier = generate_code_verifier();
            assert!(
                (43..=128).contains(&verifier.len()),
                "RFC 7636 §4.1 requires 43-128 characters, got {}",
                verifier.len()
            );
        }
    }
}
