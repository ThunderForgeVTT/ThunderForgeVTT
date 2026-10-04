//! The GraphQL for the table's standing difficulty.
//!
//! One read and two writes, merged into the application's schema roots in
//! `apps/server/src/schema_roots.rs` beside the settings fields.
//!
//! Unlike a settings change, these writes **are** announced: a difficulty the
//! Game Master sets is something every player is waiting on right now, with a
//! sheet open and a hand on the roll button. So each write records a world
//! event under the pack's own code ([`EVENT_CODE_TABLE_DIFFICULTY`]) and every
//! sheet that hears it reads again. The event carries no number — the read
//! is the one place a client learns it, and the read checks membership.

use async_graphql::{Context, Error, InputObject, Object, Result as GraphQLResult, SimpleObject};
use rand::SeedableRng;
use uuid::Uuid;

use thunderforge_server::auth::world_membership::{is_dm_of_world, require_world_member};
use thunderforge_server::graphql::mutations_roll::{roll_dice_impl, RollDiceInput};
use thunderforge_server::graphql::{app_state, authenticated_user};
use thunderforge_server::play_pause::gate::refuse_world_if_paused;
use thunderforge_server::state::AppState;
use thunderforge_server::world_events::record_world_event;

use super::EVENT_CODE_TABLE_DIFFICULTY;
use super::{clear, plan, read, set, Plan, SetDifficulty, TableDifficulty};
use crate::settings::read_or_default;

/// What every sheet at the table reads before a roll.
#[derive(SimpleObject, Debug, Clone, PartialEq, Eq)]
pub struct RollForShoesTableDifficulty {
    pub world_id: Uuid,
    /// The number to beat. `null` when the Game Master has set none, which is
    /// when a sheet lets its player enter one.
    pub target: Option<i32>,
    /// How hard the Game Master called it, when they called it by name.
    pub band: Option<String>,
    /// The Game Master's dice, when the server rolled them. Sum to `target`.
    pub gm_dice: Option<Vec<i32>>,
    /// Whether the caller is the one who may set it. Asked of the server
    /// rather than worked out by the sheet, which is told only who may edit
    /// the character and that is a different question.
    pub can_set: bool,
}

fn present(
    world_id: Uuid,
    difficulty: Option<TableDifficulty>,
    can_set: bool,
) -> RollForShoesTableDifficulty {
    match difficulty {
        Some(difficulty) => RollForShoesTableDifficulty {
            world_id,
            target: Some(difficulty.target),
            band: difficulty.band.map(|band| band.as_str().to_string()),
            gm_dice: difficulty.gm_dice,
            can_set,
        },
        None => RollForShoesTableDifficulty {
            world_id,
            target: None,
            band: None,
            gm_dice: None,
            can_set,
        },
    }
}

/// Exactly one of `target` and `band`. See `table::plan` for what each means
/// in each difficulty mode.
#[derive(InputObject, Debug, Clone)]
pub struct SetRollForShoesTableDifficultyInput {
    pub world_id: Uuid,
    pub target: Option<i32>,
    pub band: Option<String>,
}

#[derive(Default)]
pub struct RollForShoesTableQuery;

#[Object]
impl RollForShoesTableQuery {
    /// What has to be beaten at this table right now.
    ///
    /// Any member may read: it is said aloud at the table, and a player
    /// cannot roll without it.
    async fn roll_for_shoes_table_difficulty(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<RollForShoesTableDifficulty> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        read_difficulty_impl(state, user.user_id, user.is_admin, world_id).await
    }
}

pub async fn read_difficulty_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<RollForShoesTableDifficulty> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let difficulty =
        tokio::task::spawn_blocking(move || -> Result<Option<TableDifficulty>, String> {
            require_world_member(&mut conn, user_id, world_id)
                .map_err(|_| "You must be a member of this world".to_string())?;
            read(&mut conn, world_id).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| Error::new(e.to_string()))?
        .map_err(Error::new)?;

    let can_set = is_dm_of_world(state, user_id, is_admin, world_id).await?;
    Ok(present(world_id, difficulty, can_set))
}

#[derive(Default)]
pub struct RollForShoesTableMutation;

#[Object]
impl RollForShoesTableMutation {
    /// Say what has to be beaten. Game Master only.
    ///
    /// In a world that rolls for difficulty, naming a band rolls the Game
    /// Master's dice here, once, and stores what they came to.
    async fn set_roll_for_shoes_table_difficulty(
        &self,
        ctx: &Context<'_>,
        input: SetRollForShoesTableDifficultyInput,
    ) -> GraphQLResult<RollForShoesTableDifficulty> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        // Seeded per call from the OS-backed thread generator, as `rollDice`
        // seeds its own: the thread generator is not `Send` and could not be
        // held across the awaits below.
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        set_difficulty_impl(state, user.user_id, user.is_admin, input, &mut rng).await
    }

    /// Take the difficulty away, handing each sheet back its own entry.
    /// Game Master only.
    async fn clear_roll_for_shoes_table_difficulty(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<RollForShoesTableDifficulty> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        clear_difficulty_impl(state, user.user_id, user.is_admin, world_id).await
    }
}

/// The two refusals both writes share, in the order every gated write uses:
/// the pause first, then the role (ADR-100).
async fn only_the_game_master_of_a_running_world(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<()> {
    refuse_world_if_paused(state, world_id).await?;
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new(
            "Only this world's Game Master can set what has to be beaten",
        ));
    }
    Ok(())
}

pub async fn set_difficulty_impl<R: rand::Rng>(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: SetRollForShoesTableDifficultyInput,
    rng: &mut R,
) -> GraphQLResult<RollForShoesTableDifficulty> {
    let world_id = input.world_id;
    only_the_game_master_of_a_running_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let settings = tokio::task::spawn_blocking(move || read_or_default(&mut conn, world_id))
        .await
        .map_err(|e| Error::new(e.to_string()))?
        .map_err(|e| Error::new(e.to_string()))?;

    let planned = plan(
        settings.difficulty_mode,
        input.target,
        input.band.as_deref(),
    )
    .map_err(|e| Error::new(e.to_string()))?;

    let (target, band, gm_dice) = match planned {
        Plan::Named(number) => (number, None, None),
        Plan::Fixed(band) => (band.fixed_target(), Some(band), None),
        Plan::Rolled(band) => {
            // Down the same path every roll in the product takes, so the
            // Game Master's dice are in the world's roll records like anyone
            // else's and are resolved by the one resolver.
            let resolution = roll_dice_impl(
                state,
                user_id,
                RollDiceInput {
                    world_id,
                    formula: format!("{}d6", band.dice()),
                    bindings: None,
                },
                rng,
            )
            .await?;
            let faces: Vec<i32> = resolution.dice.iter().map(|die| die.final_value).collect();
            (faces.iter().sum(), Some(band), Some(faces))
        }
    };

    let values = SetDifficulty {
        world_id,
        target,
        band: band.map(|band| band.as_str().to_string()),
        gm_dice: gm_dice.map(|faces| serde_json::json!(faces)),
        set_by: Some(user_id),
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let difficulty = tokio::task::spawn_blocking(move || {
        let stored = set(&mut conn, values)?;
        // After the write, and best-effort: a lost announcement costs a
        // sheet that is one read behind, and each sheet reads again at the
        // moment it rolls.
        let _ = record_world_event(
            &mut conn,
            world_id,
            EVENT_CODE_TABLE_DIFFICULTY,
            Some(serde_json::json!({ "action": "set" })),
            user_id,
        );
        Ok::<_, diesel::result::Error>(stored)
    })
    .await
    .map_err(|e| Error::new(e.to_string()))?
    .map_err(|e| Error::new(e.to_string()))?;

    Ok(present(world_id, Some(difficulty), true))
}

pub async fn clear_difficulty_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<RollForShoesTableDifficulty> {
    only_the_game_master_of_a_running_world(state, user_id, is_admin, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        // Announced only when something was cleared: clearing nothing changes
        // nothing a sheet could be showing.
        if clear(&mut conn, world_id)? {
            let _ = record_world_event(
                &mut conn,
                world_id,
                EVENT_CODE_TABLE_DIFFICULTY,
                Some(serde_json::json!({ "action": "cleared" })),
                user_id,
            );
        }
        Ok::<_, diesel::result::Error>(())
    })
    .await
    .map_err(|e| Error::new(e.to_string()))?
    .map_err(|e| Error::new(e.to_string()))?;

    Ok(present(world_id, None, true))
}
