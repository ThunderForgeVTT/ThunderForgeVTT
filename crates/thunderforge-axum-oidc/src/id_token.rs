//! Reading claims out of an ID token — **without verifying its signature**.
//!
//! # The trust argument, and where it stops
//!
//! An ID token is a JWT: three base64url segments, the third a signature over
//! the first two. Verifying it means fetching the provider's JWKS, selecting
//! the key by `kid`, checking the algorithm, then checking `iss`, `aud`,
//! `exp` and `nonce`. **None of that happens here.** These functions decode
//! the claims segment and read it. A forged token with a garbage signature
//! parses exactly as happily as a real one.
//!
//! That is defensible in precisely one place, and it is the only place these
//! functions are called from: the **authorization-code flow's token
//! exchange**. There, the ID token arrives in the body of an HTTPS response
//! to a request *we* made, to a URL *we* configured, authenticated with our
//! own client secret. TLS already establishes that the bytes came from the
//! provider we meant to ask. The signature would prove the same fact a second
//! time, which is why OpenID Connect Core §3.1.3.7 explicitly permits
//! skipping it for tokens obtained directly from the token endpoint.
//!
//! It would **not** be defensible for a token that arrives any other way. A
//! token posted by a browser, passed in a header, pulled from a URL fragment
//! in an implicit flow, or forwarded by another service is attacker-chosen,
//! and reading `sub` out of it unverified means letting the attacker pick
//! whose account they log into. If a future call site is one of those, it
//! needs real JWKS verification, and the function names here say
//! `_unverified` so that call site cannot be written by accident.

use base64::Engine as _;
use serde_json::{Map, Value};

/// Why an ID token could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdTokenError {
    /// Not three dot-separated segments.
    NotAJwt,
    /// The claims segment was not valid unpadded base64url.
    ClaimsNotBase64Url,
    /// The claims segment decoded to something that is not a JSON object.
    ClaimsNotJsonObject,
}

impl std::fmt::Display for IdTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdTokenError::NotAJwt => f.write_str("ID token is not a three-segment JWT"),
            IdTokenError::ClaimsNotBase64Url => {
                f.write_str("ID token claims are not valid base64url")
            }
            IdTokenError::ClaimsNotJsonObject => {
                f.write_str("ID token claims are not a JSON object")
            }
        }
    }
}

impl std::error::Error for IdTokenError {}

/// Decode an ID token's claims. **Does not verify the signature** — read the
/// module documentation before calling this from anywhere new.
pub fn parse_claims_unverified(id_token: &str) -> Result<Map<String, Value>, IdTokenError> {
    let mut segments = id_token.split('.');
    let (Some(_header), Some(claims), Some(_signature), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return Err(IdTokenError::NotAJwt);
    };

    // JWT (RFC 7515 §2) mandates base64url with the padding stripped. Some
    // providers pad anyway; accepting both costs nothing and refusing would
    // reject a token that is otherwise entirely valid.
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(claims.trim_end_matches('='))
        .map_err(|_| IdTokenError::ClaimsNotBase64Url)?;

    match serde_json::from_slice::<Value>(&decoded) {
        Ok(Value::Object(map)) => Ok(map),
        _ => Err(IdTokenError::ClaimsNotJsonObject),
    }
}

/// The `sub` claim: the provider's stable, unique identifier for the user.
///
/// This is the fallback that makes OpenID Connect providers work at all when
/// they expose no userinfo endpoint, or expose one that omits the subject.
/// Before this existed the caller had a stub returning `None`, so such a
/// provider produced "Could not extract provider user id" on every login and
/// nobody could sign in with it.
///
/// Only `sub` is accepted. `email` is not an identifier — it is reassignable,
/// and treating it as one lets a provider hand a departed employee's address
/// to someone new who then inherits their ThunderForge account.
pub fn subject_from_id_token_unverified(id_token: &str) -> Option<String> {
    let claims = parse_claims_unverified(id_token).ok()?;
    claims
        .get("sub")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|sub| !sub.is_empty())
}

/// The `email` claim, when the provider puts one in the ID token.
///
/// Used only to pre-fill and to match an existing account by address; the
/// identity itself always comes from `sub`.
pub fn email_from_id_token_unverified(id_token: &str) -> Option<String> {
    let claims = parse_claims_unverified(id_token).ok()?;
    claims
        .get("email")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|email| !email.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn jwt(claims: &serde_json::Value) -> String {
        let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
        format!(
            "{}.{}.{}",
            b64(br#"{"alg":"RS256","typ":"JWT"}"#),
            b64(claims.to_string().as_bytes()),
            b64(b"not-a-real-signature"),
        )
    }

    #[test]
    fn reads_the_subject_a_provider_only_puts_in_the_id_token() {
        let token = jwt(&serde_json::json!({
            "iss": "https://idp.example.com",
            "sub": "248289761001",
            "aud": "client-123",
        }));
        assert_eq!(
            subject_from_id_token_unverified(&token),
            Some("248289761001".to_string())
        );
    }

    #[test]
    fn padded_claims_are_accepted() {
        let claims = base64::engine::general_purpose::URL_SAFE.encode(r#"{"sub":"abc"}"#);
        let token = format!("h.{claims}.s");
        assert_eq!(
            subject_from_id_token_unverified(&token),
            Some("abc".to_string())
        );
    }

    #[test]
    fn a_numeric_subject_is_not_silently_stringified() {
        // OpenID Connect Core §2 requires `sub` to be a string. A provider
        // sending a bare number is misbehaving, and guessing at what they
        // meant would let `1` and `"1"` become the same account.
        let token = jwt(&serde_json::json!({ "sub": 1 }));
        assert_eq!(subject_from_id_token_unverified(&token), None);
    }

    #[test]
    fn an_empty_subject_is_no_subject() {
        let token = jwt(&serde_json::json!({ "sub": "" }));
        assert_eq!(subject_from_id_token_unverified(&token), None);
    }

    #[test]
    fn the_segment_count_is_checked_in_both_directions() {
        assert_eq!(parse_claims_unverified("a.b"), Err(IdTokenError::NotAJwt));
        assert_eq!(
            parse_claims_unverified("a.b.c.d"),
            Err(IdTokenError::NotAJwt)
        );
    }

    #[test]
    fn claims_that_are_json_but_not_an_object_are_refused() {
        let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b"[1,2,3]");
        assert_eq!(
            parse_claims_unverified(&format!("h.{claims}.s")),
            Err(IdTokenError::ClaimsNotJsonObject)
        );
    }

    proptest! {
        /// **Totality over arbitrary bytes.** This is the property that
        /// matters most in this crate: the input is a string a third party
        /// chose, and the parser has to answer rather than abort.
        #[test]
        fn parsing_never_panics_on_arbitrary_text(token in ".{0,256}") {
            let _ = parse_claims_unverified(&token);
            let _ = subject_from_id_token_unverified(&token);
            let _ = email_from_id_token_unverified(&token);
        }

        /// Including input shaped like a JWT, which gets past the cheap
        /// segment check and into the decoder.
        #[test]
        fn parsing_never_panics_on_jwt_shaped_garbage(
            header in "[A-Za-z0-9_=-]{0,32}",
            claims in "[A-Za-z0-9_=-]{0,64}",
            signature in "[A-Za-z0-9_=-]{0,32}",
        ) {
            let token = format!("{header}.{claims}.{signature}");
            let _ = subject_from_id_token_unverified(&token);
        }

        /// And on arbitrary *bytes* in the claims segment, so a token whose
        /// claims decode to invalid UTF-8 is an `Err` and not a crash.
        #[test]
        fn parsing_never_panics_on_arbitrary_claim_bytes(bytes in proptest::collection::vec(any::<u8>(), 0..128)) {
            let claims = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes);
            let _ = parse_claims_unverified(&format!("h.{claims}.s"));
        }

        /// Whatever string subject a provider puts in, that exact string
        /// comes back — no trimming, no case folding. `sub` is an opaque
        /// identifier and normalising it would merge two accounts.
        #[test]
        fn a_string_subject_round_trips_verbatim(sub in ".{1,64}") {
            let token = jwt(&serde_json::json!({ "sub": sub }));
            prop_assert_eq!(subject_from_id_token_unverified(&token), Some(sub));
        }

        /// A token with no `sub` yields nothing, whatever else it carries.
        #[test]
        fn claims_without_a_subject_yield_nothing(
            iss in ".{0,32}",
            email in ".{0,32}",
        ) {
            let token = jwt(&serde_json::json!({ "iss": iss, "email": email }));
            prop_assert_eq!(subject_from_id_token_unverified(&token), None);
        }
    }
}
