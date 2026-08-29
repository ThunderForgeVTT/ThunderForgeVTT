//! The test that says a declared provider is actually reachable.
//!
//! # What this exists to catch
//!
//! Splitting the auth code into crates makes it testable. It does **not**, on
//! its own, make it hard to forget to wire a provider up — and that is the
//! failure this repository has already had. `extract_provider_user_id_from_token`
//! shipped as `let _ = token; None`, a stub that made any OpenID Connect
//! provider publishing its subject only in the ID token unable to log anybody
//! in; the provider configured fine, redirected fine, exchanged its code fine,
//! and then failed with "identity_missing" on every attempt.
//!
//! `thunderforge_axum_oauth::provider_kind` closes half of that with the type
//! system: `ProviderKind` is a closed enum and everything about a provider is
//! derived from it by an exhaustive `match`, so adding a variant is a compile
//! error until every question about it is answered. The compiler cannot check
//! the other half — that the answers add up to a working route in a running
//! server — so this module walks `ProviderKind::ALL` and drives each one all
//! the way from env var to HTTP redirect against a real database.
//!
//! Together: a new provider cannot compile without being described, and
//! cannot pass CI without being reachable.

use super::{extract_provider_user_id_from_token, router};
use crate::config::oauth_env::{parse_oauth_env_vars, resolve};
use crate::models::NewOAuthProvider;
use crate::schema::oauth_providers;
use crate::test_support::test_app_state;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use chrono::Utc;
use diesel::prelude::*;
use thunderforge_axum_oauth::provider_kind::{Flow, ProviderKind};
use thunderforge_axum_oauth::token::TokenResponse;
use tower::ServiceExt;

/// The env vars an operator would set for this provider, and nothing more.
///
/// Built from the provider's own declaration rather than a hand-written list,
/// so a provider that starts requiring a new field gets that field here
/// automatically instead of this test quietly configuring it wrong.
fn env_vars_for(kind: ProviderKind, unique: &str) -> Vec<(String, String)> {
    let segment = kind.env_segment();
    let mut vars = vec![
        (
            format!("OAUTH_{segment}_{unique}_CLIENT_ID"),
            "test-client-id".to_string(),
        ),
        (
            format!("OAUTH_{segment}_{unique}_CLIENT_SECRET"),
            "test-client-secret".to_string(),
        ),
    ];
    if let Some(field) = kind.required_issuer_field() {
        vars.push((
            format!("OAUTH_{segment}_{unique}_{field}"),
            "https://idp.example.invalid/realms/main".to_string(),
        ));
    }
    vars
}

/// Every provider we ship reaches a live authorization redirect.
///
/// This is the whole flow a first-time operator takes: set two env vars, boot,
/// click the login button. It goes env var -> `parse_oauth_env_vars` ->
/// `resolve` -> an `oauth_providers` row -> `GET
/// /authentication/oauth/{provider_key}/start` -> a 307 at the provider. A
/// break anywhere along that chain — a preset missing an endpoint, a provider
/// name the parser does not recognise, a route that was never added — fails
/// here and names the provider.
#[tokio::test]
async fn every_declared_provider_reaches_a_live_authorization_route() {
    let state = test_app_state();
    // Uppercase and free of `_`, so it survives the env-var parser's
    // provider/instance split as a single instance key.
    let unique = uuid::Uuid::now_v7().simple().to_string().to_uppercase();
    let unique = &unique[..12];

    for kind in ProviderKind::ALL {
        let parsed = parse_oauth_env_vars(env_vars_for(*kind, unique).into_iter());
        assert_eq!(
            parsed.len(),
            1,
            "{kind:?}: its own env vars did not parse into one provider instance",
        );
        let resolved = resolve(&parsed[0]).unwrap_or_else(|missing| {
            panic!(
                "{kind:?}: declared but not configurable — missing {}",
                missing.field
            )
        });

        let now = Utc::now().naive_utc();
        let mut conn = state.db_pool.get().expect("failed to get DB connection");
        diesel::insert_into(oauth_providers::table)
            .values(&NewOAuthProvider {
                id: uuid::Uuid::now_v7(),
                provider_key: resolved.provider_key.clone(),
                display_name: resolved.display_name.clone(),
                authorization_url: resolved.authorization_url.clone(),
                token_url: resolved.token_url.clone(),
                userinfo_url: resolved.userinfo_url.clone(),
                scopes: resolved.scopes.iter().cloned().map(Some).collect(),
                oauth_client_id: Some(resolved.client_id.clone()),
                oauth_client_secret: Some(resolved.client_secret.clone()),
                configured: true,
                enabled: true,
                created_at: now,
                updated_at: now,
                config_source: "env".to_string(),
            })
            .execute(&mut conn)
            .unwrap_or_else(|e| panic!("{kind:?}: failed to materialize provider row: {e}"));

        let request = Request::builder()
            .uri(format!(
                "/authentication/oauth/{}/start?redirect_uri=https%3A%2F%2Fvtt.example.invalid%2Fcallback",
                resolved.provider_key
            ))
            .body(Body::empty())
            .expect("request must build");

        let response = router()
            .with_state(state.clone())
            .oneshot(request)
            .await
            .expect("router must answer");

        assert_eq!(
            response.status(),
            StatusCode::TEMPORARY_REDIRECT,
            "{kind:?}: no live authorization route for provider_key {}",
            resolved.provider_key,
        );

        let location = response
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_else(|| panic!("{kind:?}: redirect carried no Location"))
            .to_string();

        assert!(
            location.starts_with(&resolved.authorization_url),
            "{kind:?}: redirected to {location}, not to its own authorization endpoint",
        );
        // The parameters without which the provider will refuse, or the
        // callback will be unusable when it comes back.
        for param in [
            "response_type=code",
            "client_id=test-client-id",
            "state=",
            "code_challenge=",
            "code_challenge_method=S256",
        ] {
            assert!(
                location.contains(param),
                "{kind:?}: authorization URL is missing {param}",
            );
        }

        diesel::delete(
            oauth_providers::table.filter(oauth_providers::provider_key.eq(&resolved.provider_key)),
        )
        .execute(&mut conn)
        .expect("failed to clean up test provider row");
    }
}

/// Every OpenID Connect provider can be identified from its ID token alone.
///
/// The precise gap that shipped: a provider whose userinfo endpoint yields no
/// subject (or that has none) fell through to
/// `extract_provider_user_id_from_token`, which returned `None` unconditionally.
/// Declaring a provider `Flow::OpenIdConnect` is a promise that this path
/// works, and this is where the promise is checked.
#[test]
fn every_oidc_provider_can_be_identified_from_its_id_token() {
    let b64 = |bytes: &[u8]| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    let id_token = format!(
        "{}.{}.{}",
        b64(br#"{"alg":"RS256"}"#),
        b64(br#"{"iss":"https://idp.example.invalid","sub":"provider-subject-1"}"#),
        b64(b"unverified-signature"),
    );

    for kind in ProviderKind::ALL {
        if kind.preset().flow != Flow::OpenIdConnect {
            continue;
        }
        let token = TokenResponse {
            access_token: "at".to_string(),
            id_token: Some(id_token.clone()),
            ..TokenResponse::default()
        };
        assert_eq!(
            extract_provider_user_id_from_token(&token),
            Some("provider-subject-1".to_string()),
            "{kind:?} speaks OIDC but its subject cannot be read from the ID token",
        );
    }
}

/// A provider that issues no ID token is still identified the old way, and
/// the fallback stays quiet rather than inventing a subject.
#[test]
fn a_token_response_without_an_id_token_yields_no_subject() {
    let token = TokenResponse {
        access_token: "at".to_string(),
        ..TokenResponse::default()
    };
    assert_eq!(extract_provider_user_id_from_token(&token), None);
}
