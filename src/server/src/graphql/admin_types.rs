use async_graphql::{Enum, InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::admin::{
    AdminStatsSnapshot, AdminWelcomeSummarySnapshot, DiskUsageSummary, OAuthProviderUpdate,
    SystemManifestDocument, editable_manifest_keys,
};
use crate::models::{AdminBootstrapSetup, AuthSecuritySetting, OAuthProvider};
use crate::settings::graphql::GraphQLSettingSource;

/// Disk usage breakdown for admin statistics
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDiskUsageBreakdown {
    pub total_bytes: i64,
    pub worlds_bytes: i64,
    pub assets_bytes: i64,
    pub client_bytes: i64,
    pub databases_bytes: i64,
    pub modules_bytes: i64,
}

impl From<DiskUsageSummary> for GraphQLDiskUsageBreakdown {
    fn from(value: DiskUsageSummary) -> Self {
        Self {
            total_bytes: value.total_bytes,
            worlds_bytes: value.worlds_bytes,
            assets_bytes: value.assets_bytes,
            client_bytes: value.client_bytes,
            databases_bytes: value.databases_bytes,
            modules_bytes: value.modules_bytes,
        }
    }
}

/// Comprehensive admin statistics snapshot
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAdminStats {
    pub total_users: i64,
    pub total_worlds: i64,
    pub total_world_tokens: i64,
    pub total_world_events: i64,
    pub total_policies: i64,
    pub disk_usage_bytes: i64,
    pub disk_usage: GraphQLDiskUsageBreakdown,
}

impl From<AdminStatsSnapshot> for GraphQLAdminStats {
    fn from(value: AdminStatsSnapshot) -> Self {
        let disk_usage_bytes = value.disk_usage.total_bytes;
        Self {
            total_users: value.total_users,
            total_worlds: value.total_worlds,
            total_world_tokens: value.total_world_tokens,
            total_world_events: value.total_world_events,
            total_policies: value.total_policies,
            disk_usage_bytes,
            disk_usage: value.disk_usage.into(),
        }
    }
}

/// Admin welcome dashboard summary
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAdminWelcomeSummary {
    pub total_users: i64,
    pub total_worlds: i64,
    pub total_tokens: i64,
    pub total_events: i64,
    pub disk_usage: i64,
}

impl From<AdminWelcomeSummarySnapshot> for GraphQLAdminWelcomeSummary {
    fn from(value: AdminWelcomeSummarySnapshot) -> Self {
        Self {
            total_users: value.total_users,
            total_worlds: value.total_worlds,
            total_tokens: value.total_world_tokens,
            total_events: value.total_world_events,
            disk_usage: value.disk_usage_bytes,
        }
    }
}

/// Where an OAuth provider instance's configuration comes from (ADR-041).
/// `Env` rows are materialized and kept in sync by the server's startup
/// env-var scan; only `enabled` is writable on them through
/// `updateOauthProvider` (see that mutation's source-aware write guard).
#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLOAuthConfigSource {
    Admin,
    Env,
}

impl From<&str> for GraphQLOAuthConfigSource {
    fn from(value: &str) -> Self {
        match value {
            "env" => GraphQLOAuthConfigSource::Env,
            _ => GraphQLOAuthConfigSource::Admin,
        }
    }
}

/// OAuth provider configuration
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLOAuthProvider {
    pub id: Uuid,
    pub provider_key: String,
    pub display_name: String,
    pub authorization_url: String,
    pub token_url: String,
    pub userinfo_url: Option<String>,
    pub scopes: Vec<String>,
    pub oauth_client_id: Option<String>,
    pub configured: bool,
    pub enabled: bool,
    pub has_client_secret: bool,
    pub updated_at: chrono::NaiveDateTime,
    pub config_source: GraphQLOAuthConfigSource,
    /// The same vocabulary every other setting reports (spec 040 FR-011,
    /// T027). `config_source` predates it and is kept because clients read it;
    /// this is the field a client should use when it wants to say "where did
    /// this value come from" in the words the rest of the product uses.
    ///
    /// There is deliberately **no `fixedBy`** here. A provider is fixed by a
    /// *set* of variables — `OAUTH_<segment>_<key>_CLIENT_ID`, `_CLIENT_SECRET`
    /// and possibly an issuer field — so naming one of them would be picking a
    /// representative and calling it the answer. A settings key has exactly one
    /// variable and can honestly name it; this cannot.
    pub source: GraphQLSettingSource,
}

impl From<OAuthProvider> for GraphQLOAuthProvider {
    fn from(value: OAuthProvider) -> Self {
        Self {
            id: value.id,
            provider_key: value.provider_key,
            display_name: value.display_name,
            authorization_url: value.authorization_url,
            token_url: value.token_url,
            userinfo_url: value.userinfo_url,
            scopes: value.scopes.into_iter().flatten().collect(),
            oauth_client_id: value.oauth_client_id,
            configured: value.configured,
            enabled: value.enabled,
            has_client_secret: value.oauth_client_secret.is_some(),
            updated_at: value.updated_at,
            source: match value.config_source.as_str() {
                "env" => GraphQLSettingSource::Environment,
                _ => GraphQLSettingSource::Instance,
            },
            config_source: value.config_source.as_str().into(),
        }
    }
}

/// System manifest entry
///
/// Spec 040 FR-009: a key the environment has fixed is not offered for
/// editing **and says which variable fixed it**. A field greyed out with no
/// stated reason is the thing that feature exists to stop, so `editable` and
/// `fixed_by` are answered together and by the same resolver the settings
/// surface uses — not by a rule restated here.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLManifestEntry {
    pub key: String,
    pub value: String,
    pub editable: bool,
    /// The environment variable that fixed this value, when one has.
    pub fixed_by: Option<String>,
    /// Where the value came from, in the vocabulary every other surface uses.
    pub source: GraphQLSettingSource,
}

/// System manifest configuration document
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLSystemManifest {
    pub path: String,
    pub schema_version: String,
    pub updated_at: DateTime<Utc>,
    pub entries: Vec<GraphQLManifestEntry>,
}

impl GraphQLSystemManifest {
    pub fn from_document(path: String, manifest: SystemManifestDocument) -> Self {
        let mut entries = manifest
            .metadata
            .into_iter()
            .map(|(key, value)| {
                let declaration = crate::settings::registry::declaration(&key);
                let fixed_by = declaration
                    .and_then(|d| d.env_name_in_use())
                    .map(str::to_string);
                // Three-way and computed from the declaration rather than
                // guessed: a key equal to its shipped default is reporting a
                // default, not an operator's choice, and conflating the two is
                // how a settings screen tells somebody they configured
                // something they never touched.
                let source = if fixed_by.is_some() {
                    GraphQLSettingSource::Environment
                } else if declaration.and_then(|d| d.default) == Some(value.as_str()) {
                    GraphQLSettingSource::Default
                } else {
                    GraphQLSettingSource::Instance
                };
                GraphQLManifestEntry {
                    source,
                    editable: fixed_by.is_none()
                        && editable_manifest_keys()
                            .iter()
                            .any(|candidate| *candidate == key),
                    fixed_by,
                    key,
                    value,
                }
            })
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.key.cmp(&right.key));

        Self {
            path,
            schema_version: manifest.schema_version,
            updated_at: manifest.updated_at,
            entries,
        }
    }
}

/// Authentication security settings
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAuthSecuritySettings {
    pub two_factor_required_for_all_users: bool,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<AuthSecuritySetting> for GraphQLAuthSecuritySettings {
    fn from(value: AuthSecuritySetting) -> Self {
        Self {
            two_factor_required_for_all_users: value.two_factor_required_for_all_users,
            updated_at: value.updated_at,
        }
    }
}

/// Spec 041 FR-021: how much of the instance holds a second factor.
///
/// # Counts, never a roster
///
/// SC-007 asks an operator to tell "at a glance, how much of their instance
/// has a second factor". A list of who has not enrolled would answer that and
/// also hand whoever reads it — or whoever takes over the account that reads
/// it — a target list: the accounts on it are exactly the ones a stolen
/// password is sufficient for. Three integers answer the question the operator
/// actually has and cannot be turned into that list.
///
/// It sits beside `authSecuritySettings` because the switch it explains lives
/// there: the point of `requiredNotEnrolled` is to say, before the switch is
/// thrown, how many people it is about to ask something of.
///
/// Named `TwoFactorCoverage` on the wire, as `contracts/requirement-policy.md`
/// specifies, rather than carrying the crate's `GraphQL` prefix into the
/// schema a client reads.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TwoFactorCoverage")]
pub struct GraphQLTwoFactorCoverage {
    /// Accounts holding a confirmed second factor.
    pub enrolled: i32,
    /// Accounts without one.
    pub not_enrolled: i32,
    /// Of those, how many are required to have one and have not — which
    /// includes every administrator, because the role requires it (FR-027).
    pub required_not_enrolled: i32,
}

impl From<crate::admin::TwoFactorCoverage> for GraphQLTwoFactorCoverage {
    fn from(value: crate::admin::TwoFactorCoverage) -> Self {
        // Saturating rather than `as`: a count that overflowed an i32 would
        // otherwise arrive as a negative, and a negative account count is
        // worse than a clamped one.
        Self {
            enrolled: i32::try_from(value.enrolled).unwrap_or(i32::MAX),
            not_enrolled: i32::try_from(value.not_enrolled).unwrap_or(i32::MAX),
            required_not_enrolled: i32::try_from(value.required_not_enrolled).unwrap_or(i32::MAX),
        }
    }
}

/// Admin bootstrap/setup configuration
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAdminBootstrapSettings {
    pub setup_completed: bool,
    pub admin_code_generated_at: Option<chrono::NaiveDateTime>,
    pub setup_completed_at: Option<chrono::NaiveDateTime>,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<AdminBootstrapSetup> for GraphQLAdminBootstrapSettings {
    fn from(value: AdminBootstrapSetup) -> Self {
        Self {
            setup_completed: value.setup_completed_at.is_some(),
            admin_code_generated_at: value.admin_code_generated_at,
            setup_completed_at: value.setup_completed_at,
            updated_at: value.updated_at,
        }
    }
}

/// Input object for OAuth provider configuration
#[derive(InputObject, Debug, Clone, Default)]
pub struct GraphQLOAuthProviderConfigInput {
    pub display_name: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub enabled: Option<bool>,
    pub userinfo_url: Option<String>,
    pub scopes: Option<Vec<String>>,
}

impl From<GraphQLOAuthProviderConfigInput> for OAuthProviderUpdate {
    fn from(value: GraphQLOAuthProviderConfigInput) -> Self {
        Self {
            display_name: value.display_name,
            oauth_client_id: value.oauth_client_id,
            oauth_client_secret: value.oauth_client_secret,
            enabled: value.enabled,
            userinfo_url: value.userinfo_url,
            scopes: value.scopes,
        }
    }
}
