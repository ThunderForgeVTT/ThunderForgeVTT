//! `rollForShoesRollSkill`: the pack's own roll.
//!
//! The input names a world, a character, a skill and — only for a world where
//! the Game Master has set no difficulty — what the player says has to be
//! beaten. It has no field for a pool size, a total or a result: the level is
//! read off the character here, and the dice are the host's.

use async_graphql::{Context, Error, InputObject, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Text, Uuid as SqlUuid};
use rand::SeedableRng;
use uuid::Uuid;

use thunderforge_canvas_core::system_contribution::{RollFacts, Verdict};
use thunderforge_server::auth::actor_permissions::require_actor_permission;
use thunderforge_server::graphql::mutations_roll::{
    kept_dice, roll_and_settle, roll_value, RollDiceInput,
};
use thunderforge_server::graphql::types::{ActorPermissionLevel, GraphQLRollResolution};
use thunderforge_server::graphql::{app_state, authenticated_user};
use thunderforge_server::play_pause::gate::refuse_world_if_paused;
use thunderforge_server::rolls::facets::RollMeta;
use thunderforge_server::schema::{world_actor_system_data, world_actors};
use thunderforge_server::state::AppState;
use thunderforge_server::world_events::{record_world_event, EVENT_CODE_ACTOR_SHEET_CHANGED};
use thunderforge_server::world_system_settings::system_of_world;

use super::{adjudicate, modified_total, skills_of, status_modifier, Judging, SKILL_CHECK};
use crate::settings::read_or_default;
use crate::SYSTEM_ID;

/// What a failed roll earns.
const FAILURE_XP: i64 = 1;

#[derive(InputObject, Debug, Clone)]
pub struct RollForShoesRollSkillInput {
    pub world_id: Uuid,
    pub actor_id: Uuid,
    /// The skill's id on this character's sheet.
    pub skill_id: String,
    /// What the player says has to be beaten. Read only when the Game Master
    /// has set no difficulty for the table; theirs wins.
    pub opposition: Option<f64>,
}

/// A skill roll as the server made and judged it.
#[derive(SimpleObject, Debug, Clone)]
pub struct RollForShoesSkillRoll {
    /// The roll, with its outcome — null when nothing was to be beaten.
    pub roll: GraphQLRollResolution,
    /// The statuses' modifiers that counted, summed.
    pub modifier: i32,
    /// The dice and the modifier together: the number that was judged.
    pub total: f64,
    /// What it was judged against, whoever named it.
    pub opposition: Option<f64>,
    /// The experience this roll earned.
    pub xp_awarded: i32,
    /// The character's experience after it.
    pub xp: i32,
}

#[derive(Default)]
pub struct RollForShoesRollMutation;

#[Object]
impl RollForShoesRollMutation {
    /// Roll one of a character's skills. The server reads the level, rolls,
    /// judges, and pays a failure its experience.
    async fn roll_for_shoes_roll_skill(
        &self,
        ctx: &Context<'_>,
        input: RollForShoesRollSkillInput,
    ) -> GraphQLResult<RollForShoesSkillRoll> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        roll_skill_impl(state, user.user_id, user.is_admin, input, &mut rng).await
    }
}

#[derive(QueryableByName)]
struct XpAfter {
    #[diesel(sql_type = BigInt)]
    xp: i64,
}

/// Add to a character's experience where it is stored, in one statement, so
/// two rolls landing together both count. A character nobody has saved yet
/// gets their row here.
fn award_xp(
    conn: &mut PgConnection,
    actor_id: Uuid,
    user_id: Uuid,
    amount: i64,
) -> QueryResult<i64> {
    diesel::sql_query(
        "INSERT INTO world_actor_system_data \
           (actor_id, game_system_id, resource_data, created_by, updated_by) \
         VALUES ($1, $2, jsonb_build_object('xp', $3), $4, $4) \
         ON CONFLICT (actor_id) DO UPDATE SET \
           resource_data = jsonb_set( \
             coalesce(world_actor_system_data.resource_data, '{}'::jsonb), \
             '{xp}', \
             to_jsonb(coalesce((world_actor_system_data.resource_data->>'xp')::bigint, 0) + $3)), \
           updated_by = $4, \
           updated_at = now() \
         RETURNING (resource_data->>'xp')::bigint AS xp",
    )
    .bind::<SqlUuid, _>(actor_id)
    .bind::<Text, _>(SYSTEM_ID)
    .bind::<BigInt, _>(amount)
    .bind::<SqlUuid, _>(user_id)
    .get_result::<XpAfter>(conn)
    .map(|after| after.xp)
}

fn stored_xp(conn: &mut PgConnection, actor_id: Uuid) -> QueryResult<i64> {
    let resource_data = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(world_actor_system_data::resource_data)
        .first::<Option<serde_json::Value>>(conn)
        .optional()?
        .flatten();
    Ok(resource_data
        .as_ref()
        .and_then(|data| data.get("xp"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0))
}

/// What the roll needs that only the database knows.
struct Gathered {
    level: i64,
    judging: Judging,
}

fn gather(
    conn: &mut PgConnection,
    world_id: Uuid,
    actor_id: Uuid,
    skill_id: &str,
    typed_opposition: Option<f64>,
) -> Result<Gathered, String> {
    let trait_data = world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(world_actor_system_data::trait_data)
        .first::<Option<serde_json::Value>>(conn)
        .optional()
        .map_err(|_| "Failed to read this character's sheet".to_string())?
        .flatten();
    let settings = read_or_default(conn, world_id)
        .map_err(|_| "Failed to read this world's settings".to_string())?;
    let table = crate::table::read(conn, world_id)
        .map_err(|_| "Failed to read what has to be beaten".to_string())?;

    let handed: Vec<(String, i64)> = settings
        .starting_skills
        .iter()
        .map(|skill| (skill.name.clone(), skill.level))
        .collect();
    let level = skills_of(trait_data.as_ref(), &handed)
        .into_iter()
        .find(|skill| skill.id == skill_id)
        .map(|skill| skill.level)
        .ok_or_else(|| "This character has no such skill".to_string())?;

    Ok(Gathered {
        level,
        judging: Judging {
            // The setting decides whether statuses apply, never whether they
            // exist: a character may carry some from a world that had them.
            modifier: if settings.statuses_enabled {
                status_modifier(trait_data.as_ref())
            } else {
                0
            },
            // The Game Master's number when there is one, and then what the
            // player sent is not consulted at all.
            opposition: table
                .map(|difficulty| f64::from(difficulty.target))
                .or(typed_opposition.filter(|typed| typed.is_finite())),
            tie_succeeds: settings.tie_succeeds,
        },
    })
}

pub async fn roll_skill_impl<R: rand::Rng>(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    input: RollForShoesRollSkillInput,
    rng: &mut R,
) -> GraphQLResult<RollForShoesSkillRoll> {
    let RollForShoesRollSkillInput {
        world_id,
        actor_id,
        skill_id,
        opposition,
    } = input;

    // Every refusal before anything is gathered, and the roll last, in the
    // order `rollCheck` uses.
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> Result<(), Error> {
        let actor_world = world_actors::table
            .filter(world_actors::id.eq(actor_id))
            .select(world_actors::world_id)
            .first::<Uuid>(&mut conn)
            .map_err(|_| Error::new("Actor not found"))?;
        if actor_world != world_id {
            return Err(Error::new("That character is not in this world"));
        }
        Ok(())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))??;

    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;
    refuse_world_if_paused(state, world_id).await?;

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let gathered = tokio::task::spawn_blocking(move || {
        // After the pause, so a paused world answers with the pause whatever
        // it plays.
        let system = system_of_world(&mut conn, world_id).map_err(|_| "World not found")?;
        if system.as_deref() != Some(SYSTEM_ID) {
            return Err("This world is not playing Roll for Shoes".to_string());
        }
        gather(&mut conn, world_id, actor_id, &skill_id, opposition)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    let judging = gathered.judging;
    let context = judging.context();
    let (roll, (total, xp_awarded, xp)) = roll_and_settle(
        state,
        user_id,
        RollDiceInput {
            world_id,
            formula: format!("{}d6", gathered.level),
            bindings: None,
            visibility: None,
            label: None,
        },
        RollMeta::default(),
        rng,
        move |conn, resolution| {
            let dice = kept_dice(resolution);
            let facts = RollFacts {
                check: SKILL_CHECK,
                dice: &dice,
                total: roll_value(resolution),
            };
            let outcome = adjudicate(&facts, &context);
            let failed = outcome
                .as_ref()
                .is_some_and(|outcome| outcome.verdict == Verdict::Failure);
            // Failure is the only thing that pays, and it is paid here, in
            // the transaction that records the roll: there is no failed roll
            // without its experience and no experience without its roll.
            let (awarded, xp) = if failed {
                let xp = award_xp(conn, actor_id, user_id, FAILURE_XP)
                    .map_err(|_| "Failed to record the experience this roll earned".to_string())?;
                (FAILURE_XP, xp)
            } else {
                let xp = stored_xp(conn, actor_id)
                    .map_err(|_| "Failed to read this character's experience".to_string())?;
                (0, xp)
            };
            Ok((outcome, (modified_total(&facts, &context), awarded, xp)))
        },
    )
    .await?;

    if xp_awarded > 0 {
        // After the write, and best-effort, like every sheet announcement: a
        // lost one costs another open sheet a read, not the experience.
        if let Ok(mut conn) = state.db_pool.get() {
            let _ = tokio::task::spawn_blocking(move || {
                record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_ACTOR_SHEET_CHANGED,
                    Some(serde_json::json!({
                        "action": "changed",
                        "actorId": actor_id,
                        "dataType": "resource_data",
                    })),
                    user_id,
                )
            })
            .await;
        }
    }

    Ok(RollForShoesSkillRoll {
        roll,
        modifier: judging.modifier as i32,
        total,
        opposition: judging.opposition,
        xp_awarded: xp_awarded as i32,
        xp: xp as i32,
    })
}
