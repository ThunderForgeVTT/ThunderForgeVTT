//! The double-submit CSRF rule, separated from the middleware that applies it.

use crate::constant_time::secure_equals;

/// HTTP methods that carry a CSRF check. Safe methods are excluded because a
/// GET that changes state is the bug, not the missing token.
pub fn method_requires_csrf(method: &str) -> bool {
    matches!(method, "POST" | "PUT" | "PATCH" | "DELETE")
}

/// Does the `x-csrf-token` header match the `csrf_token` cookie?
///
/// An **empty cookie is always a failure**, never a match against an equally
/// empty header. Without that rule a request that simply sends neither would
/// compare `"" == ""` and sail through, which is exactly the request an
/// attacker's cross-origin form produces: the browser withholds the header,
/// and a same-site-stripped or missing cookie would then authorise it.
pub fn csrf_token_matches(cookie_value: &str, header_value: &str) -> bool {
    if cookie_value.is_empty() {
        return false;
    }
    secure_equals(cookie_value.as_bytes(), header_value.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::{csrf_token_matches, method_requires_csrf};
    use proptest::prelude::*;

    #[test]
    fn empty_cookie_never_matches_even_an_empty_header() {
        assert!(!csrf_token_matches("", ""));
    }

    #[test]
    fn safe_methods_are_exempt() {
        assert!(!method_requires_csrf("GET"));
        assert!(!method_requires_csrf("HEAD"));
        assert!(!method_requires_csrf("OPTIONS"));
        assert!(method_requires_csrf("POST"));
    }

    proptest! {
        /// A token only ever authorises itself.
        #[test]
        fn a_nonempty_token_matches_itself(token in "[A-Za-z0-9-]{1,64}") {
            prop_assert!(csrf_token_matches(&token, &token));
        }

        /// Anything else is refused — including the empty header a
        /// cross-origin request would send.
        #[test]
        fn a_different_header_is_refused(
            token in "[A-Za-z0-9-]{1,64}",
            header in ".{0,64}",
        ) {
            prop_assume!(header != token);
            prop_assert!(!csrf_token_matches(&token, &header));
        }
    }
}
