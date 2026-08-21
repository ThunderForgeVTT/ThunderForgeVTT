use crate::config::oauth_env::{parse_oauth_env_vars, resolve};
use crate::models::{
    AdminBootstrapSetup, AuthSecuritySetting, NewAuthSecuritySetting, NewOAuthProvider,
    OAuthProvider,
};
use crate::schema::{
    admin_bootstrap_setup, auth_security_settings, oauth_providers, policies, users, world_events,
    world_tokens, worlds,
};
use crate::state::{AppState, DbPool};
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_MANIFEST_SCHEMA_VERSION: &str = "mvp-2026.05";
const DEFAULT_REALM_NAME: &str = "ThunderForge VTT";
const DEFAULT_INTERFACE_PACK_ID: &str = "guild-hall-default";
const DEFAULT_ASSET_PACK_ID: &str = "core-preview";
const DEFAULT_SUPPORT_EMAIL: &str = "stewards@thunderforge.local";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemManifestDocument {
    pub schema_version: String,
    pub updated_at: chrono::DateTime<Utc>,
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AdminStatsSnapshot {
    pub total_users: i64,
    pub total_worlds: i64,
    pub total_world_tokens: i64,
    pub total_world_events: i64,
    pub total_policies: i64,
    pub disk_usage: DiskUsageSummary,
}

#[derive(Debug, Clone)]
pub struct AdminWelcomeSummarySnapshot {
    pub total_users: i64,
    pub total_worlds: i64,
    pub total_world_tokens: i64,
    pub total_world_events: i64,
    pub disk_usage_bytes: i64,
}

#[derive(Debug, Clone, Default)]
pub struct DiskUsageSummary {
    pub total_bytes: i64,
    pub worlds_bytes: i64,
    pub assets_bytes: i64,
    pub client_bytes: i64,
    pub databases_bytes: i64,
    pub modules_bytes: i64,
}

pub fn user_role(is_admin: bool) -> &'static str {
    if is_admin { "admin" } else { "user" }
}

pub async fn ensure_admin_defaults(state: &AppState) -> Result<(), String> {
    ensure_manifest_exists(&state.directories.manifest_file)?;
    ensure_auth_security_settings(state).await?;
    Ok(())
}

pub async fn load_admin_stats(state: &AppState) -> Result<AdminStatsSnapshot, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    let (total_users, total_worlds, total_world_tokens, total_world_events, total_policies) =
        tokio::task::spawn_blocking(move || {
            let total_users = users::table.count().get_result::<i64>(&mut conn)?;
            let total_worlds = worlds::table.count().get_result::<i64>(&mut conn)?;
            let total_world_tokens = world_tokens::table.count().get_result::<i64>(&mut conn)?;
            let total_world_events = world_events::table.count().get_result::<i64>(&mut conn)?;
            let total_policies = policies::table.count().get_result::<i64>(&mut conn)?;
            Ok::<_, diesel::result::Error>((
                total_users,
                total_worlds,
                total_world_tokens,
                total_world_events,
                total_policies,
            ))
        })
        .await
        .map_err(|_| "Failed to spawn blocking task".to_string())?
        .map_err(|_| "Failed to query admin stats".to_string())?;

    let disk_usage = recalculate_disk_usage(state)?;

    Ok(AdminStatsSnapshot {
        total_users,
        total_worlds,
        total_world_tokens,
        total_world_events,
        total_policies,
        disk_usage,
    })
}

pub async fn load_admin_welcome_summary(
    state: &AppState,
) -> Result<AdminWelcomeSummarySnapshot, String> {
    let stats = load_admin_stats(state).await?;
    Ok(AdminWelcomeSummarySnapshot {
        total_users: stats.total_users,
        total_worlds: stats.total_worlds,
        total_world_tokens: stats.total_world_tokens,
        total_world_events: stats.total_world_events,
        disk_usage_bytes: stats.disk_usage.total_bytes,
    })
}

pub async fn load_oauth_providers(state: &AppState) -> Result<Vec<OAuthProvider>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        oauth_providers::table
            .order((
                oauth_providers::display_name.asc(),
                oauth_providers::provider_key.asc(),
            ))
            .select(OAuthProvider::as_select())
            .load::<OAuthProvider>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query OAuth providers".to_string())
}

#[derive(Debug, Clone, Default)]
pub struct OAuthProviderUpdate {
    pub display_name: Option<String>,
    pub oauth_client_id: Option<String>,
    pub oauth_client_secret: Option<String>,
    pub enabled: Option<bool>,
    pub userinfo_url: Option<String>,
    pub scopes: Option<Vec<String>>,
}

pub async fn update_oauth_provider(
    state: &AppState,
    provider_id: uuid::Uuid,
    update: OAuthProviderUpdate,
) -> Result<OAuthProvider, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let now = Utc::now().naive_utc();

    tokio::task::spawn_blocking(move || {
        let existing = oauth_providers::table
            .filter(oauth_providers::id.eq(provider_id))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()?;

        let Some(existing) = existing else {
            return Ok::<_, diesel::result::Error>(None);
        };

        // ADR-041 write guard: env-sourced rows are re-asserted by the
        // startup materialization scan on every restart, so admin edits to
        // their credential/URL/label fields would silently have no lasting
        // effect. Only `enabled` (FR-006) is ever writable on such a row
        // through this mutation — every other field in `update` is ignored,
        // not erased, so the response still reflects the row's real,
        // persisted (env-sourced) values.
        let is_env_sourced = existing.config_source == "env";
        let display_name = if is_env_sourced {
            existing.display_name
        } else {
            update.display_name.unwrap_or(existing.display_name)
        };
        let oauth_client_id = if is_env_sourced {
            existing.oauth_client_id
        } else {
            update.oauth_client_id.or(existing.oauth_client_id)
        };
        let oauth_client_secret = if is_env_sourced {
            existing.oauth_client_secret
        } else {
            update.oauth_client_secret.or(existing.oauth_client_secret)
        };
        let enabled = update.enabled.unwrap_or(existing.enabled);
        let userinfo_url = if is_env_sourced {
            existing.userinfo_url
        } else {
            update.userinfo_url.or(existing.userinfo_url)
        };
        let scopes = if is_env_sourced {
            existing.scopes
        } else {
            update
                .scopes
                .map(|items| items.into_iter().map(Some).collect::<Vec<_>>())
                .unwrap_or(existing.scopes)
        };
        let configured = oauth_client_id.is_some() && oauth_client_secret.is_some();

        diesel::update(oauth_providers::table.filter(oauth_providers::id.eq(provider_id)))
            .set((
                oauth_providers::display_name.eq(display_name),
                oauth_providers::oauth_client_id.eq(oauth_client_id),
                oauth_providers::oauth_client_secret.eq(oauth_client_secret),
                oauth_providers::enabled.eq(enabled),
                oauth_providers::userinfo_url.eq(userinfo_url),
                oauth_providers::scopes.eq(scopes),
                oauth_providers::configured.eq(configured),
                oauth_providers::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        oauth_providers::table
            .filter(oauth_providers::id.eq(provider_id))
            .select(OAuthProvider::as_select())
            .first::<OAuthProvider>(&mut conn)
            .optional()
            .map(Some)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to update OAuth provider".to_string())?
    .flatten()
    .ok_or_else(|| "OAuth provider not found".to_string())
}

/// Startup-time materialization of `OAUTH_*` env-var-configured provider
/// instances into `oauth_providers` rows (ADR-041, research.md §3/§6).
///
/// - Every env-var-detected instance is upserted with `config_source =
///   "env"`; every writable field is refreshed on each run except `enabled`,
///   which is only set `true` on first insert — an admin's later toggle
///   must survive a restart.
/// - Any row that *was* `config_source = "env"` but whose env vars are no
///   longer present in this scan is flipped back to `config_source =
///   "admin"`, values untouched (never deleted — see research.md §6 on why
///   deleting would cascade-orphan linked `user_oauth_accounts`).
/// - Incomplete env-var groups are logged (FR-010) and skipped, never
///   panicking startup.
pub async fn materialize_env_oauth_providers(db_pool: &DbPool) -> Result<(), String> {
    let parsed = parse_oauth_env_vars(std::env::vars());
    let mut resolved = Vec::with_capacity(parsed.len());
    for instance in &parsed {
        match resolve(instance) {
            Ok(r) => resolved.push(r),
            Err(missing) => {
                tracing::warn!(
                    provider = %missing.provider,
                    instance = %missing.instance,
                    missing_field = %missing.field,
                    "OAuth env-var provider instance is missing a required setting; skipping"
                );
            }
        }
    }

    let mut conn = db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let now = Utc::now().naive_utc();
        let mut seen_keys: HashSet<String> = HashSet::new();

        for r in &resolved {
            seen_keys.insert(r.provider_key.clone());
            let scopes: Vec<Option<String>> = r.scopes.iter().cloned().map(Some).collect();

            let existing = oauth_providers::table
                .filter(oauth_providers::provider_key.eq(&r.provider_key))
                .select(OAuthProvider::as_select())
                .first::<OAuthProvider>(&mut conn)
                .optional()?;

            match existing {
                Some(row) => {
                    // Only set enabled=true on the row's first transition
                    // into config_source="env"; an already-env-sourced row
                    // keeps whatever an admin last toggled it to.
                    let enabled = if row.config_source == "env" {
                        row.enabled
                    } else {
                        true
                    };
                    diesel::update(oauth_providers::table.filter(oauth_providers::id.eq(row.id)))
                        .set((
                            oauth_providers::display_name.eq(&r.display_name),
                            oauth_providers::authorization_url.eq(&r.authorization_url),
                            oauth_providers::token_url.eq(&r.token_url),
                            oauth_providers::userinfo_url.eq(&r.userinfo_url),
                            oauth_providers::scopes.eq(&scopes),
                            oauth_providers::oauth_client_id.eq(Some(&r.client_id)),
                            oauth_providers::oauth_client_secret.eq(Some(&r.client_secret)),
                            oauth_providers::configured.eq(true),
                            oauth_providers::config_source.eq("env"),
                            oauth_providers::enabled.eq(enabled),
                            oauth_providers::updated_at.eq(now),
                        ))
                        .execute(&mut conn)?;
                }
                None => {
                    let new_row = NewOAuthProvider {
                        id: uuid::Uuid::now_v7(),
                        provider_key: r.provider_key.clone(),
                        display_name: r.display_name.clone(),
                        authorization_url: r.authorization_url.clone(),
                        token_url: r.token_url.clone(),
                        userinfo_url: r.userinfo_url.clone(),
                        scopes,
                        oauth_client_id: Some(r.client_id.clone()),
                        oauth_client_secret: Some(r.client_secret.clone()),
                        configured: true,
                        enabled: true,
                        created_at: now,
                        updated_at: now,
                        config_source: "env".to_string(),
                    };
                    diesel::insert_into(oauth_providers::table)
                        .values(&new_row)
                        .execute(&mut conn)?;
                }
            }
        }

        // Anything still config_source="env" but not seen in this scan had
        // its env vars removed — revert to admin-editable, values retained.
        let stale_env_rows = oauth_providers::table
            .filter(oauth_providers::config_source.eq("env"))
            .select(OAuthProvider::as_select())
            .load::<OAuthProvider>(&mut conn)?;
        for row in stale_env_rows {
            if !seen_keys.contains(&row.provider_key) {
                diesel::update(oauth_providers::table.filter(oauth_providers::id.eq(row.id)))
                    .set((
                        oauth_providers::config_source.eq("admin"),
                        oauth_providers::updated_at.eq(now),
                    ))
                    .execute(&mut conn)?;
            }
        }

        Ok::<_, diesel::result::Error>(())
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to materialize env-configured OAuth providers".to_string())
}

pub async fn load_auth_security_settings(state: &AppState) -> Result<AuthSecuritySetting, String> {
    ensure_auth_security_settings(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        auth_security_settings::table
            .filter(auth_security_settings::id.eq(1))
            .select(AuthSecuritySetting::as_select())
            .first::<AuthSecuritySetting>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query auth security settings".to_string())
}

pub async fn update_two_factor_policy(
    state: &AppState,
    required_for_all_users: bool,
) -> Result<AuthSecuritySetting, String> {
    ensure_auth_security_settings(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let now = Utc::now().naive_utc();

    tokio::task::spawn_blocking(move || {
        diesel::update(auth_security_settings::table.filter(auth_security_settings::id.eq(1)))
            .set((
                auth_security_settings::two_factor_required_for_all_users
                    .eq(required_for_all_users),
                auth_security_settings::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        auth_security_settings::table
            .filter(auth_security_settings::id.eq(1))
            .select(AuthSecuritySetting::as_select())
            .first::<AuthSecuritySetting>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to update auth security settings".to_string())
}

pub async fn load_admin_bootstrap_settings(
    state: &AppState,
) -> Result<Option<AdminBootstrapSetup>, String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        admin_bootstrap_setup::table
            .filter(admin_bootstrap_setup::id.eq(1))
            .select(AdminBootstrapSetup::as_select())
            .first::<AdminBootstrapSetup>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query bootstrap settings".to_string())
}

pub fn read_system_manifest(state: &AppState) -> Result<SystemManifestDocument, String> {
    ensure_manifest_exists(&state.directories.manifest_file)?;
    let path = Path::new(&state.directories.manifest_file);
    let contents =
        fs::read_to_string(path).map_err(|_| "Failed to read manifest file".to_string())?;
    serde_json::from_str::<SystemManifestDocument>(&contents)
        .map_err(|_| "Failed to parse manifest file".to_string())
}

pub fn update_manifest_key(
    state: &AppState,
    key: &str,
    value: &str,
) -> Result<SystemManifestDocument, String> {
    if !is_editable_manifest_key(key) {
        return Err("Manifest key is not editable".to_string());
    }

    let mut manifest = read_system_manifest(state)?;
    manifest
        .metadata
        .insert(key.to_string(), value.trim().to_string());
    manifest.updated_at = Utc::now();
    write_manifest(&state.directories.manifest_file, &manifest)?;
    Ok(manifest)
}

pub fn editable_manifest_keys() -> &'static [&'static str] {
    &[
        "realm_name",
        "interface_pack_id",
        "asset_pack_id",
        "support_email",
        "welcome_message",
    ]
}

pub fn recalculate_disk_usage(state: &AppState) -> Result<DiskUsageSummary, String> {
    let base = Path::new(&state.config.data_path);
    let worlds = dir_size(Path::new(&state.directories.world_basedir))?;
    let assets = dir_size(Path::new(&state.directories.asset_directory))?;
    let client = dir_size(Path::new(&state.directories.static_files))?;
    let databases = dir_size(Path::new(&state.directories.databases_basedir))?;
    let modules = dir_size(Path::new(&state.directories.modules_basedir))?;
    let total = dir_size(base)?;

    Ok(DiskUsageSummary {
        total_bytes: to_i64(total),
        worlds_bytes: to_i64(worlds),
        assets_bytes: to_i64(assets),
        client_bytes: to_i64(client),
        databases_bytes: to_i64(databases),
        modules_bytes: to_i64(modules),
    })
}

async fn ensure_auth_security_settings(state: &AppState) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let existing = auth_security_settings::table
            .filter(auth_security_settings::id.eq(1))
            .select(AuthSecuritySetting::as_select())
            .first::<AuthSecuritySetting>(&mut conn)
            .optional()?;

        if existing.is_none() {
            let now = Utc::now().naive_utc();
            diesel::insert_into(auth_security_settings::table)
                .values(NewAuthSecuritySetting {
                    id: 1,
                    two_factor_required_for_all_users: false,
                    updated_at: now,
                })
                .execute(&mut conn)?;
        }

        Ok::<_, diesel::result::Error>(())
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to ensure auth security settings".to_string())
}

fn ensure_manifest_exists(manifest_path: &str) -> Result<(), String> {
    let path = Path::new(manifest_path);
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|_| "Failed to create manifest directory".to_string())?;
    }

    let manifest = default_manifest();
    write_manifest(manifest_path, &manifest)
}

fn write_manifest(path: &str, manifest: &SystemManifestDocument) -> Result<(), String> {
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|_| "Failed to serialize manifest".to_string())?;
    fs::write(path, json).map_err(|_| "Failed to write manifest file".to_string())
}

fn default_manifest() -> SystemManifestDocument {
    let mut metadata = BTreeMap::new();
    metadata.insert("realm_name".to_string(), DEFAULT_REALM_NAME.to_string());
    metadata.insert(
        "interface_pack_id".to_string(),
        DEFAULT_INTERFACE_PACK_ID.to_string(),
    );
    metadata.insert(
        "asset_pack_id".to_string(),
        DEFAULT_ASSET_PACK_ID.to_string(),
    );
    metadata.insert(
        "support_email".to_string(),
        DEFAULT_SUPPORT_EMAIL.to_string(),
    );
    metadata.insert(
        "welcome_message".to_string(),
        "Welcome to the ThunderForge guild hall.".to_string(),
    );

    SystemManifestDocument {
        schema_version: DEFAULT_MANIFEST_SCHEMA_VERSION.to_string(),
        updated_at: Utc::now(),
        metadata,
    }
}

fn is_editable_manifest_key(key: &str) -> bool {
    editable_manifest_keys().contains(&key)
}

fn dir_size(path: &Path) -> Result<u64, String> {
    if !path.exists() {
        return Ok(0);
    }

    let mut total = 0_u64;
    let mut stack = vec![PathBuf::from(path)];

    while let Some(current) = stack.pop() {
        let entries = fs::read_dir(&current)
            .map_err(|_| format!("Failed to read directory {}", current.display()))?;

        for entry in entries {
            let entry = entry.map_err(|_| "Failed to read directory entry".to_string())?;
            let metadata = entry
                .metadata()
                .map_err(|_| format!("Failed to read metadata for {}", entry.path().display()))?;

            if metadata.is_dir() {
                stack.push(entry.path());
            } else {
                total = total.saturating_add(metadata.len());
            }
        }
    }

    Ok(total)
}

fn to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::{editable_manifest_keys, is_editable_manifest_key, user_role};

    #[test]
    fn editable_manifest_keys_are_whitelisted() {
        assert!(is_editable_manifest_key("interface_pack_id"));
        assert!(!is_editable_manifest_key("schema_version"));
        assert_eq!(editable_manifest_keys().len(), 5);
    }

    #[test]
    fn user_role_reflects_admin_flag() {
        assert_eq!(user_role(true), "admin");
        assert_eq!(user_role(false), "user");
    }
}
