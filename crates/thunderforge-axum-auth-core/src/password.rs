//! What a new account is allowed to be called and secured with.
//!
//! The database enforces uniqueness; nothing here can. These are the rules
//! that hold before any query runs, which is why they live in a crate with no
//! connection pool in it.

/// Username bounds, named so the tests assert against the rule rather than
/// against a repeated literal that could drift from it.
pub const USERNAME_MIN_LEN: usize = 3;
pub const USERNAME_MAX_LEN: usize = 32;

/// Minimum password length.
///
/// Length is the only property enforced. Composition rules ("one digit, one
/// symbol") shrink the search space an attacker has to cover and push people
/// toward `Password1!`; a long passphrase is both stronger and easier to
/// remember. Argon2 covers the rest.
pub const PASSWORD_MIN_LEN: usize = 12;

/// Is this character allowed in a username?
///
/// ASCII-only and no whitespace, deliberately: usernames appear in URLs, in
/// log lines and in `@mentions`, and confusable Unicode would let one player
/// impersonate another in a chat log where nobody is inspecting code points.
pub fn is_allowed_username_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.')
}

/// Validate the three fields a manual registration supplies.
///
/// The `Err` strings are shown to the person registering, so each one names
/// the rule that was broken rather than saying "invalid".
pub fn validate_registration_input(
    username: &str,
    email: &str,
    password: &str,
) -> Result<(), String> {
    if username.is_empty() || email.is_empty() || password.is_empty() {
        return Err("Username, email, and password are required".to_string());
    }

    if username.len() < USERNAME_MIN_LEN || username.len() > USERNAME_MAX_LEN {
        return Err(format!(
            "Username must be between {USERNAME_MIN_LEN} and {USERNAME_MAX_LEN} characters"
        ));
    }

    if !username.chars().all(is_allowed_username_char) {
        return Err("Username may only contain letters, numbers, '-', '_' and '.'".to_string());
    }

    if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err("Email address is invalid".to_string());
    }

    if password.len() < PASSWORD_MIN_LEN {
        return Err(format!(
            "Password must be at least {PASSWORD_MIN_LEN} characters long"
        ));
    }

    Ok(())
}

/// Derive a username for an account nobody typed a username for.
///
/// Both the initial-admin bootstrap and OAuth auto-provisioning (ADR-011)
/// land here: there is an email address and possibly a desired name, and an
/// account has to exist regardless. Filtering rather than rejecting is the
/// point — this path has no user to ask for a different name, so it must
/// always produce *something*. It can produce the empty string (an email
/// whose local part is entirely disallowed characters); the caller that
/// persists it is responsible for substituting a fallback, because only the
/// caller knows what a sensible one is.
pub fn derive_bootstrap_username(desired_username: Option<String>, provider_email: &str) -> String {
    let candidate = desired_username
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            provider_email
                .split('@')
                .next()
                .unwrap_or("admin")
                .trim()
                .to_string()
        });

    candidate
        .chars()
        .filter(|c| is_allowed_username_char(*c))
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn registration_validation_rejects_short_password() {
        let result = validate_registration_input("wizard", "wizard@thunderforge.dev", "short");

        assert_eq!(
            result,
            Err("Password must be at least 12 characters long".to_string())
        );
    }

    #[test]
    fn registration_validation_rejects_invalid_username() {
        let result = validate_registration_input(
            "bad name",
            "wizard@thunderforge.dev",
            "very-secure-password",
        );

        assert_eq!(
            result,
            Err("Username may only contain letters, numbers, '-', '_' and '.'".to_string())
        );
    }

    #[test]
    fn registration_validation_accepts_valid_input() {
        let result = validate_registration_input(
            "archmage.1",
            "wizard@thunderforge.dev",
            "very-secure-password",
        );

        assert_eq!(result, Ok(()));
    }

    #[test]
    fn bootstrap_username_falls_back_to_email_local_part() {
        let username = derive_bootstrap_username(None, "Grand.Magister+Admin@thunderforge.dev");

        assert_eq!(username, "grand.magisteradmin");
    }

    /// The boundaries, stated once each. `len()` is bytes, and that is what
    /// the rule checks — a multi-byte name that reads as three characters is
    /// measured as its encoding, which is the behaviour the property below
    /// pins down rather than quietly assumes.
    #[test]
    fn username_length_boundaries_are_inclusive_on_both_ends() {
        let email = "a@b";
        let password = "a-long-enough-password";
        assert!(validate_registration_input("ab", email, password).is_err());
        assert!(validate_registration_input("abc", email, password).is_ok());
        assert!(validate_registration_input(&"a".repeat(32), email, password).is_ok());
        assert!(validate_registration_input(&"a".repeat(33), email, password).is_err());
    }

    #[test]
    fn password_length_boundary_is_inclusive() {
        let (username, email) = ("wizard", "a@b");
        assert!(validate_registration_input(username, email, &"x".repeat(11)).is_err());
        assert!(validate_registration_input(username, email, &"x".repeat(12)).is_ok());
    }

    proptest! {
        /// The property that matters: **anything accepted satisfies every
        /// stated rule**. A test that only checks known-good examples cannot
        /// tell you an unexpected input slipped past; this can.
        #[test]
        fn anything_accepted_satisfies_every_rule(
            username in ".{0,40}",
            email in ".{0,40}",
            password in ".{0,40}",
        ) {
            if validate_registration_input(&username, &email, &password).is_ok() {
                prop_assert!(!username.is_empty() && !email.is_empty() && !password.is_empty());
                prop_assert!((USERNAME_MIN_LEN..=USERNAME_MAX_LEN).contains(&username.len()));
                prop_assert!(username.chars().all(is_allowed_username_char));
                prop_assert!(email.contains('@'));
                prop_assert!(!email.starts_with('@') && !email.ends_with('@'));
                prop_assert!(password.len() >= PASSWORD_MIN_LEN);
            }
        }

        /// And the converse: anything that satisfies every rule is accepted.
        /// Without this the property above would be satisfied by a validator
        /// that rejected everything.
        #[test]
        fn anything_satisfying_every_rule_is_accepted(
            username in "[A-Za-z0-9._-]{3,32}",
            local in "[a-z]{1,10}",
            domain in "[a-z]{1,10}",
            password in ".{12,40}",
        ) {
            let email = format!("{local}@{domain}");
            prop_assert_eq!(
                validate_registration_input(&username, &email, &password),
                Ok(())
            );
        }

        /// Never panics, whatever bytes arrive — this runs on unauthenticated
        /// input from an open registration endpoint.
        #[test]
        fn validation_is_total(username: String, email: String, password: String) {
            let _ = validate_registration_input(&username, &email, &password);
        }

        /// Whatever it derives is composed only of characters a username may
        /// contain, so the derived name can never be one the validator would
        /// have rejected on charset grounds. Length is the caller's problem
        /// (see the doc comment), not this function's.
        #[test]
        fn derived_username_only_uses_allowed_characters(
            desired in proptest::option::of(".{0,40}"),
            email in ".{0,40}",
        ) {
            let derived = derive_bootstrap_username(desired, &email);
            prop_assert!(derived.chars().all(is_allowed_username_char));
            prop_assert_eq!(derived.clone(), derived.to_lowercase());
        }
    }
}
