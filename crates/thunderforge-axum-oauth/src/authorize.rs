//! Building the URL we send the browser to.

use crate::pkce::CODE_CHALLENGE_METHOD;
use url::Url;

/// Everything that goes into an authorization request.
///
/// Assembled by the caller from a stored provider row plus the freshly
/// generated `state`/PKCE pair, so this function has no way to accidentally
/// reuse a stale one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizeRequest<'a> {
    pub authorization_url: &'a str,
    pub client_id: &'a str,
    pub redirect_uri: &'a str,
    pub scopes: &'a [String],
    pub state: &'a str,
    pub code_challenge: &'a str,
}

/// The provider's authorization URL is unusable.
///
/// Its own kind rather than a `String`: the caller answers this with a 500
/// (we stored something invalid) whereas every other failure in the flow is a
/// 4xx or a 502, and collapsing them into one error type is how those get
/// mixed up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidAuthorizationUrl;

impl std::fmt::Display for InvalidAuthorizationUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Provider authorization URL is invalid")
    }
}

impl std::error::Error for InvalidAuthorizationUrl {}

/// Build the authorization-request URL.
///
/// Scopes are joined with a single space (RFC 6749 §3.3) and the whole query
/// is written through `url`'s encoder rather than by string concatenation —
/// a redirect URI or scope containing `&` would otherwise inject a parameter
/// of the attacker's choosing into our own authorization request.
pub fn build_authorize_url(
    request: &AuthorizeRequest<'_>,
) -> Result<String, InvalidAuthorizationUrl> {
    let mut url = Url::parse(request.authorization_url).map_err(|_| InvalidAuthorizationUrl)?;

    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", request.client_id)
        .append_pair("redirect_uri", request.redirect_uri)
        .append_pair("scope", &request.scopes.join(" "))
        .append_pair("state", request.state)
        .append_pair("code_challenge", request.code_challenge)
        .append_pair("code_challenge_method", CODE_CHALLENGE_METHOD);

    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;

    fn params(url: &str) -> BTreeMap<String, String> {
        Url::parse(url)
            .expect("built URL must parse")
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    fn request<'a>(scopes: &'a [String], state: &'a str) -> AuthorizeRequest<'a> {
        AuthorizeRequest {
            authorization_url: "https://idp.example.com/authorize",
            client_id: "client-123",
            redirect_uri: "https://vtt.example.com/callback",
            scopes,
            state,
            code_challenge: "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
        }
    }

    #[test]
    fn carries_every_parameter_the_flow_requires() {
        let scopes = vec!["openid".to_string(), "email".to_string()];
        let url = build_authorize_url(&request(&scopes, "st-1")).expect("should build");
        let params = params(&url);

        assert_eq!(params["response_type"], "code");
        assert_eq!(params["client_id"], "client-123");
        assert_eq!(params["redirect_uri"], "https://vtt.example.com/callback");
        assert_eq!(params["scope"], "openid email");
        assert_eq!(params["state"], "st-1");
        assert_eq!(params["code_challenge_method"], "S256");
    }

    /// A provider URL that already carries query parameters (Keycloak realms
    /// behind a gateway sometimes do) must keep them.
    #[test]
    fn preserves_query_parameters_already_on_the_provider_url() {
        let scopes = vec!["openid".to_string()];
        let mut req = request(&scopes, "st-1");
        req.authorization_url = "https://idp.example.com/authorize?tenant=main";
        let url = build_authorize_url(&req).expect("should build");
        assert_eq!(params(&url)["tenant"], "main");
    }

    #[test]
    fn a_url_we_cannot_parse_is_an_error_not_a_panic() {
        let scopes: Vec<String> = vec![];
        let mut req = request(&scopes, "st-1");
        req.authorization_url = "not a url";
        assert_eq!(build_authorize_url(&req), Err(InvalidAuthorizationUrl));
    }

    proptest! {
        /// **Nothing injects.** Whatever characters a scope, redirect URI or
        /// state contains, they come back out as the value of their own
        /// parameter and never as a new parameter — the property that makes
        /// string concatenation the wrong way to build this URL.
        #[test]
        fn values_round_trip_without_injecting_parameters(
            state in ".{0,64}",
            redirect in ".{0,64}",
            scope_list in proptest::collection::vec(".{0,16}", 0..5),
        ) {
            let joined = scope_list.join(" ");
            let req = AuthorizeRequest {
                authorization_url: "https://idp.example.com/authorize",
                client_id: "client-123",
                redirect_uri: &redirect,
                scopes: &scope_list,
                state: &state,
                code_challenge: "challenge",
            };
            let url = build_authorize_url(&req).expect("https URL must build");
            let params = params(&url);

            prop_assert_eq!(params.get("state"), Some(&state));
            prop_assert_eq!(params.get("redirect_uri"), Some(&redirect));
            prop_assert_eq!(params.get("scope"), Some(&joined));
            prop_assert_eq!(params.len(), 7, "exactly the seven parameters we appended");
        }

        /// Total over arbitrary provider URLs: a malformed row in
        /// `oauth_providers` is an error for one provider, not a panic that
        /// takes the request down.
        #[test]
        fn building_is_total_over_arbitrary_provider_urls(
            authorization_url in ".{0,64}",
        ) {
            let scopes: Vec<String> = vec![];
            let req = AuthorizeRequest {
                authorization_url: &authorization_url,
                client_id: "c",
                redirect_uri: "r",
                scopes: &scopes,
                state: "s",
                code_challenge: "c",
            };
            let _ = build_authorize_url(&req);
        }
    }
}
