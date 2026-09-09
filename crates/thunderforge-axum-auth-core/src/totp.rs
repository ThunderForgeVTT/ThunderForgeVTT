//! TOTP verification, with the clock passed in.
//!
//! `totp-rs` will happily read the system clock for you, which makes "does
//! this code verify one step late?" a question you can only answer by
//! waiting thirty seconds. Every rule here takes an explicit Unix timestamp
//! so the window can be walked in a test instead.

use data_encoding::BASE32_NOPAD;
use totp_rs::{Algorithm, Builder, Totp};

/// Seconds per code. RFC 6238's default, and what every authenticator app
/// assumes when it scans a plain `otpauth://` URI.
pub const STEP_SECONDS: u64 = 30;

/// How many steps either side of "now" are accepted.
///
/// One, meaning a code is good for roughly 30-90 seconds. Zero would reject
/// anyone whose phone clock is a few seconds off or who types slowly; larger
/// values widen the window an intercepted code stays replayable in.
pub const SKEW_STEPS: u16 = 1;

pub const DIGITS: u8 = 6;
pub const ISSUER: &str = "ThunderForge";

/// Build the verifier for one user's stored secret.
///
/// Fails rather than panics on a secret that is not valid base32: the value
/// comes out of the database, and a row corrupted by a bad migration must
/// surface as a failed login for one account, not a crashed request handler.
pub fn totp_for(username: &str, secret_base32: &str) -> Result<Totp, String> {
    let secret = BASE32_NOPAD
        .decode(secret_base32.as_bytes())
        .map_err(|_| "Stored 2FA secret is not valid base32".to_string())?;
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(DIGITS)
        .with_skew(SKEW_STEPS)
        .with_step_duration(STEP_SECONDS)
        .with_secret(secret)
        .with_issuer(Some(ISSUER))
        .with_account_name(username)
        .build()
        .map_err(|e| format!("Failed to build TOTP verifier: {e}"))
}

/// Does `code` match the secret right now?
pub fn verify_totp_code(username: &str, secret_base32: &str, code: &str) -> Result<bool, String> {
    Ok(matched_step(username, secret_base32, code)?.is_some())
}

/// Which time step `code` matched, or `None` if it matched none.
///
/// # Why the step, and not just a yes
///
/// Spec 041 FR-016: **a code must not be accepted twice, including within the
/// window.** A TOTP code is valid for its whole step and, with a skew of one,
/// for the neighbouring steps too — so the same six digits stay usable for
/// roughly 30-90 seconds. Anybody who reads them over a shoulder, or off a
/// screen share, or out of a logged request body, can spend them again inside
/// that window against a fresh challenge.
///
/// The only thing that can stop that is remembering **which step was spent**
/// and refusing it a second time, which needs the step to leave this function.
/// `totp-rs` has always returned it — `check`/`check_current` answer
/// `Option<u64>`, not `bool` — and this crate was throwing it away. Its own
/// comment said what the value was for while discarding it.
///
/// The caller stores the returned step against the account and refuses any
/// verification whose step is not strictly greater; see
/// `step_is_unspent` for the comparison, which is where the rule lives.
pub fn matched_step(
    username: &str,
    secret_base32: &str,
    code: &str,
) -> Result<Option<u64>, String> {
    Ok(totp_for(username, secret_base32)?.check_current(code))
}

/// The same question with an explicit clock, for tests that walk the window.
pub fn matched_step_at(
    username: &str,
    secret_base32: &str,
    code: &str,
    unix_time: u64,
) -> Result<Option<u64>, String> {
    Ok(totp_for(username, secret_base32)?.check(code, unix_time))
}

/// May a code that matched `step` be spent, given the last step this account
/// spent?
///
/// **Strictly greater**, not "different". A code from an *earlier* step than
/// the one already spent is inside the skew window looking backwards, and
/// accepting it would leave a replay path open in the direction nobody thinks
/// about: sign in with the current code, then replay the previous step's code,
/// which is still within skew and has a different step number.
///
/// `None` for `last_spent` is an account that has never verified, where every
/// step is available.
pub fn step_is_unspent(step: u64, last_spent: Option<u64>) -> bool {
    match last_spent {
        None => true,
        Some(last) => step > last,
    }
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
    Ok(totp_for(username, secret_base32)?
        .check(code, unix_time)
        .is_some())
}

/// The code the secret produces at `unix_time`. Used to render the QR-code
/// enrolment check and, here, to generate the codes the window is tested with.
pub fn generate_code_at(
    username: &str,
    secret_base32: &str,
    unix_time: u64,
) -> Result<String, String> {
    Ok(totp_for(username, secret_base32)?
        .generate(unix_time)
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// 20 bytes, the SHA-1 HMAC block size RFC 4226 recommends.
    const SECRET: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    /// A fixed instant well clear of any step boundary, so the arithmetic
    /// below is about the rule and not about rounding.
    const NOW: u64 = 1_760_000_000;

    /// FR-016, the half that says a step can be identified at all. The step is
    /// `unix_time / STEP_SECONDS`, and the same code inside the same step
    /// matches the same number however often it is offered.
    #[test]
    fn a_code_reports_the_step_it_matched() {
        let code = generate_code_at("wizard", SECRET, NOW).expect("a code");
        let step = matched_step_at("wizard", SECRET, &code, NOW)
            .expect("verified")
            .expect("the code matches at the instant it was made for");

        assert_eq!(step, NOW / STEP_SECONDS);
        assert_eq!(
            matched_step_at("wizard", SECRET, &code, NOW + 1)
                .expect("verified")
                .expect("still inside the same step"),
            step,
            "one second later is the same step, so the same number",
        );
    }

    #[test]
    fn a_code_that_matches_nothing_reports_no_step() {
        assert_eq!(
            matched_step_at("wizard", SECRET, "000000", NOW).expect("verified"),
            None,
        );
    }

    /// FR-016 itself. A code spent once is refused inside its own window,
    /// which is the whole point: it stays cryptographically valid for another
    /// 30-90 seconds, and only the spent-step record can refuse it.
    #[test]
    fn the_same_code_is_refused_the_second_time_inside_its_window() {
        let code = generate_code_at("wizard", SECRET, NOW).expect("a code");
        let step = matched_step_at("wizard", SECRET, &code, NOW)
            .expect("verified")
            .expect("matches");

        assert!(
            step_is_unspent(step, None),
            "an account that has never verified may spend any step",
        );

        // The account now records `step`. The same code, still well within its
        // skew window, matches again — and must not be spendable again.
        let again = matched_step_at("wizard", SECRET, &code, NOW + 20)
            .expect("verified")
            .expect("still cryptographically valid, which is the problem");
        assert_eq!(again, step);
        assert!(
            !step_is_unspent(again, Some(step)),
            "a spent code must be refused while it is still valid",
        );
    }

    /// The direction nobody thinks about. With a skew of one, the *previous*
    /// step's code is still accepted — and it carries a different step number,
    /// so a rule that only refused the exact same step would let it through.
    /// This is why the comparison is strictly-greater and not not-equal.
    #[test]
    fn an_earlier_steps_code_cannot_be_replayed_after_a_later_one() {
        let previous = generate_code_at("wizard", SECRET, NOW - STEP_SECONDS).expect("a code");
        let current = generate_code_at("wizard", SECRET, NOW).expect("a code");

        let current_step = matched_step_at("wizard", SECRET, &current, NOW)
            .expect("verified")
            .expect("matches");
        let previous_step = matched_step_at("wizard", SECRET, &previous, NOW)
            .expect("verified")
            .expect("the skew window still accepts the previous step");

        assert!(
            previous_step < current_step,
            "the previous step is genuinely earlier, not the same number",
        );
        assert!(
            !step_is_unspent(previous_step, Some(current_step)),
            "after spending the current step, the previous one must be dead — \
             it is a different number, so `!=` would have admitted it",
        );
    }

    #[test]
    fn a_later_step_is_spendable_after_an_earlier_one() {
        let step = NOW / STEP_SECONDS;
        assert!(
            step_is_unspent(step + 1, Some(step)),
            "the next code must work, or the factor locks the account out",
        );
    }

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
