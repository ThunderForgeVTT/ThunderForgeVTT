//! The `state` parameter: binding a callback to the request that started it.
//!
//! An OAuth callback is an unauthenticated GET that anyone can forge. `state`
//! is the only thing that says this particular redirect belongs to a login
//! *we* began — without it, an attacker can complete a flow against their own
//! provider account in a victim's browser and silently link it to the
//! victim's session (login CSRF).

use thunderforge_axum_auth_core::constant_time::secure_equals;
use thunderforge_axum_auth_core::random::random_urlsafe;

/// 32 random bytes. Enough that guessing is not a strategy; the value is
/// also a database lookup key, so it has to be unique in practice as well as
/// unpredictable.
pub const STATE_ENTROPY_BYTES: usize = 32;

pub fn generate_state() -> String {
    random_urlsafe(STATE_ENTROPY_BYTES)
}

/// Does the `state` that came back on the callback match the one we issued?
///
/// An **empty issued state never matches**, even against an empty callback
/// value. A provider that drops the parameter, or a hand-crafted callback
/// with no `state` at all, must fail closed — comparing `"" == ""` and
/// accepting is precisely the hole `state` exists to close.
pub fn state_matches(issued: &str, returned: &str) -> bool {
    if issued.is_empty() {
        return false;
    }
    secure_equals(issued.as_bytes(), returned.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn an_absent_state_never_matches() {
        assert!(!state_matches("", ""));
    }

    proptest! {
        /// Round-trips unchanged.
        #[test]
        fn an_issued_state_matches_itself(state in "[A-Za-z0-9_-]{1,64}") {
            prop_assert!(state_matches(&state, &state));
        }

        /// **Any** tampering is rejected — a changed character, a truncation,
        /// an appended suffix, an empty value.
        #[test]
        fn any_tampering_is_rejected(
            state in "[A-Za-z0-9_-]{1,64}",
            tampered in ".{0,80}",
        ) {
            prop_assume!(tampered != state);
            prop_assert!(!state_matches(&state, &tampered));
        }

        /// Truncation specifically, since a length-agnostic comparison would
        /// accept a prefix and this is the shape an attacker would try.
        #[test]
        fn a_prefix_of_the_state_is_rejected(
            state in "[A-Za-z0-9_-]{2,64}",
            cut in 1usize..64,
        ) {
            let cut = cut % state.len();
            prop_assert!(!state_matches(&state, &state[..cut]));
        }

        #[test]
        fn comparison_is_total(issued: String, returned: String) {
            let _ = state_matches(&issued, &returned);
        }
    }
}
