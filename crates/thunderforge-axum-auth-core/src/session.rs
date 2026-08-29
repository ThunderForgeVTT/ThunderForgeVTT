//! What a session cookie has to look like, decided away from the cookie jar.
//!
//! `tower_cookies::Cookie` is a builder, not a policy. The policy — which
//! flags, which lifetime, which of the two cookies is readable by JavaScript
//! — is here, so it can be asserted without standing up a server. The server
//! turns a [`CookieSpec`] into a real cookie in exactly one place; anything
//! that sets these attributes by hand elsewhere has escaped this rule.

/// How long a freshly issued session is good for.
///
/// A week. Long enough that a group's weekly game does not start with
/// everyone logging in again, short enough that a stolen cookie expires
/// without anyone noticing it was stolen.
pub const SESSION_TTL_DAYS: i64 = 7;

pub const SESSION_COOKIE_NAME: &str = "session";
pub const CSRF_COOKIE_NAME: &str = "csrf_token";

/// A cookie's attributes, with no cookie library in sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieSpec {
    pub name: &'static str,
    pub value: String,
    pub path: &'static str,
    /// `true` keeps the value out of `document.cookie`, so an XSS payload
    /// cannot read it.
    pub http_only: bool,
    /// Always `SameSite=Strict` here — see [`session_cookie`].
    pub same_site_strict: bool,
    /// Follows the deployment's `secure_cookies` config: a plain-HTTP local
    /// dev server would never receive a `Secure` cookie back, so forcing it
    /// on would make login silently fail there.
    pub secure: bool,
}

/// The session cookie: the credential itself.
///
/// `http_only` because a script must never be able to read it, and
/// `SameSite=Strict` because a cross-site request has no business carrying
/// it — that flag is the outer defence the CSRF token backs up.
pub fn session_cookie(session_id: &str, secure: bool) -> CookieSpec {
    CookieSpec {
        name: SESSION_COOKIE_NAME,
        value: session_id.to_string(),
        path: "/",
        http_only: true,
        same_site_strict: true,
        secure,
    }
}

/// The CSRF cookie: **deliberately readable by JavaScript**.
///
/// This is the "double submit" half of the pattern. The browser sends the
/// cookie automatically; only our own page can read it back and echo it in
/// the `x-csrf-token` header, because the same-origin policy stops a hostile
/// page from reading it. `http_only: true` here would break the scheme
/// entirely — the front end could never produce the header — so the `false`
/// is load-bearing, not an oversight.
pub fn csrf_cookie(token: &str, secure: bool) -> CookieSpec {
    CookieSpec {
        name: CSRF_COOKIE_NAME,
        value: token.to_string(),
        path: "/",
        http_only: false,
        same_site_strict: true,
        secure,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn the_session_cookie_is_unreadable_by_scripts() {
        assert!(session_cookie("abc", true).http_only);
    }

    #[test]
    fn the_csrf_cookie_is_readable_by_scripts_on_purpose() {
        assert!(!csrf_cookie("abc", true).http_only);
    }

    proptest! {
        /// Whatever the value and whatever the deployment, the flags that
        /// are not configurable stay put. A regression here is invisible in
        /// a browser and only shows up as a stolen session.
        #[test]
        fn invariant_flags_never_depend_on_the_value_or_the_deployment(
            value in ".{0,64}",
            secure: bool,
        ) {
            let session = session_cookie(&value, secure);
            prop_assert!(session.http_only);
            prop_assert!(session.same_site_strict);
            prop_assert_eq!(session.path, "/");
            prop_assert_eq!(session.secure, secure);
            prop_assert_eq!(session.value, value.clone());

            let csrf = csrf_cookie(&value, secure);
            prop_assert!(!csrf.http_only);
            prop_assert!(csrf.same_site_strict);
            prop_assert_eq!(csrf.path, "/");
            prop_assert_eq!(csrf.secure, secure);
        }

        /// The two cookies are never confused for one another.
        #[test]
        fn the_two_cookies_never_share_a_name(value in ".{0,64}") {
            prop_assert_ne!(
                session_cookie(&value, true).name,
                csrf_cookie(&value, true).name
            );
        }
    }
}
