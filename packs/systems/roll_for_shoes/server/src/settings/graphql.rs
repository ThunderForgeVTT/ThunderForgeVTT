//! The pack's two root fields: read a world's settings, write them.
//!
//! This is the pack's entire network surface. Statuses and bought slots are
//! *not* here — both live in the character's existing JSON and go through the
//! host's `updateActorSystemData`, which this pack's validators already gate.
//! Adding mutations for them would duplicate an authorisation path that works.
//!
//! Rolling is not here either: difficulty dice go through the host's existing
//! `rollDice` with a `(BAND)d6` formula binding, exactly as a skill roll goes
//! through it with `(LEVEL)d6`. The dice stay server-rolled and auditable, and
//! no new formula syntax is invented.
//!
//! See `specs/062-roll-for-shoes-extras/contracts/graphql.md`.

use async_graphql::{Context, Error, InputObject, Object, Result as GraphQLResult, SimpleObject};
use uuid::Uuid;

use thunderforge_server::auth::world_membership::{is_dm_of_world, require_world_member};
use thunderforge_server::graphql::{app_state, authenticated_user};
use thunderforge_server::play_pause::gate::refuse_world_if_paused;
use thunderforge_server::state::AppState;

use super::{read_or_default, upsert, validate, StartingSkill, UpsertSettings, WorldSettings};

#[derive(SimpleObject, Debug, Clone)]
pub struct RollForShoesStartingSkill {
    pub name: String,
    pub level: i32,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct RollForShoesWorldSettings {
    pub world_id: Uuid,
    /// One of `free`, `rolled`, `target`.
    pub difficulty_mode: String,
    pub tie_succeeds: bool,
    pub statuses_enabled: bool,
    pub skill_slots_enabled: bool,
    /// Empty means the core default, "Do Anything 1".
    pub starting_skills: Vec<RollForShoesStartingSkill>,
}

fn present(world_id: Uuid, settings: WorldSettings) -> RollForShoesWorldSettings {
    RollForShoesWorldSettings {
        world_id,
        difficulty_mode: settings.difficulty_mode.as_str().to_string(),
        tie_succeeds: settings.tie_succeeds,
        statuses_enabled: settings.statuses_enabled,
        skill_slots_enabled: settings.skill_slots_enabled,
        starting_skills: settings
            .starting_skills
            .into_iter()
            .map(|s| RollForShoesStartingSkill {
                name: s.name,
                level: s.level as i32,
            })
            .collect(),
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct RollForShoesStartingSkillInput {
    pub name: String,
    pub level: i32,
}

/// Every field is required, because this is a whole-row upsert.
///
/// The panel reads the settings, changes one, and sends all five back. An
/// input of optionals would make "absent" ambiguous between "leave it" and
/// "clear it", and the spec's guarantee that enabling one setting never
/// silently changes another is easiest to hold when every call states all
/// five.
#[derive(InputObject, Debug, Clone)]
pub struct UpdateRollForShoesWorldSettingsInput {
    pub world_id: Uuid,
    pub difficulty_mode: String,
    pub tie_succeeds: bool,
    pub statuses_enabled: bool,
    pub skill_slots_enabled: bool,
    pub starting_skills: Vec<RollForShoesStartingSkillInput>,
}

#[derive(Default)]
pub struct RollForShoesSettingsQuery;

#[Object]
impl RollForShoesSettingsQuery {
    /// This world's optional rules.
    ///
    /// Any member may read. The settings describe how the table plays and
    /// every player needs them to roll; there is nothing here to keep from a
    /// player. A world with no row reads as every default rather than as an
    /// error, which is what makes the whole feature opt-in.
    async fn roll_for_shoes_world_settings(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<RollForShoesWorldSettings> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        read_settings_impl(state, user.user_id, world_id).await
    }
}

pub async fn read_settings_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<RollForShoesWorldSettings> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let settings = tokio::task::spawn_blocking(move || -> Result<WorldSettings, String> {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You must be a member of this world".to_string())?;
        read_or_default(&mut conn, world_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| Error::new(e.to_string()))?
    .map_err(Error::new)?;

    Ok(present(world_id, settings))
}

#[derive(Default)]
pub struct RollForShoesSettingsMutation;

#[Object]
impl RollForShoesSettingsMutation {
    /// Set this world's optional rules. Game Master only.
    async fn update_roll_for_shoes_world_settings(
        &self,
        ctx: &Context<'_>,
        input: UpdateRollForShoesWorldSettingsInput,
    ) -> GraphQLResult<RollForShoesWorldSettings> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        write_settings_impl(state, user.user_id, user.is_admin, input).await
    }
}

pub async fn write_settings_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: UpdateRollForShoesWorldSettingsInput,
) -> GraphQLResult<RollForShoesWorldSettings> {
    let world_id = input.world_id;

    // Configuring a world's rules is a change to the world, which is what
    // pausing stops. Beside the role check and before any write (ADR-100).
    refuse_world_if_paused(state, world_id).await?;

    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only this world's Game Master can change its rules",
        ));
    }

    let starting_skills: Vec<StartingSkill> = input
        .starting_skills
        .into_iter()
        .map(|s| StartingSkill {
            name: s.name,
            level: s.level as i64,
        })
        .collect();

    let mode = validate(&input.difficulty_mode, &starting_skills)
        .map_err(|e| Error::new(e.to_string()))?;

    let values = UpsertSettings {
        world_id,
        difficulty_mode: mode.as_str().to_string(),
        tie_succeeds: input.tie_succeeds,
        statuses_enabled: input.statuses_enabled,
        skill_slots_enabled: input.skill_slots_enabled,
        starting_skills: serde_json::to_value(&starting_skills)
            .map_err(|e| Error::new(e.to_string()))?,
        updated_by: Some(user_id),
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let settings = tokio::task::spawn_blocking(move || upsert(&mut conn, values))
        .await
        .map_err(|e| Error::new(e.to_string()))?
        .map_err(|e| Error::new(e.to_string()))?;

    // No world event and no subscription. A settings change reaches other
    // clients the next time they read, which for a sheet is its next mount —
    // a Game Master changing the rules mid-roll is not a case worth a live
    // channel.
    Ok(present(world_id, settings))
}
