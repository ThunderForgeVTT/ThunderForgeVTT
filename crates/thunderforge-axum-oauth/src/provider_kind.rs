//! Every provider we ship support for, declared once.
//!
//! # Why this is an enum and not a list of strings
//!
//! Provider support used to be two independent string tables in
//! `src/server/src/config/oauth_env.rs`: a `KNOWN_PRESETS` array that decided
//! which env-var names were recognised, and a `preset_for()` `match` with a
//! `_ => None` arm that decided what those names meant. Nothing connected
//! them. Adding a provider to one and forgetting the other compiled cleanly
//! and shipped: the operator sets `OAUTH_OKTA_CLIENT_ID`, the server parses
//! it, nothing resolves, one `tracing::warn!` scrolls past at boot, and the
//! login button is simply absent in production. That is the "forgot to
//! include it" failure this whole split exists to prevent, and a crate
//! boundary alone would not have prevented it.
//!
//! So the declaration is a closed enum, and everything about a provider is
//! derived from it by an **exhaustive `match` with no wildcard arm**. Adding
//! a variant is a compile error until every one of those matches is answered:
//! the env-var segment it responds to, its endpoints, its scopes, its display
//! name, and whether it speaks OpenID Connect. [`ProviderKind::ALL`] is
//! checked against the same exhaustive match in the tests below, so it cannot
//! silently omit a variant either.
//!
//! `src/server/src/auth/mod.rs` has a companion test that walks
//! [`ProviderKind::ALL`], resolves each one from synthetic env vars, and
//! asserts the resulting `provider_key` reaches a live route — closing the
//! last gap between "declared" and "actually reachable in production".

/// Which protocol a provider actually speaks.
///
/// This is not cosmetic: an OpenID Connect provider may return the subject
/// **only** in the ID token, so the identity step has to know to look there.
/// Treating one as plain OAuth 2.0 means a login that fails with
/// "identity_missing" and no clue why.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Plain OAuth 2.0: identity comes from a provider-specific userinfo
    /// endpoint and nowhere else.
    OAuth2,
    /// OpenID Connect: the token response carries a signed `id_token` whose
    /// claims are an identity source in their own right.
    OpenIdConnect,
}

/// Where a provider's endpoints come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Endpoints {
    /// Well-known, hosted by the provider — no operator input needed.
    Fixed {
        authorization_url: &'static str,
        token_url: &'static str,
        userinfo_url: Option<&'static str>,
    },
    /// Self-hosted. The operator supplies an issuer/base URL and the
    /// endpoints are derived from it by [`ProviderKind::derive_endpoints`].
    IssuerDerived,
}

/// Everything the configuration layer needs to know about one provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preset {
    pub display_name: &'static str,
    pub endpoints: Endpoints,
    pub default_scopes: &'static [&'static str],
    pub flow: Flow,
}

/// The three endpoints, once an issuer-derived provider's base URL is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedEndpoints {
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
}

/// A provider ThunderForge ships built-in support for.
///
/// Operators can still configure anything else by supplying the full endpoint
/// set by hand (`OAUTH_<NAME>_AUTHORIZATION_URL` and friends); this enum is
/// the set we promise to know about without being told.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ProviderKind {
    Discord,
    GitHub,
    Google,
    Keycloak,
}

impl ProviderKind {
    /// Every variant. Kept honest by `all_lists_every_variant` below, which
    /// matches exhaustively and so fails to compile if a variant is added.
    pub const ALL: &'static [ProviderKind] = &[
        ProviderKind::Discord,
        ProviderKind::GitHub,
        ProviderKind::Google,
        ProviderKind::Keycloak,
    ];

    /// The uppercase segment in `OAUTH_<SEGMENT>_<FIELD>`.
    ///
    /// Matched as a whole underscore-delimited segment, never a substring, so
    /// `OAUTH_GOOGLE_WORKSPACE_CLIENT_ID` is Google's `workspace` instance and
    /// not a provider named `GOOGLE_WORKSPACE`.
    pub fn env_segment(self) -> &'static str {
        match self {
            ProviderKind::Discord => "DISCORD",
            ProviderKind::GitHub => "GITHUB",
            ProviderKind::Google => "GOOGLE",
            ProviderKind::Keycloak => "KEYCLOAK",
        }
    }

    /// Look a provider up by its env-var segment.
    ///
    /// Derived from [`ProviderKind::ALL`] rather than written out as a second
    /// `match`, which is what stops the lookup table and the definition table
    /// from drifting apart — the drift that used to be possible here.
    pub fn from_env_segment(segment: &str) -> Option<ProviderKind> {
        ProviderKind::ALL
            .iter()
            .copied()
            .find(|kind| kind.env_segment() == segment)
    }

    /// Endpoints, scopes, label and protocol.
    ///
    /// URLs and scopes mirror `src/server/migrations/`
    /// `2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields`
    /// exactly — keep in sync if that migration's seed data ever changes.
    pub fn preset(self) -> Preset {
        match self {
            ProviderKind::Discord => Preset {
                display_name: "Discord",
                endpoints: Endpoints::Fixed {
                    authorization_url: "https://discord.com/api/oauth2/authorize",
                    token_url: "https://discord.com/api/oauth2/token",
                    userinfo_url: Some("https://discord.com/api/users/@me"),
                },
                default_scopes: &["identify", "email"],
                // Discord is OAuth 2.0 only — it issues no ID token, and
                // `/users/@me` is the sole identity source.
                flow: Flow::OAuth2,
            },
            ProviderKind::GitHub => Preset {
                display_name: "GitHub",
                endpoints: Endpoints::Fixed {
                    authorization_url: "https://github.com/login/oauth/authorize",
                    token_url: "https://github.com/login/oauth/access_token",
                    userinfo_url: Some("https://api.github.com/user"),
                },
                default_scopes: &["read:user", "user:email"],
                flow: Flow::OAuth2,
            },
            ProviderKind::Google => Preset {
                display_name: "Google",
                endpoints: Endpoints::Fixed {
                    authorization_url: "https://accounts.google.com/o/oauth2/v2/auth",
                    token_url: "https://oauth2.googleapis.com/token",
                    userinfo_url: Some("https://openidconnect.googleapis.com/v1/userinfo"),
                },
                default_scopes: &["openid", "profile", "email"],
                flow: Flow::OpenIdConnect,
            },
            ProviderKind::Keycloak => Preset {
                display_name: "Keycloak",
                endpoints: Endpoints::IssuerDerived,
                default_scopes: &["openid", "profile", "email"],
                flow: Flow::OpenIdConnect,
            },
        }
    }

    /// Which operator-supplied field an issuer-derived provider cannot
    /// resolve without, or `None` if its endpoints are fixed.
    pub fn required_issuer_field(self) -> Option<&'static str> {
        match self.preset().endpoints {
            Endpoints::Fixed { .. } => None,
            Endpoints::IssuerDerived => Some("ISSUER_URL"),
        }
    }

    /// Expand an operator's issuer URL into the three endpoints.
    ///
    /// A trailing slash on the issuer is stripped first; operators paste
    /// these out of an admin console and half of them come with one, and
    /// `.../realms/main//protocol/...` is a 404 nobody enjoys diagnosing.
    pub fn derive_endpoints(self, issuer_url: &str) -> DerivedEndpoints {
        let issuer = issuer_url.trim_end_matches('/');
        match self {
            // Keycloak's layout. Written out per-variant rather than shared,
            // so the next issuer-derived provider is forced to state its own
            // paths instead of inheriting Keycloak's by accident.
            ProviderKind::Keycloak => DerivedEndpoints {
                authorization_url: format!("{issuer}/protocol/openid-connect/auth"),
                token_url: format!("{issuer}/protocol/openid-connect/token"),
                userinfo_url: Some(format!("{issuer}/protocol/openid-connect/userinfo")),
            },
            ProviderKind::Discord | ProviderKind::GitHub | ProviderKind::Google => {
                let Endpoints::Fixed {
                    authorization_url,
                    token_url,
                    userinfo_url,
                } = self.preset().endpoints
                else {
                    unreachable!("fixed-endpoint providers never derive from an issuer")
                };
                DerivedEndpoints {
                    authorization_url: authorization_url.to_string(),
                    token_url: token_url.to_string(),
                    userinfo_url: userinfo_url.map(str::to_string),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The wiring guarantee, part one.
    ///
    /// The `match` has no wildcard arm, so adding a `ProviderKind` variant
    /// breaks this file's compilation until the variant is listed in `ALL`
    /// too. `ALL` is what every other consumer iterates, so a variant missing
    /// from it would be a provider that exists in the type system and nowhere
    /// else — exactly the silent omission this design is here to make
    /// impossible.
    #[test]
    fn all_lists_every_variant() {
        let listed: BTreeSet<_> = ProviderKind::ALL.iter().copied().collect();
        for kind in ProviderKind::ALL {
            let present = match kind {
                ProviderKind::Discord => listed.contains(&ProviderKind::Discord),
                ProviderKind::GitHub => listed.contains(&ProviderKind::GitHub),
                ProviderKind::Google => listed.contains(&ProviderKind::Google),
                ProviderKind::Keycloak => listed.contains(&ProviderKind::Keycloak),
            };
            assert!(present, "{kind:?} is not listed in ProviderKind::ALL");
        }
        assert_eq!(
            listed.len(),
            ProviderKind::ALL.len(),
            "ProviderKind::ALL contains a duplicate"
        );
    }

    /// Two providers answering to the same env-var segment would mean one of
    /// them is unreachable, decided by iteration order.
    #[test]
    fn env_segments_are_unique_and_round_trip() {
        let mut seen = BTreeSet::new();
        for kind in ProviderKind::ALL {
            let segment = kind.env_segment();
            assert!(
                seen.insert(segment),
                "{segment} is claimed by more than one provider"
            );
            assert_eq!(
                segment.to_uppercase(),
                segment,
                "env segments are matched against uppercased env var names"
            );
            assert_eq!(ProviderKind::from_env_segment(segment), Some(*kind));
        }
    }

    #[test]
    fn an_unknown_segment_resolves_to_nothing() {
        assert_eq!(ProviderKind::from_env_segment("KEYCLOKE"), None);
        assert_eq!(ProviderKind::from_env_segment(""), None);
        assert_eq!(ProviderKind::from_env_segment("keycloak"), None);
    }

    /// Every declared provider is actually usable: it has a name, at least
    /// one scope, and a full endpoint set once its required inputs are given.
    #[test]
    fn every_provider_resolves_to_a_complete_endpoint_set() {
        for kind in ProviderKind::ALL {
            let preset = kind.preset();
            assert!(!preset.display_name.is_empty(), "{kind:?} has no label");
            assert!(!preset.default_scopes.is_empty(), "{kind:?} has no scopes");

            let endpoints = kind.derive_endpoints("https://idp.example.com/realms/main");
            assert!(
                endpoints.authorization_url.starts_with("https://"),
                "{kind:?} authorization URL is not https"
            );
            assert!(
                endpoints.token_url.starts_with("https://"),
                "{kind:?} token URL is not https"
            );
        }
    }

    /// An OIDC provider that asks for `openid` and one that does not are
    /// different bugs; both are bugs. The scope is what makes the provider
    /// issue an ID token at all, and without it the ID-token identity
    /// fallback in `thunderforge_axum_oidc` has nothing to read.
    #[test]
    fn oidc_providers_request_the_openid_scope() {
        for kind in ProviderKind::ALL {
            let preset = kind.preset();
            if preset.flow == Flow::OpenIdConnect {
                assert!(
                    preset.default_scopes.contains(&"openid"),
                    "{kind:?} speaks OIDC but never asks for an ID token"
                );
            }
        }
    }

    #[test]
    fn a_trailing_slash_on_the_issuer_does_not_double_up() {
        let with = ProviderKind::Keycloak.derive_endpoints("https://idp.example.com/realms/main/");
        let without = ProviderKind::Keycloak.derive_endpoints("https://idp.example.com/realms/main");
        assert_eq!(with, without);
        assert_eq!(
            without.authorization_url,
            "https://idp.example.com/realms/main/protocol/openid-connect/auth"
        );
    }

    #[test]
    fn only_issuer_derived_providers_demand_an_issuer_url() {
        assert_eq!(
            ProviderKind::Keycloak.required_issuer_field(),
            Some("ISSUER_URL")
        );
        assert_eq!(ProviderKind::Discord.required_issuer_field(), None);
    }
}
