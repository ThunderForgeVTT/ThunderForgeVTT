//! The token endpoint's response, parsed.
//!
//! This body comes from a third party. It is fetched over TLS from the
//! provider's own token endpoint, so it is not *attacker*-controlled in the
//! authorization-code flow — but it is still someone else's JSON, written by
//! someone else's server, and a provider that returns an HTML error page with
//! a 200 must produce an `Err` and not a panic.

use serde::Deserialize;

/// RFC 6749 §5.1, plus OpenID Connect's `id_token`.
///
/// Unknown fields are ignored rather than rejected: providers add their own
/// (`scope`, `token_type`, Discord's `webhook`), and a strict struct here
/// would break a login every time one of them shipped a new field.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    /// The OpenID Connect ID token, when the provider issues one.
    ///
    /// This field used to be absent from the struct entirely, so serde
    /// discarded it during deserialization and the "read the subject out of
    /// the ID token" fallback had nothing to read — see
    /// `thunderforge_axum_oidc::id_token`. A provider that returns the
    /// subject only in the ID token could not log anybody in.
    pub id_token: Option<String>,
}

/// Why a token response could not be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenParseError {
    /// The body was not JSON, or was JSON of the wrong shape.
    Malformed(String),
    /// Parsed, and carried an `access_token` that is the empty string.
    ///
    /// A *missing* `access_token` is a `Malformed` — serde stops there,
    /// and the message it produces ("missing field `access_token`") is the
    /// one `src/server` has always logged for that case. An **empty** one
    /// deserializes cleanly and would otherwise be sent as a bearer token,
    /// turning a broken token response into a confusing 401 from the
    /// userinfo endpoint one step later.
    MissingAccessToken,
}

impl std::fmt::Display for TokenParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Matches the message `src/server`'s token exchange has always
            // returned, so operator-facing logs do not change wording.
            TokenParseError::Malformed(detail) => {
                write!(f, "Invalid token response format: {detail}")
            }
            TokenParseError::MissingAccessToken => {
                f.write_str("Token response contained no access_token")
            }
        }
    }
}

impl std::error::Error for TokenParseError {}

/// Parse a token-endpoint body.
///
/// Total: every byte sequence produces `Ok` or `Err`, never a panic.
pub fn parse_token_response(body: &str) -> Result<TokenResponse, TokenParseError> {
    let parsed: TokenResponse =
        serde_json::from_str(body).map_err(|e| TokenParseError::Malformed(e.to_string()))?;

    if parsed.access_token.is_empty() {
        return Err(TokenParseError::MissingAccessToken);
    }

    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn parses_a_typical_oidc_response() {
        let parsed = parse_token_response(
            r#"{"access_token":"at","refresh_token":"rt","expires_in":3600,
                "id_token":"header.payload.signature","token_type":"Bearer","scope":"openid"}"#,
        )
        .expect("should parse");

        assert_eq!(parsed.access_token, "at");
        assert_eq!(parsed.refresh_token.as_deref(), Some("rt"));
        assert_eq!(parsed.expires_in, Some(3600));
        assert_eq!(parsed.id_token.as_deref(), Some("header.payload.signature"));
    }

    /// The regression this crate exists to prevent: a provider that issues an
    /// ID token must not have it silently dropped on the floor.
    #[test]
    fn the_id_token_survives_parsing() {
        let parsed = parse_token_response(r#"{"access_token":"at","id_token":"a.b.c"}"#)
            .expect("should parse");
        assert_eq!(parsed.id_token.as_deref(), Some("a.b.c"));
    }

    #[test]
    fn unknown_fields_are_ignored_not_rejected() {
        let parsed = parse_token_response(r#"{"access_token":"at","webhook":{"id":"1"}}"#)
            .expect("a provider's extra fields must not break login");
        assert_eq!(parsed.access_token, "at");
    }

    #[test]
    fn a_response_with_no_usable_access_token_is_refused() {
        // An error body: serde reports the missing field, which is the
        // wording the server has always surfaced for this.
        assert!(matches!(
            parse_token_response(r#"{"error":"invalid_grant"}"#),
            Err(TokenParseError::Malformed(_))
        ));
        // An empty one parses, and is caught here rather than becoming an
        // empty `Authorization: Bearer` header.
        assert_eq!(
            parse_token_response(r#"{"access_token":""}"#),
            Err(TokenParseError::MissingAccessToken)
        );
        assert!(matches!(
            parse_token_response("<html>502</html>"),
            Err(TokenParseError::Malformed(_))
        ));
    }

    proptest! {
        /// **Totality.** The parser eats data from a third party; the one
        /// thing it must never do is panic, whatever arrives.
        #[test]
        fn parsing_is_total_over_arbitrary_bytes(body in ".{0,256}") {
            let _ = parse_token_response(&body);
        }

        /// Including deliberately JSON-shaped noise, which reaches deeper
        /// into serde than random text does.
        #[test]
        fn parsing_is_total_over_arbitrary_json(value in json_value()) {
            let _ = parse_token_response(&value.to_string());
        }

        /// Anything accepted is usable: a non-empty access token, every time.
        #[test]
        fn anything_accepted_carries_a_usable_access_token(value in json_value()) {
            if let Ok(parsed) = parse_token_response(&value.to_string()) {
                prop_assert!(!parsed.access_token.is_empty());
            }
        }

        /// A well-formed response round-trips every field, whatever the
        /// values contain.
        #[test]
        fn well_formed_responses_round_trip(
            access_token in "[^\\\\\"]{1,32}",
            id_token in proptest::option::of("[^\\\\\"]{0,32}"),
            expires_in in proptest::option::of(any::<i64>()),
        ) {
            let body = serde_json::json!({
                "access_token": access_token,
                "id_token": id_token,
                "expires_in": expires_in,
            })
            .to_string();
            let parsed = parse_token_response(&body).expect("valid response must parse");
            prop_assert_eq!(parsed.access_token, access_token);
            prop_assert_eq!(parsed.id_token, id_token);
            prop_assert_eq!(parsed.expires_in, expires_in);
        }
    }

    /// Arbitrary JSON, including the nesting and type confusion a hostile or
    /// merely broken provider could send.
    fn json_value() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            ".{0,16}".prop_map(serde_json::Value::from),
        ];
        leaf.prop_recursive(4, 32, 4, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
                proptest::collection::hash_map("[a-z_]{1,12}", inner, 0..4)
                    .prop_map(|m| { serde_json::Value::Object(m.into_iter().collect()) }),
            ]
        })
    }
}
