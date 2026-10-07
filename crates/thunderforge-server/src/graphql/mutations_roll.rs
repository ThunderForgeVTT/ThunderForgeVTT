//! Spec 014: `rollDice` — the sole way to produce an authoritative dice
//! result. See `specs/014-dice-rolling-engine/contracts/graphql-roll.md`.
//!
//! World-membership is verified BEFORE resolving (never after), a real
//! OS-backed RNG is constructed fresh per call, and `RollDiceInput` has
//! no field that could express a pre-computed result — client-supplied
//! outcomes are structurally impossible, not just policy-rejected
//! (FR-001/FR-002).

use std::collections::HashMap;

use async_graphql::{Context, Error, InputObject, Result as GraphQLResult};
use diesel::prelude::*;
use rand::SeedableRng;
use uuid::Uuid;

use crate::auth::world_membership::{actor_in_world, is_dm_of_world, require_world_member};
use crate::graphql::queries::roll::{usernames, viewer_in_world};
use crate::graphql::types::{GraphQLRollResolution, RollVisibility, WorldRoll};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{NewRollRecord, RollRecord};
use crate::play_pause::gate::refuse_if_paused;
use crate::rolls::visibility::{Visibility, may_roll};
use crate::schema::world_roll_records;
use crate::state::AppState;
use crate::world_events::{
    EVENT_CODE_ROLL_MADE, EVENT_CODE_ROLL_REVEALED, record_world_event, roll_event_payload,
};
use thunderforge_canvas_core::system_contribution::RollOutcome;
use thunderforge_dice::{DiceFormula, FormulaError, ResolutionKind, RollResolution};

#[derive(InputObject, Debug, Clone)]
pub struct PlaceholderBindingInput {
    pub name: String,
    pub value: f64,
}

#[derive(InputObject, Debug, Clone)]
pub struct RollDiceInput {
    pub world_id: Uuid,
    pub formula: String,
    pub bindings: Option<Vec<PlaceholderBindingInput>>,
    /// Spec 081: who sees it. `EVERYONE` when not given; `GM_EYES` is a
    /// player's, `GM_ONLY` the GM's (FR-007).
    pub visibility: Option<RollVisibility>,
    /// Spec 081: what it was for, at most 80 characters.
    pub label: Option<String>,
}

/// Spec 081: the longest label a roll keeps.
pub const MAX_ROLL_LABEL: usize = 80;

/// A label as stored: trimmed, empty as none, refused when too long.
fn roll_label(label: Option<String>) -> GraphQLResult<Option<String>> {
    let Some(label) = label
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
    else {
        return Ok(None);
    };
    if label.chars().count() > MAX_ROLL_LABEL {
        return Err(Error::new("A roll's label is at most 80 characters"));
    }
    Ok(Some(label))
}

fn formula_error_message(err: &FormulaError) -> String {
    format!("Roll rejected: {err}")
}

/// Testable core of `RollMutation::roll_dice`. Verifies world membership,
/// resolves server-side with `rng` (the real OS-backed RNG in production;
/// a scripted RNG in tests), and — only on success — persists a
/// `world_roll_records` row (data-model.md). A failed resolution never
/// rolls a die or writes a row (FR-011).
pub async fn roll_dice_impl<R: rand::Rng>(
    state: &AppState,
    user_id: Uuid,
    input: RollDiceInput,
    rng: &mut R,
) -> GraphQLResult<GraphQLRollResolution> {
    roll_and_settle(state, user_id, input, rng, |_, _| Ok((None, ())))
        .await
        .map(|(resolution, ())| resolution)
}

/// Why a settled roll left nothing behind.
enum Unsettled {
    /// The settle step refused, in words for the caller.
    Refused(String),
    Database,
}

impl From<diesel::result::Error> for Unsettled {
    fn from(_: diesel::result::Error) -> Self {
        Unsettled::Database
    }
}

/// `roll_dice_impl`, with what follows from the roll decided in the same
/// transaction as its record (spec 067 FR-032).
///
/// `settle` sees the resolved roll and answers with how it came out — what a
/// system's adjudicator returned, or `None` — and anything of its own the
/// caller wants back. It runs on the connection the record is written on, so
/// a consequence it writes (experience for a failed roll) and the record that
/// justifies it both land or neither does. If it refuses, no row is written
/// and the refusal is the caller's error.
///
/// The dice are still rolled here and nowhere else: `settle` is handed the
/// result and has no way to change it.
pub async fn roll_and_settle<R, T, F>(
    state: &AppState,
    user_id: Uuid,
    input: RollDiceInput,
    rng: &mut R,
    settle: F,
) -> GraphQLResult<(GraphQLRollResolution, T)>
where
    R: rand::Rng,
    T: Send + 'static,
    F: FnOnce(&mut PgConnection, &RollResolution) -> Result<(Option<RollOutcome>, T), String>
        + Send
        + 'static,
{
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = input.world_id;
    let visibility: Visibility = input.visibility.unwrap_or(RollVisibility::Everyone).into();
    let label = roll_label(input.label)?;
    tokio::task::spawn_blocking(move || -> GraphQLResult<()> {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| Error::new("You must be a member of this world to roll dice"))?;
        refuse_if_paused(&mut conn, world_id)?;
        // FR-007: an admin who does not run the world rolls as a player.
        let is_gm = actor_in_world(&mut conn, user_id, false, world_id).runs_the_world();
        may_roll(visibility, is_gm).map_err(Error::new)?;
        Ok(())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))??;

    let formula =
        DiceFormula::parse(&input.formula).map_err(|e| Error::new(formula_error_message(&e)))?;

    let mut bindings = HashMap::new();
    for binding in input.bindings.into_iter().flatten() {
        bindings.insert(binding.name, binding.value);
    }

    let resolution = thunderforge_dice::resolve(&formula, &bindings, rng)
        .map_err(|e| Error::new(formula_error_message(&e)))?;

    let (result_kind, result_value) = match resolution.kind {
        ResolutionKind::Total(v) => ("total", v),
        ResolutionKind::SuccessCount(n) => ("success_count", n as f64),
    };

    let detail = serde_json::to_value(&resolution)
        .map_err(|_| Error::new("Failed to serialize roll detail"))?;
    let bindings_json = if bindings.is_empty() {
        None
    } else {
        serde_json::to_value(&bindings).ok()
    };

    let mut new_record = NewRollRecord {
        world_id: input.world_id,
        triggered_by: user_id,
        formula: resolution.formula.clone(),
        bindings: bindings_json,
        detail,
        result_kind: result_kind.to_string(),
        result_value,
        outcome: None,
        visibility: visibility.as_str().to_string(),
        label,
    };

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (resolution, outcome, settled) = tokio::task::spawn_blocking(move || {
        conn.transaction::<_, Unsettled, _>(|conn| {
            let (outcome, settled) = settle(conn, &resolution).map_err(Unsettled::Refused)?;
            new_record.outcome = outcome
                .as_ref()
                .and_then(|outcome| serde_json::to_value(outcome).ok());
            let roll_id = diesel::insert_into(world_roll_records::table)
                .values(&new_record)
                .returning(world_roll_records::id)
                .get_result::<Uuid>(conn)?;
            // Spec 081 FR-001: the table hears of it in the same transaction,
            // by id and visibility only (FR-002).
            record_world_event(
                conn,
                world_id,
                EVENT_CODE_ROLL_MADE,
                Some(roll_event_payload(roll_id, visibility)),
                user_id,
            )
            .map_err(|_| Unsettled::Database)?;
            Ok((resolution, outcome, settled))
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|unsettled| match unsettled {
        Unsettled::Refused(why) => Error::new(why),
        Unsettled::Database => Error::new("Failed to record roll"),
    })?;

    Ok((
        GraphQLRollResolution {
            outcome: outcome.as_ref().map(Into::into),
            ..GraphQLRollResolution::from(&resolution)
        },
        settled,
    ))
}

/// A resolved roll as an adjudicator is shown it: the dice that counted, in
/// the order rolled, and what the formula came to.
pub fn kept_dice(resolution: &RollResolution) -> Vec<i64> {
    resolution
        .dice
        .iter()
        .filter(|die| die.kept)
        .map(|die| die.final_value)
        .collect()
}

/// What the formula came to, a total or a success count alike.
pub fn roll_value(resolution: &RollResolution) -> f64 {
    match resolution.kind {
        ResolutionKind::Total(value) => value,
        ResolutionKind::SuccessCount(count) => count as f64,
    }
}

/// Spec 081: `revealRoll`. The GM (or an admin) shows a hidden roll to the
/// table: who and when are recorded, the dice are not touched (FR-011), and
/// every member is told to fetch it again. A roll already public answers as
/// it is and records nothing (FR-010).
pub async fn reveal_roll_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    roll_id: Uuid,
) -> GraphQLResult<WorldRoll> {
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the GM can reveal a roll"));
    }
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || -> GraphQLResult<WorldRoll> {
        refuse_if_paused(&mut conn, world_id)?;
        let row = conn.transaction::<_, Error, _>(|conn| {
            let row = world_roll_records::table
                .filter(world_roll_records::id.eq(roll_id))
                .filter(world_roll_records::world_id.eq(world_id))
                .select(RollRecord::as_select())
                .for_update()
                .first::<RollRecord>(conn)
                .optional()
                .map_err(|_| Error::new("Failed to load the roll"))?
                .ok_or_else(|| Error::new("Roll not found"))?;
            let visibility = Visibility::parse(&row.visibility);
            if visibility == Visibility::Everyone || row.revealed_at.is_some() {
                return Ok(row);
            }
            let row = diesel::update(world_roll_records::table.find(roll_id))
                .set((
                    world_roll_records::revealed_at.eq(chrono::Utc::now()),
                    world_roll_records::revealed_by.eq(user_id),
                ))
                .returning(RollRecord::as_returning())
                .get_result::<RollRecord>(conn)
                .map_err(|_| Error::new("Failed to reveal the roll"))?;
            record_world_event(
                conn,
                world_id,
                EVENT_CODE_ROLL_REVEALED,
                Some(roll_event_payload(roll_id, visibility)),
                user_id,
            )?;
            Ok(row)
        })?;
        // The revealer may see it whole; asked anyway, so a reveal answers
        // through the same rule as every other read.
        viewer_in_world(&mut conn, user_id, is_admin, world_id)?;
        let mut ids = vec![row.triggered_by];
        ids.extend(row.revealed_by);
        let names = usernames(&mut conn, &ids).map_err(|_| Error::new("Failed to load names"))?;
        let roller = names.get(&row.triggered_by).cloned().unwrap_or_default();
        let revealer = row.revealed_by.and_then(|id| names.get(&id).cloned());
        Ok(WorldRoll::from_row(row, roller, revealer))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[derive(Default)]
pub struct RollMutation;

#[async_graphql::Object]
impl RollMutation {
    async fn roll_dice(
        &self,
        ctx: &Context<'_>,
        input: RollDiceInput,
    ) -> GraphQLResult<GraphQLRollResolution> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        // `StdRng`, freshly seeded from the OS-entropy-backed thread RNG
        // — the one and only place in the whole system a "real" roll is
        // produced (research.md §3). `ThreadRng` itself isn't `Send`
        // (thread-local `Rc`), so it can't be held across this async
        // resolver's `.await`; `StdRng` is a self-contained CSPRNG with
        // no such restriction.
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        roll_dice_impl(state, auth_user.user_id, input, &mut rng).await
    }

    /// Spec 081: show a hidden roll to the table. GM or admin; idempotent.
    async fn reveal_roll(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        roll_id: Uuid,
    ) -> GraphQLResult<WorldRoll> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        reveal_roll_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            roll_id,
        )
        .await
    }
}

#[cfg(test)]
#[path = "mutations_roll_tests.rs"]
mod tests;
