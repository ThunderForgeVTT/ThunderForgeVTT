//! Hashing and verifying a password.
//!
//! Spec 041 T006. Before this, `hash_password` lived in `sessions.rs` and
//! **five** call sites did their own `Argon2::default().verify_password(...)`
//! against a `PasswordHash` they parsed themselves. Five copies of a security
//! primitive is five chances to get one of them subtly wrong, and one of them
//! already was:
//!
//! ```ignore
//! let parsed = PasswordHash::new(&password_hash).expect("Invalid hash in db");
//! ```
//!
//! A stored hash that will not parse **panicked the request handler**. That is
//! reachable — a truncated column, a value written by an older format, a row
//! restored from a partial backup — and the correct answer to "this hash is not
//! a hash" is that the password does not verify, not that the server falls
//! over. [`verify`] returns `false`.
//!
//! # Why it lives in this crate
//!
//! Hashing needs no connection pool, no request and no cookie jar, which is
//! this crate's entire admission test. `password.rs` beside it holds the
//! *policy* — what a password is allowed to be — and this holds the
//! *mechanism*; the crate already separates `constant_time`, `csrf`, `random`,
//! `session` and `totp` the same way.
//!
//! # Why `verify` swallows the reason
//!
//! There is exactly one thing a caller can do with the outcome, and it is the
//! same for a wrong password and an unparseable hash: refuse. A `Result` here
//! would invite a caller to distinguish them, and the distinction is one an
//! attacker would like to have.

use argon2::{Argon2, PasswordHash, PasswordHasher as _, PasswordVerifier as _};

/// Hash a password — or any secret verified the same way — for storage.
///
/// `Argon2::default()` is **Argon2id**, and a large random salt is generated
/// per call, so two people with the same password get different stored strings
/// and a stolen table cannot be attacked once for everybody. What comes back is
/// a PHC string carrying the variant, the version, the parameters and the salt
/// alongside the digest, which is what lets a future re-hash on verify know
/// what it is looking at.
pub fn hash(value: &str) -> Result<String, String> {
    // One argument: this `hash_password` (as distinct from
    // `hash_password_with_salt`) generates a large random salt per call.
    Argon2::default()
        .hash_password(value.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|e| format!("Failed to hash value: {e}"))
}

/// Whether `presented` matches `stored`.
///
/// `false` for a wrong password **and** for a stored value that is not a valid
/// PHC string. See the module header for why that is not an error.
pub fn verify(presented: &str, stored: &str) -> bool {
    PasswordHash::new(stored)
        .ok()
        .map(|parsed| {
            Argon2::default()
                .verify_password(presented.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash() {
        let stored = hash("correct horse battery staple").expect("hashed");
        assert!(verify("correct horse battery staple", &stored));
        assert!(!verify("correct horse battery stapl", &stored));
    }

    #[test]
    fn two_hashes_of_one_password_differ() {
        // The salt is the whole point. Losing it would be a one-character
        // change that no other test would notice, and it would make a stolen
        // table attackable once for everybody.
        let a = hash("same password").expect("hashed");
        let b = hash("same password").expect("hashed");
        assert_ne!(a, b);
        assert!(verify("same password", &a));
        assert!(verify("same password", &b));
    }

    #[test]
    fn a_stored_value_that_is_not_a_hash_does_not_verify_and_does_not_panic() {
        // The bug this module was extracted to fix: the previous inline form
        // was `PasswordHash::new(stored).expect(...)`, which panicked the
        // request handler on a truncated or foreign-format column.
        for nonsense in ["", "not a hash", "$argon2id$v=19$truncated"] {
            assert!(!verify("anything", nonsense), "{nonsense:?}");
        }
    }

    #[test]
    fn the_stored_form_says_what_it_is() {
        let stored = hash("a password").expect("hashed");
        assert!(stored.starts_with("$argon2id$"), "{stored}");
    }
}
