//! Spec 084 US3: `rerollRoll` — spend something on a sheet to roll one of
//! your own d20 tests again (contracts/graphql-rolls-facets.md).
//!
//! The host refuses in the contract's order ([`may_reroll`]); the pack says
//! whether the sheet can pay and what the reroll changes. Then one
//! transaction locks the roll and the sheet, writes the sheet the pack
//! returned, replays the dice with only that change, records the new roll
//! beside the old one and judges it. Either all of it lands or none of it
//! does, and the unique index on `reroll_of` keeps a roll to one
//! replacement however many ask at once.

use async_graphql::{Context, Error, Result as GraphQLResult};
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use rand::SeedableRng;
use uuid::Uuid;

use crate::combat::attack::FightRefusal;
use crate::combat::attack_reroll::reroll_attack;
use crate::graphql::queries::roll::{RollContext, usernames, viewer_in_world};
use crate::graphql::types::WorldRoll;
use crate::graphql::{app_state, authenticated_user};
use crate::models::{NewRollRecord, RollRecord};
use crate::play_pause::gate::refuse_if_paused;
use crate::rolls::facets::{RollMeta, facets_for};
use crate::rolls::reroll::{
    ALREADY_REROLLED, NOT_A_D20_TEST, ONLY_MAKER, Standing, attack_hit, chain_of, load_sheet,
    may_act, may_reroll, reroll_input, rerolled_by, spend_label, spent_in,
};
use crate::rolls::visibility::Visibility;
use crate::schema::{world_actor_system_data, world_roll_records};
use crate::state::AppState;
use crate::world_events::{
    EVENT_CODE_ACTOR_SHEET_CHANGED, EVENT_CODE_ROLL_MADE, record_world_event, roll_event_payload,
};
use crate::world_system_settings::{self, system_of_world};
use thunderforge_canvas_core::roll_facets::{RerollEdit, RollKind};
use thunderforge_canvas_core::system_contribution::contribution_for;
use thunderforge_dice::{PlaceholderBindings, ResolutionKind, RollResolution};
use thunderforge_dice::{Recorded, ReplayEdit, lowest_die, replay};

/// What to reroll, and with what.
#[derive(Debug, Clone)]
pub struct RerollRequest {
    pub world_id: Uuid,
    pub roll_id: Uuid,
    pub spend: String,
}

/// Why nothing was written.
enum Refused {
    /// In words for the caller.
    Said(String),
    Database,
}

impl From<DieselError> for Refused {
    fn from(error: DieselError) -> Self {
        match error {
            // Someone else's reroll of the same roll landed first.
            DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _) => {
                Refused::Said(ALREADY_REROLLED.to_string())
            }
            _ => Refused::Database,
        }
    }
}

impl From<Refused> for Error {
    fn from(refused: Refused) -> Self {
        match refused {
            Refused::Said(sentence) => Error::new(sentence),
            Refused::Database => Error::new("Failed to reroll the roll"),
        }
    }
}

fn said(sentence: impl Into<String>) -> Refused {
    Refused::Said(sentence.into())
}

/// Testable core of `RerollMutation::reroll_roll`. `now` is the clock the
/// window is judged by.
pub async fn reroll_roll_impl<R: rand::Rng>(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    request: RerollRequest,
    rng: &mut R,
    now: DateTime<Utc>,
) -> GraphQLResult<WorldRoll> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let systems_dir = state.directories.systems_dir.clone();
    // Drawn from here, so the blocking task holds an RNG of its own.
    let mut rng = rand::rngs::StdRng::from_rng(rng);
    tokio::task::spawn_blocking(move || -> GraphQLResult<WorldRoll> {
        let RerollRequest {
            world_id,
            roll_id,
            spend,
        } = request;
        let viewer = viewer_in_world(&mut conn, user_id, is_admin, world_id)?;
        refuse_if_paused(&mut conn, world_id)?;
        let system_id = system_of_world(&mut conn, world_id)
            .map_err(|_| Error::new("World not found"))?
            .unwrap_or_default();
        let row = conn.transaction::<_, Refused, _>(|conn| {
            let row = world_roll_records::table
                .filter(world_roll_records::id.eq(roll_id))
                .filter(world_roll_records::world_id.eq(world_id))
                .select(RollRecord::as_select())
                .for_update()
                .first::<RollRecord>(conn)
                .optional()?
                .ok_or_else(|| said(ONLY_MAKER))?;
            let acts = match row.actor_id {
                Some(actor_id) => may_act(conn, user_id, is_admin, world_id, actor_id)?,
                None => false,
            };
            let chain = chain_of(conn, &row)?;
            let spent = spent_in(&chain);
            let standing = Standing {
                row: &row,
                rerolled_by: rerolled_by(conn, row.id)?,
                may_act: acts,
                spent: &spent,
                hit: row.roll_kind.as_deref() == Some("to_hit") && attack_hit(conn, row.id)?,
            };
            let label = spend_label(&system_id, &spend);
            let kind = may_reroll(user_id, &standing, &spend, &label, now).map_err(said)?;
            let actor_id = row.actor_id.ok_or_else(|| said(ONLY_MAKER))?;
            reroll_d20(
                conn,
                &systems_dir,
                &system_id,
                user_id,
                &row,
                actor_id,
                &spend,
                kind,
                &mut rng,
            )
        })?;
        let mut ids = vec![row.triggered_by];
        ids.extend(row.revealed_by);
        let names = usernames(&mut conn, &ids).map_err(|_| Error::new("Failed to load names"))?;
        let mut context = RollContext::load(&mut conn, world_id, &[row.id])
            .map_err(|_| Error::new("Failed to load the roll"))?;
        context
            .offer(&mut conn, &systems_dir, viewer, std::slice::from_ref(&row))
            .map_err(|_| Error::new("Failed to load the roll"))?;
        let roller = names.get(&row.triggered_by).cloned().unwrap_or_default();
        let links = context.links(row.id);
        Ok(WorldRoll::from_row(row, roller, None, links))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Everything after the host's refusals, on the transaction's connection:
/// the pack's plan, the sheet it pays from, the replayed dice, the record
/// and the events. A to-hit then re-judges its attack (research R7).
#[allow(clippy::too_many_arguments)]
fn reroll_d20(
    conn: &mut PgConnection,
    systems_dir: &str,
    system_id: &str,
    user_id: Uuid,
    row: &RollRecord,
    actor_id: Uuid,
    spend: &str,
    kind: RollKind,
    rng: &mut rand::rngs::StdRng,
) -> Result<RollRecord, Refused> {
    let world_id = row.world_id;
    // The sheet is locked after the roll, always in that order.
    world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select(world_actor_system_data::id)
        .for_update()
        .first::<Uuid>(conn)
        .optional()?;
    let sheet = load_sheet(conn, systems_dir, world_id, system_id, actor_id)?;
    let system = facets_for(system_id)
        .ok_or_else(|| said(format!("This system has no reroll called \"{spend}\".")))?;
    let facets = row.facet_ids();
    let plan = (system.reroll)(&reroll_input(spend, kind, row, &facets, &sheet)).map_err(said)?;

    crate::systems::validate_actor_system_data(system_id, "trait_data", &plan.trait_data)
        .map_err(said)?;
    diesel::update(
        world_actor_system_data::table.filter(world_actor_system_data::actor_id.eq(actor_id)),
    )
    .set((
        world_actor_system_data::trait_data.eq(Some(plan.trait_data.clone())),
        world_actor_system_data::updated_by.eq(user_id),
        world_actor_system_data::updated_at.eq(Utc::now().naive_utc()),
    ))
    .execute(conn)?;
    record_world_event(
        conn,
        world_id,
        EVENT_CODE_ACTOR_SHEET_CHANGED,
        Some(serde_json::json!({
            "action": "changed",
            "actorId": actor_id,
            "dataType": "trait_data",
        })),
        user_id,
    )
    .map_err(|_| Refused::Database)?;

    let replayed = replay_with(row, &plan.edit, rng)?;
    let outcome = judge(conn, systems_dir, system_id, row, &replayed)?;
    let (result_kind, result_value) = match replayed.kind {
        ResolutionKind::Total(value) => ("total", value),
        ResolutionKind::SuccessCount(count) => ("success_count", count as f64),
    };
    let detail =
        serde_json::to_value(&replayed).map_err(|_| said("Failed to serialize roll detail"))?;
    let mut record = NewRollRecord {
        bindings: row.bindings.clone(),
        visibility: row.visibility.clone(),
        label: row.label.clone(),
        outcome,
        ..NewRollRecord::plain(
            world_id,
            user_id,
            replayed.formula.clone(),
            detail,
            result_kind,
            result_value,
        )
    };
    let mut with_spend = facets;
    with_spend.push(spend.to_string());
    RollMeta {
        actor_id: Some(actor_id),
        roll_kind: Some(kind),
        check_id: row.check_id.clone(),
        facets: with_spend,
        reroll_of: Some(row.id),
        reroll_spent: Some(spend.to_string()),
    }
    .write_to(&mut record);
    let new_row = diesel::insert_into(world_roll_records::table)
        .values(&record)
        .returning(RollRecord::as_returning())
        .get_result::<RollRecord>(conn)?;
    record_world_event(
        conn,
        world_id,
        EVENT_CODE_ROLL_MADE,
        Some(roll_event_payload(
            new_row.id,
            Visibility::parse(&new_row.visibility),
        )),
        user_id,
    )
    .map_err(|_| Refused::Database)?;
    if kind == RollKind::ToHit {
        let attack = reroll_attack(conn, systems_dir, user_id, row.id, &new_row, rng).map_err(
            |refusal| match refusal {
                FightRefusal::Failed(_) => Refused::Database,
                other => said(other.message()),
            },
        )?;
        tracing::info!(
            attack_id = %attack.attack_id,
            outcome = %attack.outcome,
            offer_id = ?attack.offer_id,
            "an attack was rerolled"
        );
    }
    Ok(new_row)
}

/// The recorded roll again, with only the pack's change.
fn replay_with(
    row: &RollRecord,
    edit: &RerollEdit,
    rng: &mut rand::rngs::StdRng,
) -> Result<RollResolution, Refused> {
    let unreadable = || said("This roll can no longer be replayed.");
    let resolution: RollResolution =
        serde_json::from_value(row.detail.clone()).map_err(|_| unreadable())?;
    let bindings: PlaceholderBindings = match &row.bindings {
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| unreadable())?,
        None => PlaceholderBindings::default(),
    };
    let recorded = Recorded {
        formula: &row.formula,
        bindings: &bindings,
        resolution: &resolution,
    };
    match edit {
        RerollEdit::RerollLowest { sides } => {
            let die = lowest_die(&resolution, *sides).ok_or_else(|| said(NOT_A_D20_TEST))?;
            replay(recorded, None, ReplayEdit::RerollDie(die), rng)
        }
        RerollEdit::Reshape { formula } => replay(recorded, Some(formula), ReplayEdit::None, rng),
    }
    .map_err(|_| unreadable())
}

/// The check judged again by its system's adjudicator, if it has one.
fn judge(
    conn: &mut PgConnection,
    systems_dir: &str,
    system_id: &str,
    row: &RollRecord,
    replayed: &RollResolution,
) -> Result<Option<serde_json::Value>, Refused> {
    let (Some(adjudicate), Some(check_id)) = (
        contribution_for(system_id).and_then(|pack| pack.adjudicate),
        row.check_id.as_deref(),
    ) else {
        return Ok(None);
    };
    let declarations = world_system_settings::declarations_for_system(systems_dir, system_id);
    let outcome = crate::graphql::mutations_roll_check::judge_check(
        conn,
        adjudicate,
        row.world_id,
        system_id,
        declarations,
        check_id,
        replayed,
    )
    .map_err(said)?;
    Ok(outcome.and_then(|outcome| serde_json::to_value(outcome).ok()))
}

#[derive(Default)]
pub struct RerollMutation;

#[async_graphql::Object]
impl RerollMutation {
    /// Spec 084: spend a resource to reroll one of your own d20 tests.
    /// Gated by a pause.
    async fn reroll_roll(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        roll_id: Uuid,
        spend: String,
    ) -> GraphQLResult<WorldRoll> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        // A fresh CSPRNG per call, as `rollDice` draws (research.md §3).
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        reroll_roll_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            RerollRequest {
                world_id,
                roll_id,
                spend,
            },
            &mut rng,
            Utc::now(),
        )
        .await
    }
}

#[cfg(test)]
#[path = "mutations_reroll_tests.rs"]
mod tests;
