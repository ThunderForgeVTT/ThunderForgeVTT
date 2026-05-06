use async_graphql::{InputObject, SimpleObject};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::admin::{AdminStatsSnapshot, AdminWelcomeSummarySnapshot, DiskUsageSummary, OAuthProviderUpdate, SystemManifestDocument, editable_manifest_keys};
use crate::models::{AdminBootstrapSetup, AuthSecuritySetting, OAuthProvider};

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
        }
    }
}

/// System manifest entry
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLManifestEntry {
    pub key: String,
    pub value: String,
    pub editable: bool,
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
            .map(|(key, value)| GraphQLManifestEntry {
                editable: editable_manifest_keys()
                    .iter()
                    .any(|candidate| *candidate == key),
                key,
                value,
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
