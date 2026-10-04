//! `worldSystemSettings` and `setWorldSystemSetting` (spec 067 Story 1).
//!
//! The declaration and the value travel together, so a client renders a
//! world's settings form from this one read and no shared web file has to
//! know which system it is looking at.
//!
//! Any member may read: a setting says how the table plays, and a player's
//! sheet needs it to play that way. Only a Game Master may write, one setting
//! per call — there is no "absent means leave it" to get wrong when a call
//! names exactly the one thing it changes.

use async_graphql::{Context, Error, Json, Object, Result as GraphQLResult, SimpleObject};
use uuid::Uuid;

use crate::auth::world_membership::{actor_in_world, is_dm_of_world};
use crate::graphql::{app_state, authenticated_user};
use crate::play_pause::gate::refuse_world_if_paused;
use crate::state::AppState;
use crate::world_events::{EVENT_CODE_WORLD_SYSTEM_SETTING_CHANGED, record_world_event};
use crate::world_system_settings::{
    EffectiveSetting, declarations_for_system, read_effective, system_of_world, validate, write,
};

#[derive(SimpleObject, Debug, Clone)]
pub struct WorldSystemSettingOption {
    pub value: String,
    pub label: String,
}

/// One setting the world's system declares, and what this world plays by.
#[derive(SimpleObject, Debug, Clone)]
pub struct WorldSystemSetting {
    pub key: String,
    pub label: String,
    pub description: Option<String>,
    /// `boolean`, `integer`, `choice` or `text`.
    pub kind: String,
    pub options: Vec<WorldSystemSettingOption>,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub max_length: Option<i32>,
    pub default_value: Json<serde_json::Value>,
    pub value: Json<serde_json::Value>,
    /// True when nothing usable is stored and `value` is the default.
    pub is_default: bool,
}

fn present(setting: EffectiveSetting) -> WorldSystemSetting {
    let declaration = setting.declaration;
    WorldSystemSetting {
        key: declaration.id,
        label: declaration.label,
        description: declaration.description,
        kind: declaration.kind.as_str().to_string(),
        options: declaration
            .options
            .into_iter()
            .map(|option| WorldSystemSettingOption {
                value: option.value,
                label: option.label,
            })
            .collect(),
        min: declaration.min,
        max: declaration.max,
        max_length: declaration
            .max_length
            .and_then(|length| i32::try_from(length).ok()),
        default_value: Json(declaration.default),
        value: Json(setting.value),
        is_default: setting.is_default,
    }
}

#[derive(Default)]
pub struct WorldSystemSettingsQuery;

#[Object]
impl WorldSystemSettingsQuery {
    /// The settings this world's game system declares, each with the value
    /// the world plays by. Empty when the world has no system or the system
    /// declares none.
    async fn world_system_settings(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<WorldSystemSetting>> {
        let user = authenticated_user(ctx)?;
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let state = app_state(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            let actor = actor_in_world(&mut conn, user_id, is_admin, world_id);
            if actor.role.is_none() && !actor.is_site_admin {
                return Err(Error::new("Not a member of this world"));
            }
            let Some(system_id) =
                system_of_world(&mut conn, world_id).map_err(|_| Error::new("World not found"))?
            else {
                return Ok(Vec::new());
            };
            let declarations = declarations_for_system(&systems_dir, &system_id);
            let settings = read_effective(&mut conn, world_id, &system_id, declarations)
                .map_err(|e| Error::new(format!("Failed to read settings: {e}")))?;
            Ok(settings.into_iter().map(present).collect())
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}

#[derive(Default)]
pub struct WorldSystemSettingsMutation;

#[Object]
impl WorldSystemSettingsMutation {
    /// Set one of this world's system settings. Game Master only.
    async fn set_world_system_setting(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        key: String,
        value: Json<serde_json::Value>,
    ) -> GraphQLResult<WorldSystemSetting> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        set_setting_impl(state, user.user_id, user.is_admin, world_id, key, value.0).await
    }
}

pub async fn set_setting_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    key: String,
    value: serde_json::Value,
) -> GraphQLResult<WorldSystemSetting> {
    // How a world plays is a change to the world, which is what pausing
    // stops. Beside the role check and before any write (ADR-100).
    refuse_world_if_paused(state, world_id).await?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only this world's Game Master can change its settings",
        ));
    }

    let systems_dir = state.directories.systems_dir.clone();
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let system_id = system_of_world(&mut conn, world_id)
            .map_err(|_| Error::new("World not found"))?
            .ok_or_else(|| Error::new("This world has no game system"))?;
        let declaration = declarations_for_system(&systems_dir, &system_id)
            .into_iter()
            .find(|declaration| declaration.id == key)
            .ok_or_else(|| Error::new(format!("This world's system has no setting \"{key}\"")))?;

        validate(&system_id, &declaration, &value).map_err(Error::new)?;
        write(&mut conn, world_id, &system_id, &key, &value, user_id)
            .map_err(|e| Error::new(format!("Failed to store setting: {e}")))?;

        // After the write, and best-effort: a lost announcement costs a
        // client that is one read behind until it next mounts.
        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_WORLD_SYSTEM_SETTING_CHANGED,
            Some(serde_json::json!({ "key": key })),
            user_id,
        );

        Ok(present(EffectiveSetting {
            declaration,
            value,
            is_default: false,
        }))
    })
    .await
    .map_err(|e| Error::new(format!("Task failed: {e}")))?
}

#[cfg(test)]
#[path = "world_system_settings_tests.rs"]
mod tests;
