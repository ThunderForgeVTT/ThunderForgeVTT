//! Spec 036 US3b: `rollCheck` — a check rolled from a character sheet is the
//! *system's* check, resolved here.
//!
//! See `specs/036-concurrent-client-sessions/contracts/system-checks.md`.
//!
//! # What this module is, in one sentence
//!
//! A lookup and a substitution in front of `rollDice`.
//!
//! It resolves nothing about dice, decides nothing about outcomes, and owns no
//! randomness. It takes the check a system declared, replaces the formula's
//! placeholders with numbers read off the actor, and hands the finished
//! formula to [`crate::graphql::mutations_roll::roll_dice_impl`] — the one
//! path ADR-044 permits to produce a result. That is what makes FR-036 true by
//! construction rather than by comparison: a check rolled from a sheet is not
//! *like* a roll made at the table, it is the same call with the same record.
//!
//! # Why the input has no formula field
//!
//! `RollDiceInput` was written so a client-supplied *outcome* is structurally
//! impossible (FR-001/FR-002 of spec 014). The same argument applies one level
//! up: a client that can name a formula is a client that has decided what the
//! check is, and the sheet is not entitled to that. So this mutation takes
//! three arguments — a world, an actor and a check id — and there is no
//! argument by which a caller could smuggle in dice, a value, or a verdict.
//! An unknown `checkId` is refused; it is never handed to the parser.
//!
//! # Why the permission check is the ordinary one
//!
//! A sheet window is a surface, not a capability. Rolling a check against an
//! actor's numbers is an action on that actor and requires exactly what any
//! other action on it requires, through the same
//! `auth::actor_permissions::require_actor_permission`. Nothing here has its
//! own idea of who may do what.

use async_graphql::{Context, Error, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use rand::SeedableRng;
use uuid::Uuid;

use crate::auth::actor_permissions::require_actor_permission;
use crate::auth::world_membership::require_world_member;
use crate::declared_values::{ActorSlots, declared_values_for_actor};
use crate::graphql::mutations_roll::{PlaceholderBindingInput, RollDiceInput, roll_dice_impl};
use crate::graphql::types::{ActorPermissionLevel, GraphQLRollResolution};
use crate::graphql::{app_state, authenticated_user};
use crate::schema::{world_actor_system_data, world_actors, worlds};
use crate::state::AppState;
use thunderforge_canvas_core::system_rules::{
    CheckBinding, CheckDeclaration, DeclaredValues, checks_from_manifest,
};

/// What a sheet is told about a check: enough to draw a button, and no more.
///
/// Deliberately not the formula and not the bindings. Those are in the pack's
/// manifest and no secret, but publishing them *through this API* would invite
/// a client to compute the roll it is about to ask for, and then to notice
/// that it need not ask.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLSystemCheck {
    /// The id to pass back to `rollCheck`.
    pub id: String,
    /// What a person is shown.
    pub label: String,
    /// The set it belongs to — a sheet groups by this and does not read it.
    pub group: Option<String>,
}

impl From<&CheckDeclaration> for GraphQLSystemCheck {
    fn from(check: &CheckDeclaration) -> Self {
        Self {
            id: check.id.clone(),
            label: check.label.clone(),
            group: check.group.clone(),
        }
    }
}

/// The checks a system declares, read from its own pack.
///
/// An absent or unreadable manifest yields none, the same answer
/// `attribute_declarations_for_system` gives and for the same reason: a system
/// that declares no check has none, and there is nothing sensible to invent.
pub fn checks_for_system(systems_dir: &str, system_id: &str) -> Vec<CheckDeclaration> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    checks_from_manifest(&manifest)
}

/// The numbers a check's formula needs, read off one actor.
///
/// Split out from the database work so the substitution itself is testable
/// without one — the property this whole feature rests on is "the server chose
/// these numbers", and that is worth asserting directly.
///
/// # Why a missing value refuses instead of defaulting
///
/// Because zero is a statement. `DeclaredValues::integer` already draws this
/// line for the same reason: a character with no Strength recorded has not got
/// a Strength of nought, and rolling `1d20 + 0` on their behalf would report a
/// number the sheet never claimed. The refusal names the identifier so a Game
/// Master can see which square of the sheet is empty.
pub fn bindings_for_check(
    check: &CheckDeclaration,
    values: &DeclaredValues,
) -> Result<Vec<PlaceholderBindingInput>, String> {
    let mut out = Vec::new();
    for (placeholder, binding) in &check.bindings {
        let CheckBinding::Value { id } = binding;
        let value = values
            .get(id)
            .ok_or_else(|| format!("This character has no value for '{id}' to check against"))?;
        let number = numeric(&value.value).ok_or_else(|| {
            format!("'{id}' is not a number this character can be checked against")
        })?;
        out.push(PlaceholderBindingInput {
            name: placeholder.clone(),
            value: number,
        });
    }
    Ok(out)
}

/// A declared value as a number, when it is one.
///
/// Leans on `as_integer` — which already knows a pool's number is its current
/// value and a state set has none — and then accepts a genuinely fractional
/// number, which `as_integer` refuses because rounding a score would invent
/// precision. A formula is arithmetic and has no such problem.
fn numeric(value: &thunderforge_canvas_core::system_rules::DeclaredValueKind) -> Option<f64> {
    use thunderforge_canvas_core::system_rules::DeclaredValueKind;
    match value {
        DeclaredValueKind::Number(n) => Some(*n),
        other => other.as_integer().map(f64::from),
    }
}

/// One actor's stored slots, and the world it is in.
fn actor_world(conn: &mut PgConnection, actor_id: Uuid) -> Result<Uuid, Error> {
    world_actors::table
        .filter(world_actors::id.eq(actor_id))
        .select(world_actors::world_id)
        .first::<Uuid>(conn)
        .map_err(|_| Error::new("Actor not found"))
}

fn actor_slots(conn: &mut PgConnection, actor_id: Uuid) -> ActorSlots {
    type SlotRow = (
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
        Option<serde_json::Value>,
    );
    world_actor_system_data::table
        .filter(world_actor_system_data::actor_id.eq(actor_id))
        .select((
            world_actor_system_data::ability_data,
            world_actor_system_data::resource_data,
            world_actor_system_data::proficiency_data,
            world_actor_system_data::trait_data,
        ))
        .first::<SlotRow>(conn)
        .map(
            |(ability_data, resource_data, proficiency_data, trait_data)| ActorSlots {
                ability_data,
                resource_data,
                proficiency_data,
                trait_data,
            },
        )
        .unwrap_or_default()
}

/// The system a world is being played with, if it has one.
fn world_system(conn: &mut PgConnection, world_id: Uuid) -> Result<String, Error> {
    worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::game_system_id)
        .first::<Option<String>>(conn)
        .map_err(|_| Error::new("World not found"))?
        .ok_or_else(|| Error::new("This world has no game system, so it declares no checks"))
}

/// Testable core of `RollCheckMutation::roll_check`.
///
/// The order is the point, and it is the same order `roll_dice_impl` uses:
/// every refusal happens before anything is looked up, and the roll happens
/// last. An unknown check id, an actor in another world and a caller without
/// permission all return before a die exists — and, because the roll is
/// produced by `roll_dice_impl`, a refusal here leaves no `world_roll_records`
/// row for the same reason a malformed formula does.
pub async fn roll_check_impl<R: rand::Rng>(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    actor_id: Uuid,
    check_id: String,
    rng: &mut R,
) -> GraphQLResult<GraphQLRollResolution> {
    // The actor must be in the world the caller named. Without this an actor
    // id from another table could be rolled "into" a world the caller happens
    // to belong to, and the record would name the wrong table.
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let (actor_world_id, system_id, slots) =
        tokio::task::spawn_blocking(move || -> Result<(Uuid, String, ActorSlots), Error> {
            let actor_world_id = actor_world(&mut conn, actor_id)?;
            let system_id = world_system(&mut conn, world_id)?;
            let slots = actor_slots(&mut conn, actor_id);
            Ok((actor_world_id, system_id, slots))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))??;

    if actor_world_id != world_id {
        return Err(Error::new("That character is not in this world"));
    }

    // A sheet is not a permission bypass: the same requirement as any other
    // action on this actor, through the same function.
    require_actor_permission(
        state,
        user_id,
        is_admin,
        actor_id,
        ActorPermissionLevel::Editor,
    )
    .await?;

    // The id is matched against what the system declared. It is never parsed,
    // never interpolated and never reaches the dice crate — a caller who
    // guesses wrong gets a refusal, not an evaluation.
    let checks = checks_for_system(&state.directories.systems_dir, &system_id);
    let check = checks
        .iter()
        .find(|check| check.id == check_id)
        .ok_or_else(|| Error::new("This system declares no such check"))?;

    let values = DeclaredValues::new(declared_values_for_actor(
        &state.directories.systems_dir,
        &system_id,
        &slots,
    ));
    let bindings = bindings_for_check(check, &values).map_err(Error::new)?;

    // And now the ordinary path, with a formula the client never saw.
    roll_dice_impl(
        state,
        user_id,
        RollDiceInput {
            world_id,
            formula: check.formula.clone(),
            bindings: Some(bindings),
        },
        rng,
    )
    .await
}

#[derive(Default)]
pub struct RollCheckQuery;

#[async_graphql::Object]
impl RollCheckQuery {
    /// The checks this world's system declares, for a sheet to offer.
    ///
    /// Empty for a system that declares none (FR-037), which is a real answer
    /// and not a failure: seven of the eight bundled packs are that case, and
    /// their sheets show no check button rather than an invented one.
    async fn system_checks(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLSystemCheck>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let user_id = auth_user.user_id;
        let system_id = tokio::task::spawn_blocking(move || -> Result<Option<String>, Error> {
            require_world_member(&mut conn, user_id, world_id)
                .map_err(|_| Error::new("You must be a member of this world"))?;
            worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first::<Option<String>>(&mut conn)
                .map_err(|_| Error::new("World not found"))
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))??;

        let Some(system_id) = system_id else {
            return Ok(Vec::new());
        };

        Ok(
            checks_for_system(&state.directories.systems_dir, &system_id)
                .iter()
                .map(GraphQLSystemCheck::from)
                .collect(),
        )
    }
}

#[derive(Default)]
pub struct RollCheckMutation;

#[async_graphql::Object]
impl RollCheckMutation {
    /// Roll a check the world's system declares, against one actor's values.
    ///
    /// Resolved server-side and recorded exactly as `rollDice` is. There is
    /// deliberately no formula argument, no value argument and no outcome
    /// argument: the dice, the numbers and the result are the system's and the
    /// server's, never the sheet's (FR-035).
    async fn roll_check(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        actor_id: Uuid,
        check_id: String,
    ) -> GraphQLResult<GraphQLRollResolution> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        // Same construction as `rollDice`, and for the same reason — see the
        // comment there. This module owns no randomness of its own.
        let mut rng = rand::rngs::StdRng::from_rng(&mut rand::rng());
        roll_check_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            actor_id,
            check_id,
            &mut rng,
        )
        .await
    }
}

#[cfg(test)]
#[path = "mutations_roll_check_tests.rs"]
mod tests;
