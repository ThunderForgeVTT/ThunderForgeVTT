//! The OpenID Connect discovery document
//! (`<issuer>/.well-known/openid-configuration`).
//!
//! Parsing only. Fetching it is the server's job; so is deciding whether to
//! trust what it says, which is why [`DiscoveryDocument::issuer_matches`]
//! exists as a separate step a caller has to take deliberately.

use serde::Deserialize;

/// The URL a discovery document is published at, given an issuer.
///
/// The trailing slash is stripped first: OpenID Connect Discovery §4 defines
/// the path as appended to the issuer *identifier*, and an operator who pastes
/// `https://idp.example.com/realms/main/` would otherwise get a double slash
/// that many providers 404 on.
pub fn discovery_url(issuer_url: &str) -> String {
    format!(
        "{}/.well-known/openid-configuration",
        issuer_url.trim_end_matches('/')
    )
}

/// The fields we actually use. Everything else in the document is ignored
/// rather than rejected — it is a large, growing, optional-heavy schema and a
/// strict parse would break on the next spec extension a provider adopts.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DiscoveryDocument {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    /// Where the signing keys live. We do not verify ID tokens today (see
    /// `id_token`), so nothing reads this yet; it is parsed because the day
    /// somebody adds verification, the absence of this field is the thing
    /// they need to detect, and discovering that it was never captured is a
    /// worse moment to find out.
    pub jwks_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryError {
    /// Not JSON, or missing one of the fields the flow cannot run without.
    Malformed(String),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::Malformed(detail) => {
                write!(f, "Invalid OpenID Connect discovery document: {detail}")
            }
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Parse a discovery document. Total over arbitrary input.
pub fn parse_discovery_document(body: &str) -> Result<DiscoveryDocument, DiscoveryError> {
    serde_json::from_str(body).map_err(|e| DiscoveryError::Malformed(e.to_string()))
}

impl DiscoveryDocument {
    /// Does the document's `issuer` match the URL we fetched it from?
    ///
    /// OpenID Connect Discovery §4.3 requires this check, and it is not
    /// ceremony: without it a provider (or anything that can answer for one)
    /// can hand back endpoints belonging to a different issuer, and the flow
    /// then sends the user's credentials somewhere the operator never
    /// configured. Compared after stripping a trailing slash on either side,
    /// since that difference is presentational.
    pub fn issuer_matches(&self, expected_issuer: &str) -> bool {
        self.issuer.trim_end_matches('/') == expected_issuer.trim_end_matches('/')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const KEYCLOAK: &str = r#"{
        "issuer": "https://idp.example.com/realms/main",
        "authorization_endpoint": "https://idp.example.com/realms/main/protocol/openid-connect/auth",
        "token_endpoint": "https://idp.example.com/realms/main/protocol/openid-connect/token",
        "userinfo_endpoint": "https://idp.example.com/realms/main/protocol/openid-connect/userinfo",
        "jwks_uri": "https://idp.example.com/realms/main/protocol/openid-connect/certs",
        "response_types_supported": ["code", "id_token"],
        "some_future_field": {"nested": true}
    }"#;

    #[test]
    fn parses_a_real_shaped_document_and_ignores_what_it_does_not_use() {
        let doc = parse_discovery_document(KEYCLOAK).expect("should parse");
        assert_eq!(doc.issuer, "https://idp.example.com/realms/main");
        assert!(doc.jwks_uri.is_some());
        assert!(doc.issuer_matches("https://idp.example.com/realms/main/"));
    }

    #[test]
    fn a_document_missing_the_token_endpoint_is_an_error() {
        let body = r#"{"issuer":"i","authorization_endpoint":"a"}"#;
        assert!(matches!(
            parse_discovery_document(body),
            Err(DiscoveryError::Malformed(_))
        ));
    }

    #[test]
    fn an_issuer_that_does_not_match_is_rejected() {
        let doc = parse_discovery_document(KEYCLOAK).expect("should parse");
        assert!(!doc.issuer_matches("https://evil.example.com/realms/main"));
    }

    #[test]
    fn the_discovery_url_never_doubles_the_slash() {
        assert_eq!(
            discovery_url("https://idp.example.com/realms/main/"),
            "https://idp.example.com/realms/main/.well-known/openid-configuration"
        );
    }

    proptest! {
        /// **Totality.** The document is fetched from an operator-supplied
        /// URL; whatever comes back must be an `Err`, never a panic.
        #[test]
        fn parsing_is_total_over_arbitrary_bodies(body in ".{0,256}") {
            let _ = parse_discovery_document(&body);
        }

        /// Anything accepted agrees with itself: serde guarantees the
        /// three endpoints are present, and the issuer a document states is
        /// the issuer it matches.
        #[test]
        fn anything_accepted_agrees_with_itself(body in ".{0,256}") {
            if let Ok(doc) = parse_discovery_document(&body) {
                let own_issuer = doc.issuer.clone();
                prop_assert!(doc.issuer_matches(&own_issuer));
            }
        }

        /// The issuer check is exact apart from a trailing slash — a
        /// same-prefix issuer must not pass.
        #[test]
        fn a_prefix_issuer_does_not_match(
            issuer in "https://[a-z]{3,10}\\.example\\.com/[a-z]{1,10}",
            suffix in "[a-z]{1,10}",
        ) {
            let doc = DiscoveryDocument {
                issuer: issuer.clone(),
                authorization_endpoint: "a".into(),
                token_endpoint: "t".into(),
                userinfo_endpoint: None,
                jwks_uri: None,
            };
            let extended = format!("{issuer}{suffix}");
            prop_assert!(doc.issuer_matches(&issuer));
            prop_assert!(!doc.issuer_matches(&extended));
        }

        /// The discovery URL is derived, never guessed: whatever issuer goes
        /// in, exactly one well-known suffix comes out.
        #[test]
        fn the_discovery_url_appends_exactly_once(issuer in ".{0,64}") {
            let url = discovery_url(&issuer);
            prop_assert_eq!(
                url.matches("/.well-known/openid-configuration").count(),
                issuer.matches("/.well-known/openid-configuration").count() + 1
            );
        }
    }
}
