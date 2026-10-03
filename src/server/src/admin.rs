/// Spec 041 FR-021/FR-023/FR-024: coverage counts and the one-account lookup an
/// operator acts through. Its own file since `admin.rs` reached the length
/// gate; the cut is by subject, not by convenience.
#[path = "admin/two_factor_operations.rs"]
pub mod two_factor_operations;

/// The sign-in providers an operator may configure. Its own file for the same
/// reason as `two_factor_operations`, and along a seam that was already there:
/// one table, and the two rules that govern it.
#[path = "admin/providers.rs"]
pub mod providers;
pub use two_factor_operations::{
    AdminAccountView, TwoFactorCoverage, find_account_for_admin, load_two_factor_coverage,
};

use crate::auth::instance_access::InstanceAccessPolicy;
use crate::models::{
    AdminBootstrapSetup, AuthSecuritySetting, InstanceAccessSetting, NewAuthSecuritySetting,
};
use crate::schema::{
    admin_bootstrap_setup, auth_security_settings, instance_access_settings, policies, tokens,
    users, world_events, worlds,
};
use crate::state::AppState;
use chrono::Utc;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// What a fresh realm is seeded with, as shipped.
///
/// Six `const`s stood here — the realm's name, its support address, its
/// welcome message, and `default_game_system_id`, which was the last system
/// identifier written into shared server code and the standing exception in
/// `scripts/check-system-registry.mjs`'s `KNOWN` list (T014a3).
///
/// Seeding a realm is configuration, not logic. It belongs in a file an
/// operator can read and a diff can show, and moving it there is what takes
/// the last system name out of `src/server`.
///
/// `include_str!` rather than a runtime read, and the two reasons are
/// different. `src/server/data` is gitignored, so a config file placed there
/// ships with no install and every new world would come out systemless — the
/// silent regression the old comment warned about. And a seed read from disk
/// is a seed that can be absent at exactly the moment it is needed, which is
/// the first boot, on someone else's machine. Compiled in, the file is
/// editable, reviewable and versioned, and cannot go missing.
const REALM_DEFAULTS_JSON: &str = include_str!("../../../config/realm-defaults.json");

#[derive(Debug, Deserialize)]
struct RealmDefaults {
    schema_version: String,
    metadata: BTreeMap<String, String>,
}

fn realm_defaults() -> RealmDefaults {
    // A parse failure here is a malformed file that shipped, which a test in
    // this module fails on. Panicking is right: a realm seeded from a
    // half-read config is worse than one that refuses to start, and the
    // condition cannot arise from anything an operator does at runtime.
    serde_json::from_str(REALM_DEFAULTS_JSON).expect("config/realm-defaults.json is malformed")
}

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
            // The tokens on every scene: the scene-scoped `tokens` table, the
            // only one since the world-scoped `world_tokens` was dropped.
            let total_world_tokens = tokens::table.count().get_result::<i64>(&mut conn)?;
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

pub use providers::{
    OAuthProviderUpdate, load_oauth_providers, materialize_env_oauth_providers,
    update_oauth_provider,
};

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

/// The system a new world starts with, as the operator configured it.
///
/// An unreadable manifest, a missing key, or an empty value all mean the same
/// thing — no default — and a world created that way simply has no system.
/// That is a state the product handles; guessing one instead would bind a
/// world to a ruleset nobody chose.
pub fn default_game_system_id(state: &AppState) -> Option<String> {
    read_system_manifest(state)
        .ok()?
        .metadata
        .get("default_game_system_id")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn read_system_manifest(state: &AppState) -> Result<SystemManifestDocument, String> {
    ensure_manifest_exists(&state.directories.manifest_file)?;
    let path = Path::new(&state.directories.manifest_file);
    let contents =
        fs::read_to_string(path).map_err(|_| "Failed to read manifest file".to_string())?;
    let mut manifest = serde_json::from_str::<SystemManifestDocument>(&contents)
        .map_err(|_| "Failed to parse manifest file".to_string())?;

    backfill_missing_seeds(&mut manifest);
    Ok(manifest)
}

/// Give a stored manifest the seeds it was written before we shipped.
///
/// The manifest file is created once, on first boot, and `ensure_manifest_exists`
/// returns early forever after. So a seed key added to the product later is
/// absent on every install that already existed — permanently, and silently.
///
/// That is not hypothetical. `default_game_system_id` arrived after this
/// machine's manifest was written, so `default_game_system_id()` returned
/// `None` and a world created without naming a system came out with **no
/// system at all** — no error, no default, and a character sheet with nothing
/// on it. Traced from exactly that symptom.
///
/// **Absent keys only.** A value an operator has set is theirs, including one
/// they deliberately set to something other than the shipped default, and this
/// must never reach in and correct it. Only a key that is not there at all
/// gets filled, which is the difference between a migration and an override.
fn backfill_missing_seeds(manifest: &mut SystemManifestDocument) {
    for (key, value) in realm_defaults().metadata {
        manifest.metadata.entry(key).or_insert(value);
    }
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

/// What the manifest keys hold as shipped, before anybody edits them.
///
/// Spec 040 needs this to answer a question the manifest file alone cannot:
/// whether a value is one an operator chose or one that was seeded. Reporting
/// a freshly seeded `realm_name` as the *instance's* value would imply
/// somebody set it, and readiness would then have no way to notice that a
/// support address is still `stewards@thunderforge.local` (research.md § D6).
///
/// Read from the same compiled-in file `default_manifest` and
/// `backfill_missing_seeds` use, so there is one shipped answer rather than
/// two that can drift.
pub fn shipped_manifest_defaults() -> BTreeMap<String, String> {
    realm_defaults().metadata
}

pub fn editable_manifest_keys() -> &'static [&'static str] {
    &[
        "realm_name",
        "interface_pack_id",
        "asset_pack_id",
        "support_email",
        "welcome_message",
        // Which system a new world starts with. An operator's decision, and
        // deliberately not a constant in shared server code — see
        // `prepare_world_input` and spec 032 FR-029.
        "default_game_system_id",
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
    let defaults = realm_defaults();

    SystemManifestDocument {
        schema_version: defaults.schema_version,
        updated_at: Utc::now(),
        metadata: defaults.metadata,
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

/// Spec 035 / ADR-072: read the instance's admission policy.
///
/// The settings row is seeded by the migration, conditionally on whether the
/// instance already had users (FR-013, FR-013a). `ensure_` exists only so a
/// missing row cannot take the instance down; it must never be the thing that
/// decides a fresh instance's default, because after the fact nothing can tell
/// a fresh instance from an upgraded one.
pub async fn load_instance_access_settings(
    state: &AppState,
) -> Result<InstanceAccessSetting, String> {
    ensure_instance_access_settings(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        instance_access_settings::table
            .filter(instance_access_settings::id.eq(1))
            .select(InstanceAccessSetting::as_select())
            .first::<InstanceAccessSetting>(&mut conn)
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to query the instance access settings".to_string())
}

async fn ensure_instance_access_settings(state: &AppState) -> Result<(), String> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;

    tokio::task::spawn_blocking(move || {
        let existing = instance_access_settings::table
            .filter(instance_access_settings::id.eq(1))
            .select(InstanceAccessSetting::as_select())
            .first::<InstanceAccessSetting>(&mut conn)
            .optional()?;

        if existing.is_none() {
            // Fail shut. A row that should have been seeded by the migration
            // and is not there is an anomaly, and the safe way to be wrong
            // about an anomaly is to admit nobody.
            diesel::insert_into(instance_access_settings::table)
                .values((
                    instance_access_settings::id.eq(1),
                    instance_access_settings::access_policy.eq("closed"),
                    instance_access_settings::updated_at.eq(Utc::now().naive_utc()),
                ))
                .execute(&mut conn)?;
        }

        Ok::<_, diesel::result::Error>(())
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to ensure the instance access settings".to_string())
}

/// Writes the policy and its audit event **in one transaction** (FR-002,
/// FR-004). An audit row that can be lost independently of the change it
/// records is not an audit row.
///
/// `actor_user_id` is optional because the first time this is called there is
/// nobody signed in: "who may join" is the first-run wizard's opening
/// question (spec 064), and the wizard is authenticated by the bootstrap admin
/// code rather than by a session. Both columns are nullable, so an
/// unattributed change is recorded as one rather than attributed to a guess.
pub async fn update_instance_access_policy(
    state: &AppState,
    actor_user_id: Option<uuid::Uuid>,
    new_policy: InstanceAccessPolicy,
) -> Result<InstanceAccessSetting, String> {
    ensure_instance_access_settings(state).await?;
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| "Failed to get DB connection".to_string())?;
    let now = Utc::now().naive_utc();

    tokio::task::spawn_blocking(move || {
        conn.transaction::<_, diesel::result::Error, _>(|conn| {
            let previous = instance_access_settings::table
                .filter(instance_access_settings::id.eq(1))
                .select(instance_access_settings::access_policy)
                .first::<String>(conn)?;

            let updated = diesel::update(
                instance_access_settings::table.filter(instance_access_settings::id.eq(1)),
            )
            .set((
                instance_access_settings::access_policy.eq(new_policy.as_db_str()),
                instance_access_settings::updated_by.eq(actor_user_id),
                instance_access_settings::updated_at.eq(now),
            ))
            .returning(InstanceAccessSetting::as_returning())
            .get_result::<InstanceAccessSetting>(conn)?;

            diesel::insert_into(crate::schema::instance_access_events::table)
                .values(crate::models::NewInstanceAccessEvent {
                    id: uuid::Uuid::now_v7(),
                    event_type: "policy_changed".to_string(),
                    actor_user_id,
                    previous_policy: Some(previous),
                    new_policy: Some(new_policy.as_db_str().to_string()),
                    attempted_route: None,
                    policy_at_attempt: None,
                })
                .execute(conn)?;

            Ok(updated)
        })
    })
    .await
    .map_err(|_| "Failed to spawn blocking task".to_string())?
    .map_err(|_| "Failed to update the instance access policy".to_string())
}

#[cfg(test)]
mod tests {
    use super::{default_manifest, editable_manifest_keys, is_editable_manifest_key, user_role};

    /// The shipped seed file parses, and seeds what it says it seeds.
    ///
    /// It is compiled in with `include_str!`, so a malformed or renamed key is
    /// a runtime panic on first boot rather than a compile error — the one
    /// failure mode this arrangement has, and the reason this test exists. It
    /// caught exactly that: the file shipped `schemaVersion` while the struct
    /// expected `schema_version`, and everything still built.
    #[test]
    fn the_shipped_realm_defaults_parse_and_seed_a_manifest() {
        let manifest = default_manifest();

        assert!(
            !manifest.schema_version.is_empty(),
            "a realm seeded with no schema version"
        );
        // Every key the settings page offers to edit must actually be seeded,
        // or an operator opens the page to a blank field for a setting the
        // product claims to have.
        for key in editable_manifest_keys() {
            assert!(
                manifest.metadata.contains_key(*key),
                "editable setting `{key}` is not seeded by config/realm-defaults.json"
            );
        }
    }

    /// A manifest written before a seed key existed gains it (traced bug).
    ///
    /// `ensure_manifest_exists` returns early once the file is there, so a key
    /// added to the product later was absent on every install that already
    /// existed — permanently and silently. This machine's manifest predated
    /// `default_game_system_id`, so `default_game_system_id()` answered `None`
    /// and a world created without naming a system came out with **no system
    /// at all**: no error, no default, and a sheet with nothing on it.
    #[test]
    fn a_manifest_written_before_a_seed_existed_gains_it_on_read() {
        let mut stored = default_manifest();
        stored.metadata.remove("default_game_system_id");
        assert!(!stored.metadata.contains_key("default_game_system_id"));

        super::backfill_missing_seeds(&mut stored);

        assert_eq!(
            stored.metadata.get("default_game_system_id"),
            default_manifest().metadata.get("default_game_system_id"),
            "a key the stored manifest never had must arrive from the shipped seeds"
        );
    }

    /// And a value an operator set is theirs, including one that differs from
    /// the shipped default on purpose. Backfilling absent keys is a migration;
    /// correcting present ones would be an override, and would silently undo
    /// somebody's decision on every read.
    #[test]
    fn an_operators_own_value_is_never_corrected_to_the_shipped_one() {
        let mut stored = default_manifest();
        stored
            .metadata
            .insert("realm_name".to_string(), "The Iron Table".to_string());
        stored.metadata.insert(
            "default_game_system_id".to_string(),
            "fate_core".to_string(),
        );

        super::backfill_missing_seeds(&mut stored);

        assert_eq!(stored.metadata.get("realm_name").unwrap(), "The Iron Table");
        assert_eq!(
            stored.metadata.get("default_game_system_id").unwrap(),
            "fate_core"
        );
    }

    /// Blanking the seeded system would make every new world systemless on
    /// every install, which is a silent regression rather than a tidy-up: no
    /// world would name a system and nothing would say why.
    #[test]
    fn a_fresh_realm_is_seeded_with_a_game_system() {
        let manifest = default_manifest();
        let seeded = manifest
            .metadata
            .get("default_game_system_id")
            .map(String::as_str)
            .unwrap_or("");

        assert!(!seeded.is_empty(), "a realm seeded with no game system");
        // Deliberately not asserting *which*. That is an operator's choice and
        // a shipped default, and pinning it here would put the system's name
        // back into src/server — which is the thing T014a3 just removed.
    }

    #[test]
    fn editable_manifest_keys_are_whitelisted() {
        assert!(is_editable_manifest_key("interface_pack_id"));
        // Which system a new world starts with is an operator's decision, and
        // this is where operators make it — spec 032 FR-029 is what moved it
        // out of `prepare_world_input`.
        assert!(is_editable_manifest_key("default_game_system_id"));
        assert!(!is_editable_manifest_key("schema_version"));
        assert_eq!(editable_manifest_keys().len(), 6);
    }

    #[test]
    fn user_role_reflects_admin_flag() {
        assert_eq!(user_role(true), "admin");
        assert_eq!(user_role(false), "user");
    }
}
