//! Token generation. Shape only — the entropy comes from the OS RNG.

use base64::Engine as _;
use rand::RngExt;

/// `len` random bytes, URL-safe base64 without padding.
///
/// Used for OAuth `state`, PKCE verifiers and session-adjacent tokens, all of
/// which end up in a URL or a form field; padding characters there survive
/// one round trip and break on the next.
pub fn random_urlsafe(len: usize) -> String {
    let mut bytes = vec![0u8; len];
    let mut rng = rand::rng();
    rng.fill(&mut bytes[..]);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// A one-time admin bootstrap code, in `XXXX-XXXX-XXXX` form.
///
/// The alphabet omits `I`, `O`, `0` and `1`: an operator reads this code off
/// a server log and types it into a browser, and the pairs that look alike in
/// a terminal font are the ones that turn a working setup into a support
/// question.
pub fn random_setup_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 12];
    let mut rng = rand::rng();
    rand::RngExt::fill(&mut rng, &mut bytes);

    let token = bytes
        .iter()
        .map(|byte| CHARSET[*byte as usize % CHARSET.len()] as char)
        .collect::<String>();

    format!("{}-{}-{}", &token[0..4], &token[4..8], &token[8..12])
}

#[cfg(test)]
mod tests {
    use super::{random_setup_code, random_urlsafe};
    use base64::Engine as _;
    use proptest::prelude::*;

    #[test]
    fn setup_code_uses_expected_fantasy_friendly_format() {
        let code = random_setup_code();

        assert_eq!(code.len(), 14);
        assert!(
            code.chars()
                .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '-')
        );
        assert_eq!(code.chars().filter(|ch| *ch == '-').count(), 2);
    }

    #[test]
    fn setup_code_avoids_the_lookalike_characters() {
        for _ in 0..256 {
            let code = random_setup_code();
            assert!(
                !code.contains(['I', 'O', '0', '1']),
                "an operator has to retype this: {code}",
            );
        }
    }

    proptest! {
        /// Whatever length is asked for, the result decodes back to exactly
        /// that many bytes — no padding, no truncation, no panic.
        #[test]
        fn urlsafe_token_round_trips_to_the_requested_byte_count(len in 0usize..=128) {
            let token = random_urlsafe(len);
            let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(token.as_bytes())
                .expect("generated token must be valid URL-safe base64");
            prop_assert_eq!(decoded.len(), len);
        }

        /// Nothing that needs escaping in a query string ever appears.
        #[test]
        fn urlsafe_token_is_url_safe(len in 0usize..=128) {
            let token = random_urlsafe(len);
            prop_assert!(
                token
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            );
        }
    }
}
