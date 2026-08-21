//! Environment-variable OAuth provider configuration (ADR-041, spec 007).
//!
//! Parses `OAUTH_<PROVIDER>_[<INSTANCE>_]<FIELD>` env vars into resolved
//! provider-instance configs ready to be materialized into `oauth_providers`
//! rows by `crate::auth::materialize_env_oauth_providers` (research.md §3).

use std::collections::BTreeMap;

/// Preset provider names recognized by [`preset_for`]. Matched as a whole
/// underscore-delimited segment (never a substring) against the first
/// segment remaining after stripping a known field suffix — see
/// [`split_provider_instance`].
const KNOWN_PRESETS: &[&str] = &["DISCORD", "GITHUB", "GOOGLE", "KEYCLOAK"];

/// Field suffixes recognized on any `OAUTH_<PROVIDER>_[<INSTANCE>_]<FIELD>`
/// env var. None is a suffix of another, so match order doesn't affect
/// correctness.
const FIELD_SUFFIXES: &[&str] = &[
    "AUTHORIZATION_URL",
    "TOKEN_URL",
    "USERINFO_URL",
    "CLIENT_SECRET",
    "CLIENT_ID",
    "ISSUER_URL",
    "SCOPES",
    "LABEL",
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedInstanceFields {
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub userinfo_url: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub issuer_url: Option<String>,
    pub scopes: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedProviderInstance {
    /// Uppercase preset name (e.g. `"KEYCLOAK"`) or the verbatim generic
    /// provider name an operator chose (e.g. `"MYSERVICE"`).
    pub provider: String,
    /// Lowercased instance key, or empty string for the default/unnamed
    /// instance. Always empty for a generic (non-preset) provider — a
    /// generic provider's whole name is already the unique identifier;
    /// running two generic instances just means picking two different names.
    pub instance: String,
    pub fields: ParsedInstanceFields,
}

/// Parses all `OAUTH_*` env vars into grouped per-instance field sets.
/// Empty-string values are treated as unset (never produce a group on their
/// own), matching this repo's other `env::var().ok()` conventions.
pub fn parse_oauth_env_vars(
    vars: impl Iterator<Item = (String, String)>,
) -> Vec<ParsedProviderInstance> {
    let mut groups: BTreeMap<(String, String), ParsedInstanceFields> = BTreeMap::new();

    for (key, value) in vars {
        let Some(rest) = key.strip_prefix("OAUTH_") else {
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        let Some((field, remaining)) = match_field_suffix(rest) else {
            continue;
        };
        let (provider, instance) = split_provider_instance(remaining);

        let entry = groups.entry((provider, instance)).or_default();
        match field {
            "AUTHORIZATION_URL" => entry.authorization_url = Some(value),
            "TOKEN_URL" => entry.token_url = Some(value),
            "USERINFO_URL" => entry.userinfo_url = Some(value),
            "CLIENT_SECRET" => entry.client_secret = Some(value),
            "CLIENT_ID" => entry.client_id = Some(value),
            "ISSUER_URL" => entry.issuer_url = Some(value),
            "SCOPES" => entry.scopes = Some(value),
            "LABEL" => entry.label = Some(value),
            _ => unreachable!("match_field_suffix only returns known FIELD_SUFFIXES entries"),
        }
    }

    groups
        .into_iter()
        .map(|((provider, instance), fields)| ParsedProviderInstance {
            provider,
            instance,
            fields,
        })
        .collect()
}

fn match_field_suffix(rest: &str) -> Option<(&'static str, &str)> {
    for suffix in FIELD_SUFFIXES {
        if let Some(remaining) = rest.strip_suffix(suffix).and_then(|r| r.strip_suffix('_')) {
            return Some((suffix, remaining));
        }
    }
    None
}

/// Splits the env-var remainder (after stripping `OAUTH_` and the field
/// suffix) into `(provider, instance)`. If the first `_`-delimited segment
/// matches a known preset name exactly, everything after it is the
/// (possibly empty) instance key, lowercased. Otherwise the whole remainder
/// is treated as a generic provider's name, with no instance key.
fn split_provider_instance(remaining: &str) -> (String, String) {
    let mut segments = remaining.splitn(2, '_');
    if let Some(first) = segments.next() {
        if KNOWN_PRESETS.contains(&first) {
            let instance = segments.next().unwrap_or("").to_lowercase();
            return (first.to_string(), instance);
        }
    }
    (remaining.to_string(), String::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PresetKind {
    /// Fixed, well-known endpoints — no issuer URL needed.
    Fixed {
        authorization_url: &'static str,
        token_url: &'static str,
        userinfo_url: Option<&'static str>,
        default_scopes: &'static [&'static str],
    },
    /// Self-hosted, OIDC-discovery-shaped preset — endpoints are derived
    /// from an operator-supplied issuer/base URL (data-model.md).
    IssuerDerived {
        default_scopes: &'static [&'static str],
    },
}

struct PresetTemplate {
    display_name: &'static str,
    kind: PresetKind,
}

fn preset_for(provider: &str) -> Option<PresetTemplate> {
    match provider {
        // URLs/scopes mirror src/server/migrations/
        // 2026-05-02-021115-0002_seed_oauth_providers_and_credentials_fields
        // exactly (research.md §2) — keep in sync if that migration's seed
        // data ever changes.
        "DISCORD" => Some(PresetTemplate {
            display_name: "Discord",
            kind: PresetKind::Fixed {
                authorization_url: "https://discord.com/api/oauth2/authorize",
                token_url: "https://discord.com/api/oauth2/token",
                userinfo_url: Some("https://discord.com/api/users/@me"),
                default_scopes: &["identify", "email"],
            },
        }),
        "GOOGLE" => Some(PresetTemplate {
            display_name: "Google",
            kind: PresetKind::Fixed {
                authorization_url: "https://accounts.google.com/o/oauth2/v2/auth",
                token_url: "https://oauth2.googleapis.com/token",
                userinfo_url: Some("https://openidconnect.googleapis.com/v1/userinfo"),
                default_scopes: &["openid", "profile", "email"],
            },
        }),
        "GITHUB" => Some(PresetTemplate {
            display_name: "GitHub",
            kind: PresetKind::Fixed {
                authorization_url: "https://github.com/login/oauth/authorize",
                token_url: "https://github.com/login/oauth/access_token",
                userinfo_url: Some("https://api.github.com/user"),
                default_scopes: &["read:user", "user:email"],
            },
        }),
        "KEYCLOAK" => Some(PresetTemplate {
            display_name: "Keycloak",
            kind: PresetKind::IssuerDerived {
                default_scopes: &["openid", "profile", "email"],
            },
        }),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProviderInstance {
    /// Compound identifier for a named instance (`<provider>__<instance>`,
    /// lowercase), or just `<provider>` for the default instance — slots
    /// directly into `oauth_providers.provider_key` (research.md §4).
    pub provider_key: String,
    pub display_name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub scopes: Vec<String>,
    pub client_id: String,
    pub client_secret: String,
}

/// Why a candidate `OAUTH_*` env-var group could not be resolved into a
/// working provider instance — logged (FR-010) and skipped, never a crash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingField {
    pub provider: String,
    pub instance: String,
    pub field: &'static str,
}

/// Resolves one parsed env-var group into a materializable provider
/// instance, filling in preset endpoints/scopes where applicable.
pub fn resolve(parsed: &ParsedProviderInstance) -> Result<ResolvedProviderInstance, MissingField> {
    let missing = |field: &'static str| MissingField {
        provider: parsed.provider.clone(),
        instance: parsed.instance.clone(),
        field,
    };

    let client_id = parsed.fields.client_id.clone().ok_or_else(|| missing("CLIENT_ID"))?;
    let client_secret = parsed
        .fields
        .client_secret
        .clone()
        .ok_or_else(|| missing("CLIENT_SECRET"))?;

    let (authorization_url, token_url, userinfo_url, scopes, default_display_name) =
        if let Some(preset) = preset_for(&parsed.provider) {
            match preset.kind {
                PresetKind::Fixed {
                    authorization_url,
                    token_url,
                    userinfo_url,
                    default_scopes,
                } => (
                    authorization_url.to_string(),
                    token_url.to_string(),
                    userinfo_url.map(str::to_string),
                    default_scopes.iter().map(|s| s.to_string()).collect(),
                    preset.display_name.to_string(),
                ),
                PresetKind::IssuerDerived { default_scopes } => {
                    let issuer = parsed
                        .fields
                        .issuer_url
                        .clone()
                        .ok_or_else(|| missing("ISSUER_URL"))?;
                    let issuer = issuer.trim_end_matches('/');
                    (
                        format!("{issuer}/protocol/openid-connect/auth"),
                        format!("{issuer}/protocol/openid-connect/token"),
                        Some(format!("{issuer}/protocol/openid-connect/userinfo")),
                        default_scopes.iter().map(|s| s.to_string()).collect(),
                        preset.display_name.to_string(),
                    )
                }
            }
        } else {
            // Generic/unlisted provider (research.md §4's edge-case rule):
            // only resolvable when the full endpoint set is present.
            let authorization_url = parsed
                .fields
                .authorization_url
                .clone()
                .ok_or_else(|| missing("AUTHORIZATION_URL"))?;
            let token_url = parsed
                .fields
                .token_url
                .clone()
                .ok_or_else(|| missing("TOKEN_URL"))?;
            let scopes = parsed
                .fields
                .scopes
                .as_deref()
                .map(|s| s.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            (
                authorization_url,
                token_url,
                parsed.fields.userinfo_url.clone(),
                scopes,
                title_case(&parsed.provider),
            )
        };

    let display_name = parsed.fields.label.clone().unwrap_or_else(|| {
        if parsed.instance.is_empty() {
            default_display_name
        } else {
            format!("{default_display_name} ({})", title_case(&parsed.instance))
        }
    });

    let provider_key = if parsed.instance.is_empty() {
        parsed.provider.to_lowercase()
    } else {
        format!("{}__{}", parsed.provider.to_lowercase(), parsed.instance)
    };

    Ok(ResolvedProviderInstance {
        provider_key,
        display_name,
        authorization_url,
        token_url,
        userinfo_url,
        scopes,
        client_id,
        client_secret,
    })
}

fn title_case(s: &str) -> String {
    s.split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn parses_default_instance() {
        let parsed = parse_oauth_env_vars(
            vars(&[
                ("OAUTH_KEYCLOAK_CLIENT_ID", "abc"),
                ("OAUTH_KEYCLOAK_CLIENT_SECRET", "shh"),
                ("OAUTH_KEYCLOAK_ISSUER_URL", "https://idp.example.com/realms/main"),
                ("UNRELATED_VAR", "ignored"),
            ])
            .into_iter(),
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].provider, "KEYCLOAK");
        assert_eq!(parsed[0].instance, "");
        assert_eq!(parsed[0].fields.client_id.as_deref(), Some("abc"));

        let resolved = resolve(&parsed[0]).expect("should resolve");
        assert_eq!(resolved.provider_key, "keycloak");
        assert_eq!(resolved.display_name, "Keycloak");
        assert_eq!(
            resolved.authorization_url,
            "https://idp.example.com/realms/main/protocol/openid-connect/auth"
        );
    }

    #[test]
    fn parses_named_instance() {
        let parsed = parse_oauth_env_vars(
            vars(&[
                ("OAUTH_KEYCLOAK_WORK_CLIENT_ID", "abc"),
                ("OAUTH_KEYCLOAK_WORK_CLIENT_SECRET", "shh"),
                ("OAUTH_KEYCLOAK_WORK_ISSUER_URL", "https://work.example.com/realms/main"),
                ("OAUTH_KEYCLOAK_WORK_LABEL", "Work SSO"),
            ])
            .into_iter(),
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].provider, "KEYCLOAK");
        assert_eq!(parsed[0].instance, "work");

        let resolved = resolve(&parsed[0]).expect("should resolve");
        assert_eq!(resolved.provider_key, "keycloak__work");
        assert_eq!(resolved.display_name, "Work SSO");
    }

    #[test]
    fn generic_provider_requires_full_endpoint_set() {
        let parsed = parse_oauth_env_vars(
            vars(&[
                ("OAUTH_MYSERVICE_CLIENT_ID", "abc"),
                ("OAUTH_MYSERVICE_CLIENT_SECRET", "shh"),
            ])
            .into_iter(),
        );
        assert_eq!(parsed.len(), 1);
        let err = resolve(&parsed[0]).expect_err("missing AUTHORIZATION_URL should fail");
        assert_eq!(err.field, "AUTHORIZATION_URL");
    }

    #[test]
    fn generic_provider_resolves_with_full_endpoint_set() {
        let parsed = parse_oauth_env_vars(
            vars(&[
                ("OAUTH_MYSERVICE_CLIENT_ID", "abc"),
                ("OAUTH_MYSERVICE_CLIENT_SECRET", "shh"),
                ("OAUTH_MYSERVICE_AUTHORIZATION_URL", "https://myservice.example/auth"),
                ("OAUTH_MYSERVICE_TOKEN_URL", "https://myservice.example/token"),
            ])
            .into_iter(),
        );
        let resolved = resolve(&parsed[0]).expect("should resolve");
        assert_eq!(resolved.provider_key, "myservice");
        assert_eq!(resolved.display_name, "Myservice");
    }

    #[test]
    fn unrecognized_partial_config_is_missing_field_not_panic() {
        // A typo'd/unrecognized preset name with only partial fields must
        // surface as a MissingField, never panic (FR-010).
        let parsed = parse_oauth_env_vars(vars(&[("OAUTH_KEYCLOKE_CLIENT_ID", "abc")]).into_iter());
        assert_eq!(parsed.len(), 1);
        let err = resolve(&parsed[0]).unwrap_err();
        assert_eq!(err.field, "CLIENT_SECRET");
    }

    #[test]
    fn empty_value_is_treated_as_unset() {
        let parsed = parse_oauth_env_vars(vars(&[("OAUTH_DISCORD_CLIENT_ID", "")]).into_iter());
        assert!(parsed.is_empty());
    }

    #[test]
    fn discord_preset_resolves_without_issuer_url() {
        let parsed = parse_oauth_env_vars(
            vars(&[
                ("OAUTH_DISCORD_CLIENT_ID", "abc"),
                ("OAUTH_DISCORD_CLIENT_SECRET", "shh"),
            ])
            .into_iter(),
        );
        let resolved = resolve(&parsed[0]).expect("should resolve");
        assert_eq!(resolved.provider_key, "discord");
        assert_eq!(resolved.authorization_url, "https://discord.com/api/oauth2/authorize");
        assert_eq!(resolved.scopes, vec!["identify", "email"]);
    }
}
