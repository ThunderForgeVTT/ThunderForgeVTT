//! TOTP verification, with the clock passed in.
//!
//! `totp-rs` will happily read the system clock for you, which makes "does
//! this code verify one step late?" a question you can only answer by
//! waiting thirty seconds. Every rule here takes an explicit Unix timestamp
//! so the window can be walked in a test instead.

use data_encoding::BASE32_NOPAD;
use totp_rs::{Algorithm, TOTP};

/// Seconds per code. RFC 6238's default, and what every authenticator app
/// assumes when it scans a plain `otpauth://` URI.
pub const STEP_SECONDS: u64 = 30;

/// How many steps either side of "now" are accepted.
///
/// One, meaning a code is good for roughly 30-90 seconds. Zero would reject
/// anyone whose phone clock is a few seconds off or who types slowly; larger
/// values widen the window an intercepted code stays replayable in.
pub const SKEW_STEPS: u8 = 1;

pub const DIGITS: usize = 6;
pub const ISSUER: &str = "ThunderForge";

/// Build the verifier for one user's stored secret.
///
/// Fails rather than panics on a secret that is not valid base32: the value
/// comes out of the database, and a row corrupted by a bad migration must
/// surface as a failed login for one account, not a crashed request handler.
pub fn totp_for(username: &str, secret_base32: &str) -> Result<TOTP, String> {
    let secret = BASE32_NOPAD
        .decode(secret_base32.as_bytes())
        .map_err(|_| "Stored 2FA secret is not valid base32".to_string())?;
    TOTP::new(
        Algorithm::SHA1,
        DIGITS,
        SKEW_STEPS,
        STEP_SECONDS,
        secret,
        Some(ISSUER.to_string()),
        username.to_string(),
    )
    .map_err(|e| format!("Failed to build TOTP verifier: {e}"))
}

/// Does `code` match the secret right now?
pub fn verify_totp_code(username: &str, secret_base32: &str, code: &str) -> Result<bool, String> {
    let totp = totp_for(username, secret_base32)?;
    // totp-rs 6.0 changed this from `Result<bool, _>` to `Option<u64>`: `Some`
    // carries the matched time step so a caller can refuse to accept the same
    // step twice, and there is no longer a fallible-clock error to surface.
    // We only ask whether the code matched, so the step is dropped here.
    Ok(totp.check_current(code).is_some())
}

/// Does `code` match the secret at `unix_time`?
///
/// The testable form of [`verify_totp_code`]. Same rule, explicit clock.
pub fn verify_totp_code_at(
    username: &str,
    secret_base32: &str,
    code: &str,
    unix_time: u64,
) -> Result<bool, String> {
    Ok(totp_for(username, secret_base32)?.check(code, unix_time).is_some())
}

/// The code the secret produces at `unix_time`. Used to render the QR-code
/// enrolment check and, here, to generate the codes the window is tested with.
pub fn generate_code_at(
    username: &str,
    secret_base32: &str,
    unix_time: u64,
) -> Result<String, String> {
    Ok(totp_for(username, secret_base32)?.generate(unix_time).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// 20 bytes, the SHA-1 HMAC block size RFC 4226 recommends.
    const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn a_secret_that_is_not_base32_fails_instead_of_panicking() {
        let err = verify_totp_code("wizard", "not base32!", "123456").unwrap_err();
        assert_eq!(err, "Stored 2FA secret is not valid base32");
    }

    proptest! {
        /// **Inside the window is accepted.** `SKEW_STEPS` either side of the
        /// step the code was generated for, which is the promise made to the
        /// person whose phone clock drifts.
        #[test]
        fn a_code_verifies_anywhere_inside_the_skew_window(
            base_step in 1u64..100_000_000,
            offset in -(SKEW_STEPS as i64)..=(SKEW_STEPS as i64),
        ) {
            let generated_at = base_step * STEP_SECONDS;
            let code = generate_code_at("wizard", SECRET, generated_at).unwrap();
            let checked_at = (base_step as i64 + offset) as u64 * STEP_SECONDS;
            prop_assert!(
                verify_totp_code_at("wizard", SECRET, &code, checked_at).unwrap(),
                "code from step {base_step} must verify {offset} steps away",
            );
        }

        /// **Outside the window is refused.** Without this the test above is
        /// satisfied by a verifier that accepts everything, and an
        /// intercepted code would stay usable indefinitely.
        #[test]
        fn a_code_is_refused_outside_the_skew_window(
            base_step in 1000u64..100_000_000,
            offset in (SKEW_STEPS as i64 + 1)..=500,
            ahead: bool,
        ) {
            let generated_at = base_step * STEP_SECONDS;
            let code = generate_code_at("wizard", SECRET, generated_at).unwrap();
            let delta = if ahead { offset } else { -offset };
            let checked_at = (base_step as i64 + delta) as u64 * STEP_SECONDS;
            // Six digits collide once in a million by chance. That is a
            // genuine property of TOTP, not a bug in the window rule, so
            // discard the case rather than let it flake the suite.
            prop_assume!(generate_code_at("wizard", SECRET, checked_at).unwrap() != code);
            prop_assert!(
                !verify_totp_code_at("wizard", SECRET, &code, checked_at).unwrap(),
                "code from step {base_step} must not verify {delta} steps away",
            );
        }

        /// Whatever a client submits as a code, verification answers rather
        /// than panics. This is reached from an unauthenticated endpoint.
        #[test]
        fn verification_is_total_over_arbitrary_submitted_codes(
            code in ".{0,32}",
            unix_time in 0u64..100_000_000,
        ) {
            let _ = verify_totp_code_at("wizard", SECRET, &code, unix_time);
        }

        /// And over arbitrary stored secrets — including the corrupt row.
        #[test]
        fn verification_is_total_over_arbitrary_stored_secrets(
            secret in ".{0,64}",
            code in "[0-9]{0,8}",
        ) {
            let _ = verify_totp_code_at("wizard", &secret, &code, 1_700_000_000);
        }
    }
}
