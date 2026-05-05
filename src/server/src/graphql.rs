use async_graphql::{
    Context, Enum, Error, InputObject, Json, MergedObject, Result as GraphQLResult, Schema,
    SimpleObject, Subscription,
};
use base64::Engine;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use futures_util::Stream;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::IntervalStream;

use crate::admin::{
    AdminStatsSnapshot, AdminWelcomeSummarySnapshot, DiskUsageSummary, OAuthProviderUpdate,
    SystemManifestDocument, editable_manifest_keys, load_admin_bootstrap_settings,
    load_admin_stats, load_admin_welcome_summary, load_auth_security_settings,
    load_oauth_providers, read_system_manifest, recalculate_disk_usage as calculate_disk_usage,
    update_manifest_key as persist_manifest_key, update_oauth_provider as persist_oauth_provider,
    update_two_factor_policy as persist_two_factor_policy,
};
use crate::auth_middleware::AuthenticatedUser;
use crate::db_types::PolicyEffectEnum;
use crate::models::{
    AdminBootstrapSetup, AuthSecuritySetting, GameSystem, OAuthProvider, Policy, User, World,
    WorldEvent, WorldToken,
};
use crate::schema::{game_systems, policies, users, world_events, world_tokens, worlds};
use crate::state::AppState;
use crate::users::{
    UserDataDeleteSummary, UserDataExport, delete_user_data_owned, export_user_data_payload,
};

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLUser {
    id: uuid::Uuid,
    username: String,
    email: String,
    role: String,
    is_admin: bool,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<User> for GraphQLUser {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            email: user.email,
            role: if user.is_admin {
                "admin".to_string()
            } else {
                "user".to_string()
            },
            is_admin: user.is_admin,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLGameSystem {
    id: uuid::Uuid,
    slug: String,
    title: String,
    manifest_url: String,
    version: String,
    installed_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<GameSystem> for GraphQLGameSystem {
    fn from(system: GameSystem) -> Self {
        Self {
            id: system.id,
            slug: system.slug,
            title: system.title,
            manifest_url: system.manifest_url,
            version: system.version,
            installed_by: system.installed_by,
            created_at: system.created_at,
            updated_at: system.updated_at,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateWorldInput {
    name: String,
    description: Option<String>,
    game_system_id: Option<String>,
    interface_pack_id: Option<String>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorld {
    id: uuid::Uuid,
    name: String,
    description: Option<String>,
    game_system_id: Option<String>,
    interface_pack_id: Option<String>,
    scenes: Vec<String>,
    actors: Vec<String>,
    tokens: Vec<String>,
    events: Vec<String>,
    game_system: Option<String>,
    interface_pack: Option<String>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<World> for GraphQLWorld {
    fn from(world: World) -> Self {
        Self {
            id: world.id,
            name: world.name,
            description: world.description,
            game_system_id: world.game_system_id,
            interface_pack_id: world.interface_pack_id,
            scenes: Vec::new(),
            actors: Vec::new(),
            tokens: Vec::new(),
            events: Vec::new(),
            game_system: None,
            interface_pack: None,
            created_by: world.created_by,
            updated_by: world.updated_by,
            created_at: world.created_at,
            updated_at: world.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorldToken {
    id: String,
    world_id: uuid::Uuid,
    x: f64,
    y: f64,
    z: f64,
    label: Option<String>,
    health: Option<i32>,
    max_health: Option<i32>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    schema_version: i32,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<WorldToken> for GraphQLWorldToken {
    fn from(token: WorldToken) -> Self {
        Self {
            id: token.id,
            world_id: token.world_id,
            x: token.x,
            y: token.y,
            z: token.z,
            label: token.label,
            health: token.health,
            max_health: token.max_health,
            created_by: token.created_by,
            updated_by: token.updated_by,
            schema_version: token.schema_version,
            created_at: token.created_at,
            updated_at: token.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLWorldEvent {
    id: i64,
    world_id: uuid::Uuid,
    event_code: i32,
    token_event: Option<Json<serde_json::Value>>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    schema_version: i32,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<WorldEvent> for GraphQLWorldEvent {
    fn from(event: WorldEvent) -> Self {
        Self {
            id: event.id,
            world_id: event.world_id,
            event_code: event.event_code,
            token_event: event.token_event.map(Json),
            created_by: event.created_by,
            updated_by: event.updated_by,
            schema_version: event.schema_version,
            created_at: event.created_at,
            updated_at: event.updated_at,
        }
    }
}

#[derive(Enum, Debug, Copy, Clone, Eq, PartialEq)]
pub enum GraphQLPolicyEffect {
    Allow,
    Deny,
}

impl From<PolicyEffectEnum> for GraphQLPolicyEffect {
    fn from(effect: PolicyEffectEnum) -> Self {
        match effect {
            PolicyEffectEnum::Allow => Self::Allow,
            PolicyEffectEnum::Deny => Self::Deny,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPolicy {
    id: uuid::Uuid,
    effect: GraphQLPolicyEffect,
    resources: Vec<String>,
    created_by: uuid::Uuid,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<Policy> for GraphQLPolicy {
    fn from(policy: Policy) -> Self {
        Self {
            id: policy.id,
            effect: policy.effect.into(),
            resources: policy.resources.into_iter().flatten().collect(),
            created_by: policy.created_by,
            updated_by: policy.updated_by,
            created_at: policy.created_at,
            updated_at: policy.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLPlaceholderDomainObject {
    schema_version: String,
    status: String,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLExportManifest {
    schema_version: String,
    exported_at: DateTime<Utc>,
    worlds: i32,
    world_tokens: i32,
    world_events: i32,
    policies: i32,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLExportMyDataPayload {
    manifest: GraphQLExportManifest,
    user: GraphQLUser,
    worlds: Vec<GraphQLWorld>,
    world_tokens: Vec<GraphQLWorldToken>,
    world_events: Vec<GraphQLWorldEvent>,
    policies: Vec<GraphQLPolicy>,
    scenes: Vec<GraphQLPlaceholderDomainObject>,
    actors: Vec<GraphQLPlaceholderDomainObject>,
    asset_packs: Vec<GraphQLPlaceholderDomainObject>,
    game_systems: Vec<GraphQLPlaceholderDomainObject>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDeleteMyDataPayload {
    status: String,
    message: String,
    worlds_deleted: i64,
    world_tokens_deleted: i64,
    world_events_deleted: i64,
    policies_deleted: i64,
    oauth_links_deleted: i64,
    sessions_deleted: i64,
    login_challenges_deleted: i64,
    oauth_link_challenges_deleted: i64,
    users_deleted: i64,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDeleteWorldPayload {
    id: uuid::Uuid,
    status: String,
    message: String,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLDiskUsageBreakdown {
    total_bytes: i64,
    worlds_bytes: i64,
    assets_bytes: i64,
    client_bytes: i64,
    databases_bytes: i64,
    modules_bytes: i64,
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

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAdminStats {
    total_users: i64,
    total_worlds: i64,
    total_world_tokens: i64,
    total_world_events: i64,
    total_policies: i64,
    disk_usage_bytes: i64,
    disk_usage: GraphQLDiskUsageBreakdown,
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

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAdminWelcomeSummary {
    total_users: i64,
    total_worlds: i64,
    total_tokens: i64,
    total_events: i64,
    disk_usage: i64,
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

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLOAuthProvider {
    id: uuid::Uuid,
    provider_key: String,
    display_name: String,
    authorization_url: String,
    token_url: String,
    userinfo_url: Option<String>,
    scopes: Vec<String>,
    oauth_client_id: Option<String>,
    configured: bool,
    enabled: bool,
    has_client_secret: bool,
    updated_at: chrono::NaiveDateTime,
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

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLManifestEntry {
    key: String,
    value: String,
    editable: bool,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLSystemManifest {
    path: String,
    schema_version: String,
    updated_at: DateTime<Utc>,
    entries: Vec<GraphQLManifestEntry>,
}

impl GraphQLSystemManifest {
    fn from_document(path: String, manifest: SystemManifestDocument) -> Self {
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

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAuthSecuritySettings {
    two_factor_required_for_all_users: bool,
    updated_at: chrono::NaiveDateTime,
}

impl From<AuthSecuritySetting> for GraphQLAuthSecuritySettings {
    fn from(value: AuthSecuritySetting) -> Self {
        Self {
            two_factor_required_for_all_users: value.two_factor_required_for_all_users,
            updated_at: value.updated_at,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAdminBootstrapSettings {
    setup_completed: bool,
    admin_code_generated_at: Option<chrono::NaiveDateTime>,
    setup_completed_at: Option<chrono::NaiveDateTime>,
    updated_at: chrono::NaiveDateTime,
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

#[derive(InputObject, Debug, Clone, Default)]
pub struct GraphQLOAuthProviderConfigInput {
    display_name: Option<String>,
    oauth_client_id: Option<String>,
    oauth_client_secret: Option<String>,
    enabled: Option<bool>,
    userinfo_url: Option<String>,
    scopes: Option<Vec<String>>,
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

impl From<UserDataDeleteSummary> for GraphQLDeleteMyDataPayload {
    fn from(summary: UserDataDeleteSummary) -> Self {
        Self {
            status: "deleted".to_string(),
            message: "User profile and owned data were permanently deleted".to_string(),
            worlds_deleted: summary.worlds_deleted,
            world_tokens_deleted: summary.world_tokens_deleted,
            world_events_deleted: summary.world_events_deleted,
            policies_deleted: summary.policies_deleted,
            oauth_links_deleted: summary.oauth_links_deleted,
            sessions_deleted: summary.sessions_deleted,
            login_challenges_deleted: summary.login_challenges_deleted,
            oauth_link_challenges_deleted: summary.oauth_link_challenges_deleted,
            users_deleted: summary.users_deleted,
        }
    }
}

impl From<UserDataExport> for GraphQLExportMyDataPayload {
    fn from(export: UserDataExport) -> Self {
        Self {
            manifest: GraphQLExportManifest {
                schema_version: export.manifest.schema_version.to_string(),
                exported_at: export.manifest.exported_at,
                worlds: export.manifest.counts.worlds as i32,
                world_tokens: export.manifest.counts.world_tokens as i32,
                world_events: export.manifest.counts.world_events as i32,
                policies: export.manifest.counts.policies as i32,
            },
            user: GraphQLUser {
                id: export.user.id,
                username: export.user.username,
                email: export.user.email,
                role: export.user.role,
                is_admin: export.user.is_admin,
                created_at: export.user.created_at,
                updated_at: export.user.updated_at,
            },
            worlds: export.worlds.into_iter().map(GraphQLWorld::from).collect(),
            world_tokens: export
                .world_tokens
                .into_iter()
                .map(GraphQLWorldToken::from)
                .collect(),
            world_events: export
                .world_events
                .into_iter()
                .map(GraphQLWorldEvent::from)
                .collect(),
            policies: export
                .policies
                .into_iter()
                .map(GraphQLPolicy::from)
                .collect(),
            scenes: export
                .scenes
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            actors: export
                .actors
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            asset_packs: export
                .asset_packs
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
            game_systems: export
                .game_systems
                .into_iter()
                .map(|item| GraphQLPlaceholderDomainObject {
                    schema_version: item.schema_version.to_string(),
                    status: item.status.to_string(),
                })
                .collect(),
        }
    }
}

const MIN_WORLD_NAME_LEN: usize = 3;
const MAX_WORLD_NAME_LEN: usize = 64;
const MAX_WORLD_DESCRIPTION_LEN: usize = 600;
const MAX_WORLD_REFERENCE_ID_LEN: usize = 64;

#[derive(Debug, Clone)]
struct PreparedWorldInput {
    name: String,
    description: Option<String>,
    game_system_id: Option<String>,
    interface_pack_id: Option<String>,
}

fn app_state<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a AppState> {
    ctx.data::<AppState>()
        .map_err(|_| Error::new("Application state unavailable"))
}

fn authenticated_user<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a AuthenticatedUser> {
    ctx.data::<AuthenticatedUser>()
        .map_err(|_| Error::new("Authentication required"))
}

fn admin_user<'a>(ctx: &'a Context<'_>) -> GraphQLResult<&'a AuthenticatedUser> {
    let user = authenticated_user(ctx)?;
    if user.is_admin {
        Ok(user)
    } else {
        Err(Error::new("Admin privileges required"))
    }
}

fn normalize_world_name(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn validate_world_name(name: &str) -> Result<(), String> {
    if name.len() < MIN_WORLD_NAME_LEN || name.len() > MAX_WORLD_NAME_LEN {
        return Err(format!(
            "World name must be between {MIN_WORLD_NAME_LEN} and {MAX_WORLD_NAME_LEN} characters"
        ));
    }

    if !name.chars().all(|ch| {
        ch.is_ascii_alphanumeric()
            || matches!(
                ch,
                ' ' | '\'' | '-' | '_' | '.' | ',' | ':' | '!' | '?' | '(' | ')'
            )
    }) {
        return Err(
            "World name may only contain letters, numbers, spaces, apostrophes, and - _ . , : ! ? ( )"
                .to_string(),
        );
    }

    Ok(())
}

fn validate_optional_reference_id(label: &str, value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };

    if value.len() > MAX_WORLD_REFERENCE_ID_LEN {
        return Err(format!(
            "{label} must be {MAX_WORLD_REFERENCE_ID_LEN} characters or fewer"
        ));
    }

    if !value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
    {
        return Err(format!(
            "{label} may only contain letters, numbers, '-', '_', '.', and ':'"
        ));
    }

    Ok(())
}

fn prepare_world_input(input: GraphQLCreateWorldInput) -> Result<PreparedWorldInput, String> {
    let name = normalize_world_name(&input.name);
    validate_world_name(&name)?;

    let description = normalize_optional_text(input.description);
    if description
        .as_ref()
        .is_some_and(|value| value.len() > MAX_WORLD_DESCRIPTION_LEN)
    {
        return Err(format!(
            "World description must be {MAX_WORLD_DESCRIPTION_LEN} characters or fewer"
        ));
    }

    let game_system_id = normalize_optional_text(input.game_system_id);
    validate_optional_reference_id("Game system ID", game_system_id.as_deref())?;

    let interface_pack_id = normalize_optional_text(input.interface_pack_id);
    validate_optional_reference_id("Interface pack ID", interface_pack_id.as_deref())?;

    Ok(PreparedWorldInput {
        name,
        description,
        game_system_id,
        interface_pack_id,
    })
}

fn world_write_error(error: DieselError, fallback_message: &str) -> Error {
    match error {
        DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
            Error::new("You already own a world with this name")
        }
        _ => Error::new(fallback_message),
    }
}

async fn load_game_systems(state: &AppState) -> GraphQLResult<Vec<GameSystem>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        game_systems::table
            .order(game_systems::title.asc())
            .select(GameSystem::as_select())
            .load::<GameSystem>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query game systems"))
}

async fn load_owned_worlds(state: &AppState, user_id: uuid::Uuid) -> GraphQLResult<Vec<World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        worlds::table
            .filter(worlds::created_by.eq(user_id))
            .order(worlds::created_at.desc())
            .select(World::as_select())
            .load::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query worlds"))
}

async fn load_all_worlds(state: &AppState) -> GraphQLResult<Vec<World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        worlds::table
            .order((worlds::updated_at.desc(), worlds::created_at.desc()))
            .select(World::as_select())
            .load::<World>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query worlds"))
}

async fn load_owned_world_tokens(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<WorldToken>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_tokens::table
            .filter(world_tokens::created_by.eq(user_id))
            .order(world_tokens::created_at.desc())
            .select(WorldToken::as_select())
            .load::<WorldToken>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world tokens"))
}

async fn load_owned_world_events(
    state: &AppState,
    user_id: uuid::Uuid,
) -> GraphQLResult<Vec<WorldEvent>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        world_events::table
            .filter(world_events::created_by.eq(user_id))
            .order(world_events::created_at.desc())
            .select(WorldEvent::as_select())
            .load::<WorldEvent>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world events"))
}

async fn load_owned_policies(state: &AppState, user_id: uuid::Uuid) -> GraphQLResult<Vec<Policy>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        policies::table
            .filter(policies::created_by.eq(user_id))
            .order(policies::created_at.desc())
            .select(Policy::as_select())
            .load::<Policy>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query policies"))
}

async fn load_visible_world_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    is_admin: bool,
    world_id: uuid::Uuid,
) -> GraphQLResult<Option<World>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        worlds::table
            .filter(worlds::id.eq(world_id))
            .select(World::as_select())
            .first::<World>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world"))?;

    match found {
        Some(world) if !is_admin && world.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

async fn load_owned_world_token_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    token_id: String,
) -> GraphQLResult<Option<WorldToken>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        world_tokens::table
            .filter(world_tokens::id.eq(token_id))
            .select(WorldToken::as_select())
            .first::<WorldToken>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world token"))?;

    match found {
        Some(token) if token.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

async fn load_owned_world_event_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    event_id: i64,
) -> GraphQLResult<Option<WorldEvent>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        world_events::table
            .filter(world_events::id.eq(event_id))
            .select(WorldEvent::as_select())
            .first::<WorldEvent>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query world event"))?;

    match found {
        Some(event) if event.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

async fn load_owned_policy_by_id(
    state: &AppState,
    user_id: uuid::Uuid,
    policy_id: uuid::Uuid,
) -> GraphQLResult<Option<Policy>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let found = tokio::task::spawn_blocking(move || {
        policies::table
            .filter(policies::id.eq(policy_id))
            .select(Policy::as_select())
            .first::<Policy>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query policy"))?;

    match found {
        Some(policy) if policy.created_by != user_id => Err(Error::new("Forbidden")),
        other => Ok(other),
    }
}

#[allow(dead_code)]
async fn load_game_system_by_id(
    state: &AppState,
    system_id: uuid::Uuid,
) -> GraphQLResult<Option<GameSystem>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        game_systems::table
            .filter(game_systems::id.eq(system_id))
            .select(GameSystem::as_select())
            .first::<GameSystem>(&mut conn)
            .optional()
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to query game system"))
}

// ========== Phase 3.5: Scene System GraphQL Types ==========

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLScene {
    scene_id: uuid::Uuid,
    world_id: uuid::Uuid,
    name: String,
    description: Option<String>,
    #[graphql(name = "type")]
    type_: String,
    grid_size: i32,
    grid_type: String,
    width: i32,
    height: i32,
    metadata: Option<Json<serde_json::Value>>,
    owner_id: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::Scene> for GraphQLScene {
    fn from(scene: crate::models::Scene) -> Self {
        Self {
            scene_id: scene.scene_id,
            world_id: scene.world_id,
            name: scene.name,
            description: scene.description,
            type_: scene.type_,
            grid_size: scene.grid_size,
            grid_type: scene.grid_type,
            width: scene.width,
            height: scene.height,
            metadata: scene.metadata.map(Json),
            owner_id: scene.owner_id,
            created_at: scene.created_at,
            updated_at: scene.updated_at,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateSceneInput {
    world_id: uuid::Uuid,
    name: String,
    description: Option<String>,
    #[graphql(name = "type")]
    type_: Option<String>,
    grid_size: Option<i32>,
    grid_type: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    metadata: Option<Json<serde_json::Value>>,
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateSceneInput {
    name: Option<String>,
    description: Option<String>,
    grid_size: Option<i32>,
    grid_type: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    metadata: Option<Json<serde_json::Value>>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLToken {
    token_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    actor_id: Option<uuid::Uuid>,
    x: f64,
    y: f64,
    rotation: f64,
    scale: f64,
    metadata: Option<Json<serde_json::Value>>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::Token> for GraphQLToken {
    fn from(token: crate::models::Token) -> Self {
        Self {
            token_id: token.token_id,
            scene_id: token.scene_id,
            actor_id: token.actor_id,
            x: token.x,
            y: token.y,
            rotation: token.rotation,
            scale: token.scale,
            metadata: token.metadata.map(Json),
            created_at: token.created_at,
            updated_at: token.updated_at,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpsertTokenInput {
    token_id: Option<uuid::Uuid>,
    scene_id: uuid::Uuid,
    actor_id: Option<uuid::Uuid>,
    x: Option<f64>,
    y: Option<f64>,
    rotation: Option<f64>,
    scale: Option<f64>,
    metadata: Option<Json<serde_json::Value>>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLFogMask {
    fog_id: uuid::Uuid,
    scene_id: uuid::Uuid,
    bitmap_data_base64: String,
    version: i32,
    width: i32,
    height: i32,
    updated_by: uuid::Uuid,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<crate::models::FogMask> for GraphQLFogMask {
    fn from(fog: crate::models::FogMask) -> Self {
        Self {
            fog_id: fog.fog_id,
            scene_id: fog.scene_id,
            bitmap_data_base64: fog.bitmap_data_base64(),
            version: fog.version,
            width: fog.width,
            height: fog.height,
            updated_by: fog.updated_by,
            created_at: fog.created_at,
            updated_at: fog.updated_at,
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateFogMaskInput {
    scene_id: uuid::Uuid,
    bitmap_data_base64: String,
    width: i32,
    height: i32,
}

// ========== Phase 4: World Token GraphQL Types ==========

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateWorldTokenInput {
    world_id: uuid::Uuid,
    label: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    health: Option<i32>,
    max_health: Option<i32>,
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpsertWorldTokenInput {
    world_id: uuid::Uuid,
    token_id: Option<String>,
    label: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    health: Option<i32>,
    max_health: Option<i32>,
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLMoveTokenInput {
    token_id: String,
    x: f64,
    y: f64,
    z: Option<f64>,
}

#[derive(Default)]
pub struct WorldTokenMutation;

#[async_graphql::Object]
impl WorldTokenMutation {
    async fn create_world_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWorldTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = uuid::Uuid::now_v7().to_string();
        let world_id = input.world_id;
        let label = input.label;
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let z = input.z.unwrap_or(0.0);
        let health = input.health;
        let max_health = input.max_health;

        let created_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;
            
            diesel::insert_into(world_tokens::table)
                .values((
                    world_tokens::id.eq(&token_id),
                    world_tokens::world_id.eq(world_id),
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::schema_version.eq(1),
                    world_tokens::created_at.eq(now),
                    world_tokens::updated_at.eq(now),
                    world_tokens::created_by.eq(user_id),
                    world_tokens::updated_by.eq(user_id),
                ))
                .returning(crate::models::WorldToken::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create world token"))?;

        Ok(GraphQLWorldToken::from(created_token))
    }

    async fn upsert_world_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpsertWorldTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input.token_id.unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let world_id = input.world_id;
        let label = input.label;
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let z = input.z.unwrap_or(0.0);
        let health = input.health;
        let max_health = input.max_health;

        let upserted_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;
            
            diesel::insert_into(world_tokens::table)
                .values((
                    world_tokens::id.eq(&token_id),
                    world_tokens::world_id.eq(world_id),
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::schema_version.eq(1),
                    world_tokens::created_at.eq(now),
                    world_tokens::updated_at.eq(now),
                    world_tokens::created_by.eq(user_id),
                    world_tokens::updated_by.eq(user_id),
                ))
                .on_conflict(world_tokens::id)
                .do_update()
                .set((
                    world_tokens::x.eq(x),
                    world_tokens::y.eq(y),
                    world_tokens::z.eq(z),
                    world_tokens::label.eq(&label),
                    world_tokens::health.eq(health),
                    world_tokens::max_health.eq(max_health),
                    world_tokens::updated_by.eq(user_id),
                    world_tokens::updated_at.eq(now),
                ))
                .returning(crate::models::WorldToken::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to upsert world token"))?;

        Ok(GraphQLWorldToken::from(upserted_token))
    }

    async fn move_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLMoveTokenInput,
    ) -> GraphQLResult<GraphQLWorldToken> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input.token_id;
        let x = input.x;
        let y = input.y;
        let z = input.z.unwrap_or(0.0);

        let moved_token = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;
            
            diesel::update(
                world_tokens::table
                    .filter(world_tokens::id.eq(&token_id))
                    .filter(world_tokens::created_by.eq(user_id)),
            )
            .set((
                world_tokens::x.eq(x),
                world_tokens::y.eq(y),
                world_tokens::z.eq(z),
                world_tokens::updated_by.eq(user_id),
                world_tokens::updated_at.eq(now),
            ))
            .returning(crate::models::WorldToken::as_returning())
            .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to move token"))?;

        Ok(GraphQLWorldToken::from(moved_token))
    }

    async fn delete_world_token(
        &self,
        ctx: &Context<'_>,
        token_id: String,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::world_tokens;
            use diesel::prelude::*;
            diesel::delete(
                world_tokens::table
                    .filter(world_tokens::id.eq(&token_id))
                    .filter(world_tokens::created_by.eq(user_id)),
            )
            .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete token"))?;

        Ok(deleted > 0)
    }
}

#[derive(Default)]
pub struct SceneQuery;

#[async_graphql::Object]
impl SceneQuery {
    async fn scenes(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLScene>> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let scenes = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            scenes::table
                .filter(scenes::world_id.eq(world_id))
                .select(crate::models::Scene::as_select())
                .load::<crate::models::Scene>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load scenes"))?;

        Ok(scenes.into_iter().map(GraphQLScene::from).collect())
    }

    async fn scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLScene>> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(crate::models::Scene::as_select())
                .first::<crate::models::Scene>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load scene"))?;

        Ok(scene.map(GraphQLScene::from))
    }

    async fn tokens(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Vec<GraphQLToken>> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let tokens = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .select(crate::models::Token::as_select())
                .load::<crate::models::Token>(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load tokens"))?;

        Ok(tokens.into_iter().map(GraphQLToken::from).collect())
    }

    async fn fog_mask(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLFogMask>> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let fog_mask = tokio::task::spawn_blocking(move || {
            use crate::schema::fog_masks;
            fog_masks::table
                .filter(fog_masks::scene_id.eq(scene_id))
                .select(crate::models::FogMask::as_select())
                .first::<crate::models::FogMask>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load fog mask"))?;

        Ok(fog_mask.map(GraphQLFogMask::from))
    }
}

#[derive(Default)]
pub struct SceneMutation;

#[async_graphql::Object]
impl SceneMutation {
    async fn create_scene(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateSceneInput,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = uuid::Uuid::now_v7();
        let new_scene = crate::models::Scene {
            scene_id,
            world_id: input.world_id,
            name: input.name,
            description: input.description,
            type_: input.type_.unwrap_or_else(|| "battlemap".to_string()),
            grid_size: input.grid_size.unwrap_or(5),
            grid_type: input.grid_type.unwrap_or_else(|| "square".to_string()),
            width: input.width.unwrap_or(100),
            height: input.height.unwrap_or(100),
            metadata: input.metadata.map(|j| j.0),
            owner_id: user_id,
            created_at: now,
            updated_at: now,
        };

        let inserted_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;
            
            let values = (
                scenes::scene_id.eq(new_scene.scene_id),
                scenes::world_id.eq(new_scene.world_id),
                scenes::name.eq(&new_scene.name),
                scenes::description.eq(&new_scene.description),
                scenes::type_.eq(&new_scene.type_),
                scenes::grid_size.eq(new_scene.grid_size),
                scenes::grid_type.eq(&new_scene.grid_type),
                scenes::width.eq(new_scene.width),
                scenes::height.eq(new_scene.height),
                scenes::metadata.eq(&new_scene.metadata),
                scenes::owner_id.eq(new_scene.owner_id),
                scenes::created_at.eq(new_scene.created_at),
                scenes::updated_at.eq(new_scene.updated_at),
            );
            
            diesel::insert_into(scenes::table)
                .values(values)
                .returning(crate::models::Scene::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to create scene"))?;

        Ok(GraphQLScene::from(inserted_scene))
    }

    async fn update_scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
        input: GraphQLUpdateSceneInput,
    ) -> GraphQLResult<GraphQLScene> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let _now = Utc::now().naive_utc();

        let updated_scene = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;
            
            let update_data = crate::models::SceneUpdate {
                name: input.name,
                description: input.description,
                grid_size: input.grid_size,
                grid_type: input.grid_type,
                width: input.width,
                height: input.height,
                metadata: input.metadata.map(|j| j.0),
            };

            diesel::update(
                scenes::table
                    .filter(scenes::scene_id.eq(scene_id))
                    .filter(scenes::owner_id.eq(user_id)),
            )
            .set(update_data)
            .returning(crate::models::Scene::as_returning())
            .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update scene"))?;

        Ok(GraphQLScene::from(updated_scene))
    }

    async fn delete_scene(
        &self,
        ctx: &Context<'_>,
        scene_id: uuid::Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::scenes;
            use diesel::prelude::*;
            diesel::delete(
                scenes::table
                    .filter(scenes::scene_id.eq(scene_id))
                    .filter(scenes::owner_id.eq(user_id)),
            )
            .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete scene"))?;

        Ok(deleted > 0)
    }

    async fn upsert_token(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpsertTokenInput,
    ) -> GraphQLResult<GraphQLToken> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let token_id = input.token_id.unwrap_or_else(uuid::Uuid::now_v7);
        let scene_id = input.scene_id;
        let actor_id = input.actor_id;
        let x = input.x.unwrap_or(0.0);
        let y = input.y.unwrap_or(0.0);
        let rotation = input.rotation.unwrap_or(0.0);
        let scale = input.scale.unwrap_or(1.0);
        let metadata = input.metadata.map(|j| j.0);

        let upserted_token = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            use diesel::prelude::*;
            
            diesel::insert_into(tokens::table)
                .values((
                    tokens::token_id.eq(token_id),
                    tokens::scene_id.eq(scene_id),
                    tokens::actor_id.eq(actor_id),
                    tokens::x.eq(x),
                    tokens::y.eq(y),
                    tokens::rotation.eq(rotation),
                    tokens::scale.eq(scale),
                    tokens::metadata.eq(&metadata),
                    tokens::created_at.eq(now),
                    tokens::updated_at.eq(now),
                ))
                .on_conflict(tokens::token_id)
                .do_update()
                .set((
                    tokens::actor_id.eq(actor_id),
                    tokens::x.eq(x),
                    tokens::y.eq(y),
                    tokens::rotation.eq(rotation),
                    tokens::scale.eq(scale),
                    tokens::metadata.eq(&metadata),
                    tokens::updated_at.eq(now),
                ))
                .returning(crate::models::Token::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to upsert token"))?;

        Ok(GraphQLToken::from(upserted_token))
    }

    async fn delete_token(
        &self,
        ctx: &Context<'_>,
        token_id: uuid::Uuid,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let _auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let deleted = tokio::task::spawn_blocking(move || {
            use crate::schema::tokens;
            use diesel::prelude::*;
            diesel::delete(tokens::table.filter(tokens::token_id.eq(token_id)))
                .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete token"))?;

        Ok(deleted > 0)
    }

    async fn update_fog_mask(
        &self,
        ctx: &Context<'_>,
        input: GraphQLUpdateFogMaskInput,
    ) -> GraphQLResult<GraphQLFogMask> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let scene_id = input.scene_id;
        let bitmap_data_base64 = input.bitmap_data_base64;
        let width = input.width;
        let height = input.height;

        let updated_fog_mask = tokio::task::spawn_blocking(move || {
            use crate::schema::fog_masks;
            use diesel::prelude::*;
            
            let bitmap_bytes = base64::engine::general_purpose::STANDARD
                .decode(&bitmap_data_base64)
                .map_err(|_| DieselError::NotFound)?;

            diesel::insert_into(fog_masks::table)
                .values((
                    fog_masks::fog_id.eq(uuid::Uuid::now_v7()),
                    fog_masks::scene_id.eq(scene_id),
                    fog_masks::bitmap_data.eq(&bitmap_bytes),
                    fog_masks::version.eq(1),
                    fog_masks::width.eq(width),
                    fog_masks::height.eq(height),
                    fog_masks::updated_by.eq(user_id),
                    fog_masks::created_at.eq(now),
                    fog_masks::updated_at.eq(now),
                ))
                .on_conflict(fog_masks::scene_id)
                .do_update()
                .set((
                    fog_masks::bitmap_data.eq(&bitmap_bytes),
                    fog_masks::version.eq(fog_masks::version + 1),
                    fog_masks::width.eq(width),
                    fog_masks::height.eq(height),
                    fog_masks::updated_by.eq(user_id),
                    fog_masks::updated_at.eq(now),
                ))
                .returning(crate::models::FogMask::as_returning())
                .get_result(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to update fog mask"))?;

        Ok(GraphQLFogMask::from(updated_fog_mask))
    }
}

#[derive(Default)]
pub struct HealthcheckQuery;

#[async_graphql::Object]
impl HealthcheckQuery {
    async fn healthcheck(&self) -> bool {
        true
    }
}

#[derive(Default)]
pub struct UserQuery;

#[async_graphql::Object]
impl UserQuery {
    async fn me(&self, ctx: &Context<'_>) -> GraphQLResult<Option<GraphQLUser>> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let user = tokio::task::spawn_blocking(move || {
            users::table
                .filter(users::id.eq(user_id))
                .select(User::as_select())
                .first::<User>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to load user"))?;

        Ok(user.map(GraphQLUser::from))
    }

    async fn game_systems(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLGameSystem>> {
        let state = app_state(ctx)?;
        load_game_systems(state)
            .await
            .map(|items| items.into_iter().map(GraphQLGameSystem::from).collect())
    }

    async fn my_worlds(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_worlds(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorld::from).collect())
    }

    async fn world(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_visible_world_by_id(state, auth_user.user_id, auth_user.is_admin, id)
            .await
            .map(|item| item.map(GraphQLWorld::from))
    }

    async fn my_world_tokens(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorldToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_tokens(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorldToken::from).collect())
    }

    async fn world_token(
        &self,
        ctx: &Context<'_>,
        token_id: String,
    ) -> GraphQLResult<Option<GraphQLWorldToken>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_token_by_id(state, auth_user.user_id, token_id)
            .await
            .map(|item| item.map(GraphQLWorldToken::from))
    }

    async fn my_world_events(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorldEvent>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_events(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLWorldEvent::from).collect())
    }

    async fn world_event(
        &self,
        ctx: &Context<'_>,
        event_id: i64,
    ) -> GraphQLResult<Option<GraphQLWorldEvent>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_world_event_by_id(state, auth_user.user_id, event_id)
            .await
            .map(|item| item.map(GraphQLWorldEvent::from))
    }

    async fn my_policies(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLPolicy>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_policies(state, auth_user.user_id)
            .await
            .map(|items| items.into_iter().map(GraphQLPolicy::from).collect())
    }

    async fn policy(
        &self,
        ctx: &Context<'_>,
        policy_id: uuid::Uuid,
    ) -> GraphQLResult<Option<GraphQLPolicy>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        load_owned_policy_by_id(state, auth_user.user_id, policy_id)
            .await
            .map(|item| item.map(GraphQLPolicy::from))
    }

    async fn export_my_data(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLExportMyDataPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        export_user_data_payload(state, auth_user.user_id)
            .await
            .map(GraphQLExportMyDataPayload::from)
            .map_err(Error::new)
    }
}

#[derive(Default)]
pub struct AdminQuery;

#[async_graphql::Object]
impl AdminQuery {
    async fn all_worlds(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLWorld>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_all_worlds(state)
            .await
            .map(|items| items.into_iter().map(GraphQLWorld::from).collect())
    }

    async fn admin_welcome_summary(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLAdminWelcomeSummary> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_admin_welcome_summary(state)
            .await
            .map(GraphQLAdminWelcomeSummary::from)
            .map_err(Error::new)
    }

    async fn admin_stats(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLAdminStats> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_admin_stats(state)
            .await
            .map(GraphQLAdminStats::from)
            .map_err(Error::new)
    }

    async fn system_manifest(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLSystemManifest> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        read_system_manifest(state)
            .map(|manifest| {
                GraphQLSystemManifest::from_document(
                    state.directories.manifest_file.clone(),
                    manifest,
                )
            })
            .map_err(Error::new)
    }

    async fn oauth_providers(&self, ctx: &Context<'_>) -> GraphQLResult<Vec<GraphQLOAuthProvider>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_oauth_providers(state)
            .await
            .map(|providers| {
                providers
                    .into_iter()
                    .map(GraphQLOAuthProvider::from)
                    .collect()
            })
            .map_err(Error::new)
    }

    async fn auth_security_settings(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<GraphQLAuthSecuritySettings> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_auth_security_settings(state)
            .await
            .map(GraphQLAuthSecuritySettings::from)
            .map_err(Error::new)
    }

    async fn admin_bootstrap_settings(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Option<GraphQLAdminBootstrapSettings>> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        load_admin_bootstrap_settings(state)
            .await
            .map(|item| item.map(GraphQLAdminBootstrapSettings::from))
            .map_err(Error::new)
    }
}

#[derive(Default)]
pub struct WorldMutation;

#[async_graphql::Object]
impl WorldMutation {
    async fn create_world(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateWorldInput,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let prepared_input = prepare_world_input(input).map_err(Error::new)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let now = Utc::now().naive_utc();

        let new_world = World {
            id: uuid::Uuid::now_v7(),
            name: prepared_input.name,
            description: prepared_input.description,
            game_system_id: prepared_input.game_system_id,
            interface_pack_id: prepared_input.interface_pack_id,
            created_by: auth_user.user_id,
            updated_by: auth_user.user_id,
            created_at: now,
            updated_at: now,
        };

        let inserted_world = new_world.clone();
        tokio::task::spawn_blocking(move || {
            diesel::insert_into(worlds::table)
                .values(&inserted_world)
                .execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|error| world_write_error(error, "Failed to create world"))?;

        Ok(GraphQLWorld::from(new_world))
    }

    async fn rename_world(
        &self,
        ctx: &Context<'_>,
        world_id: uuid::Uuid,
        world_name: String,
    ) -> GraphQLResult<GraphQLWorld> {
        let state = app_state(ctx)?;
        let user_id = authenticated_user(ctx)?.user_id;
        let world_name = normalize_world_name(&world_name);
        validate_world_name(&world_name).map_err(Error::new)?;
        let now = Utc::now().naive_utc();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let updated = tokio::task::spawn_blocking(move || {
            diesel::update(
                worlds::table
                    .filter(worlds::id.eq(world_id))
                    .filter(worlds::created_by.eq(user_id)),
            )
            .set((
                worlds::name.eq(world_name),
                worlds::updated_by.eq(user_id),
                worlds::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

            worlds::table
                .filter(worlds::id.eq(world_id))
                .select(World::as_select())
                .first::<World>(&mut conn)
                .optional()
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|error| world_write_error(error, "Failed to rename world"))?;

        match updated {
            Some(world) => Ok(GraphQLWorld::from(world)),
            None => Err(Error::new("Forbidden")),
        }
    }

    async fn delete_world(
        &self,
        ctx: &Context<'_>,
        id: uuid::Uuid,
    ) -> GraphQLResult<GraphQLDeleteWorldPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let existing = load_visible_world_by_id(state, auth_user.user_id, false, id).await?;

        let Some(world) = existing else {
            return Err(Error::new("World not found"));
        };

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            diesel::delete(worlds::table.filter(worlds::id.eq(id))).execute(&mut conn)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("Failed to delete world"))?;

        Ok(GraphQLDeleteWorldPayload {
            id: world.id,
            status: "deleted".to_string(),
            message: format!("World '{}' was deleted", world.name),
        })
    }
}

#[derive(Default)]
pub struct UserDataMutation;

#[async_graphql::Object]
impl UserDataMutation {
    async fn delete_my_data(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLDeleteMyDataPayload> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        delete_user_data_owned(state, auth_user.user_id)
            .await
            .map(GraphQLDeleteMyDataPayload::from)
            .map_err(Error::new)
    }
}

#[derive(Default)]
pub struct AdminMutation;

#[async_graphql::Object]
impl AdminMutation {
    async fn update_oauth_provider(
        &self,
        ctx: &Context<'_>,
        provider_id: uuid::Uuid,
        config: GraphQLOAuthProviderConfigInput,
    ) -> GraphQLResult<GraphQLOAuthProvider> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        persist_oauth_provider(state, provider_id, config.into())
            .await
            .map(GraphQLOAuthProvider::from)
            .map_err(Error::new)
    }

    async fn update_manifest_key(
        &self,
        ctx: &Context<'_>,
        key: String,
        value: String,
    ) -> GraphQLResult<GraphQLSystemManifest> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        persist_manifest_key(state, &key, &value)
            .map(|manifest| {
                GraphQLSystemManifest::from_document(
                    state.directories.manifest_file.clone(),
                    manifest,
                )
            })
            .map_err(Error::new)
    }

    async fn recalculate_disk_usage(&self, ctx: &Context<'_>) -> GraphQLResult<GraphQLAdminStats> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        let stats = load_admin_stats(state).await.map_err(Error::new)?;
        let disk_usage = calculate_disk_usage(state).map_err(Error::new)?;

        Ok(GraphQLAdminStats {
            disk_usage_bytes: disk_usage.total_bytes,
            disk_usage: disk_usage.into(),
            total_users: stats.total_users,
            total_worlds: stats.total_worlds,
            total_world_tokens: stats.total_world_tokens,
            total_world_events: stats.total_world_events,
            total_policies: stats.total_policies,
        })
    }

    async fn update_two_factor_policy(
        &self,
        ctx: &Context<'_>,
        required_for_all_users: bool,
    ) -> GraphQLResult<GraphQLAuthSecuritySettings> {
        let state = app_state(ctx)?;
        let _ = admin_user(ctx)?;
        persist_two_factor_policy(state, required_for_all_users)
            .await
            .map(GraphQLAuthSecuritySettings::from)
            .map_err(Error::new)
    }
}

#[derive(Default)]
pub struct SubscriptionRoot;

#[Subscription]
impl SubscriptionRoot {
    async fn tick(&self) -> impl Stream<Item = i32> {
        let mut value = 0;
        tokio_stream::StreamExt::map(
            IntervalStream::new(tokio::time::interval(Duration::from_secs(1))),
            move |_| {
                value += 1;
                value
            },
        )
    }
}

#[derive(MergedObject, Default)]
pub struct QueryRoot(HealthcheckQuery, UserQuery, AdminQuery, SceneQuery);

#[derive(MergedObject, Default)]
pub struct MutationRoot(WorldMutation, UserDataMutation, AdminMutation, SceneMutation, WorldTokenMutation);

pub type AppSchema = Schema<QueryRoot, MutationRoot, SubscriptionRoot>;

#[cfg(test)]
mod tests {
    use super::{GraphQLCreateWorldInput, prepare_world_input, validate_world_name};

    #[test]
    fn world_name_validation_rejects_invalid_characters() {
        let result = validate_world_name("Bad@World");

        assert_eq!(
            result,
            Err(
                "World name may only contain letters, numbers, spaces, apostrophes, and - _ . , : ! ? ( )"
                    .to_string(),
            )
        );
    }

    #[test]
    fn prepare_world_input_trims_optional_fields() {
        let prepared = prepare_world_input(GraphQLCreateWorldInput {
            name: "  The   Ember   Crown  ".to_string(),
            description: Some("  A fallen kingdom  ".to_string()),
            game_system_id: Some("  systemless-sandbox ".to_string()),
            interface_pack_id: Some(" guild-hall-default ".to_string()),
        })
        .expect("world input should be valid");

        assert_eq!(prepared.name, "The Ember Crown");
        assert_eq!(prepared.description.as_deref(), Some("A fallen kingdom"));
        assert_eq!(
            prepared.game_system_id.as_deref(),
            Some("systemless-sandbox")
        );
        assert_eq!(
            prepared.interface_pack_id.as_deref(),
            Some("guild-hall-default")
        );
    }
}
