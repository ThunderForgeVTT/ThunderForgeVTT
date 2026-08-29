//! Pulling identity out of a userinfo response.
//!
//! The body is whatever the provider's endpoint returned. OpenID Connect
//! specifies `sub`; the OAuth-2.0-only providers we support each invented
//! their own name for the same thing before OIDC existed.

use serde_json::Value;

/// Keys checked for the provider's user identifier, in order.
///
/// `sub` first because it is the standardised one; `id` for Discord and
/// GitHub, which predate OIDC and never adopted it. First match wins, so a
/// provider returning both gets the standard one — the alternative, letting
/// key order in the JSON decide, would make the account a login lands on
/// depend on how the provider happened to serialise its response.
pub const SUBJECT_KEYS: &[&str] = &["sub", "id", "user_id"];

/// The provider's identifier for this user, or `None` if the response has no
/// usable one.
///
/// Only string values count. Discord returns its snowflake id as a string;
/// a provider returning a bare number is out of spec, and coercing it would
/// mean `1` and `"1"` could resolve to two different local accounts
/// depending on which code path ran.
pub fn extract_subject(userinfo: &Value) -> Option<String> {
    SUBJECT_KEYS
        .iter()
        .find_map(|key| userinfo.get(*key))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// The user's email address, if the provider volunteered one.
///
/// Never an identifier — see `id_token::subject_from_id_token_unverified`.
pub fn extract_email(userinfo: &Value) -> Option<String> {
    userinfo
        .get("email")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

/// Parse a userinfo body and read the identity out of it in one step.
///
/// Returns `None` for a body that is not JSON at all, which is what an
/// HTML error page served with a 200 looks like.
pub fn subject_from_userinfo_body(body: &str) -> Option<String> {
    extract_subject(&serde_json::from_str::<Value>(body).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    #[test]
    fn the_standard_claim_wins_over_the_legacy_ones() {
        let userinfo = json!({ "id": "legacy", "sub": "standard", "user_id": "older" });
        assert_eq!(extract_subject(&userinfo), Some("standard".to_string()));
    }

    #[test]
    fn a_discord_style_response_still_resolves() {
        let userinfo = json!({ "id": "80351110224678912", "username": "wizard" });
        assert_eq!(
            extract_subject(&userinfo),
            Some("80351110224678912".to_string())
        );
    }

    #[test]
    fn a_non_string_identifier_is_refused() {
        assert_eq!(extract_subject(&json!({ "id": 80351110224678912i64 })), None);
    }

    #[test]
    fn a_body_that_is_not_json_yields_nothing_rather_than_erroring_out() {
        assert_eq!(subject_from_userinfo_body("<html>502 Bad Gateway</html>"), None);
    }

    proptest! {
        /// **Totality.** A provider's userinfo body is third-party data on an
        /// unauthenticated code path; no byte sequence may panic.
        #[test]
        fn extraction_is_total_over_arbitrary_bodies(body in ".{0,256}") {
            let _ = subject_from_userinfo_body(&body);
        }

        /// Whatever is extracted is exactly what the provider sent, for
        /// whichever key it used.
        #[test]
        fn any_supported_key_round_trips_verbatim(
            key_index in 0usize..SUBJECT_KEYS.len(),
            value in ".{1,64}",
        ) {
            let key = SUBJECT_KEYS[key_index];
            let userinfo = json!({ key: value });
            prop_assert_eq!(extract_subject(&userinfo), Some(value));
        }

        /// A response using none of the known keys yields nothing rather
        /// than guessing.
        #[test]
        fn an_unknown_key_is_not_guessed_at(
            key in "[a-z_]{1,16}",
            value in ".{1,32}",
        ) {
            prop_assume!(!SUBJECT_KEYS.contains(&key.as_str()));
            prop_assert_eq!(extract_subject(&json!({ key: value })), None);
        }

        /// Email extraction is independent of the subject: a body with an
        /// email and no subject still surfaces the email, and vice versa.
        #[test]
        fn email_and_subject_are_read_independently(
            sub in proptest::option::of(".{1,32}"),
            email in proptest::option::of(".{1,32}"),
        ) {
            let mut body = serde_json::Map::new();
            if let Some(ref s) = sub {
                body.insert("sub".into(), json!(s));
            }
            if let Some(ref e) = email {
                body.insert("email".into(), json!(e));
            }
            let value = Value::Object(body);
            prop_assert_eq!(extract_subject(&value), sub);
            prop_assert_eq!(extract_email(&value), email);
        }
    }
}
