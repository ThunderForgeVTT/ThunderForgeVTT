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

use crate::auth::world_membership::require_world_member;
use crate::graphql::types::GraphQLRollResolution;
use crate::graphql::{app_state, authenticated_user};
use crate::models::NewRollRecord;
use crate::schema::world_roll_records;
use crate::state::AppState;
use thunderforge_dice::{DiceFormula, FormulaError, ResolutionKind};

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
    let mut conn = state.db_pool.get().map_err(|_| Error::new("Failed to get DB connection"))?;

    let world_id = input.world_id;
    tokio::task::spawn_blocking(move || require_world_member(&mut conn, user_id, world_id))
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|_| Error::new("You must be a member of this world to roll dice"))?;

    let formula = DiceFormula::parse(&input.formula).map_err(|e| Error::new(formula_error_message(&e)))?;

    let mut bindings = HashMap::new();
    for binding in input.bindings.into_iter().flatten() {
        bindings.insert(binding.name, binding.value);
    }

    let resolution =
        thunderforge_dice::resolve(&formula, &bindings, rng).map_err(|e| Error::new(formula_error_message(&e)))?;

    let (result_kind, result_value) = match resolution.kind {
        ResolutionKind::Total(v) => ("total", v),
        ResolutionKind::SuccessCount(n) => ("success_count", n as f64),
    };

    let detail = serde_json::to_value(&resolution).map_err(|_| Error::new("Failed to serialize roll detail"))?;
    let bindings_json = if bindings.is_empty() { None } else { serde_json::to_value(&bindings).ok() };

    let new_record = NewRollRecord {
        world_id: input.world_id,
        triggered_by: user_id,
        formula: resolution.formula.clone(),
        bindings: bindings_json,
        detail,
        result_kind: result_kind.to_string(),
        result_value,
    };

    let mut conn = state.db_pool.get().map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        diesel::insert_into(world_roll_records::table).values(&new_record).execute(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to record roll"))?;

    Ok(GraphQLRollResolution::from(&resolution))
}

#[derive(Default)]
pub struct RollMutation;

#[async_graphql::Object]
impl RollMutation {
    async fn roll_dice(&self, ctx: &Context<'_>, input: RollDiceInput) -> GraphQLResult<GraphQLRollResolution> {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    /// Deterministic RNG for tests — `rand_core` 0.10 dropped its old
    /// `mock::StepRng`, so this is a minimal always-increasing generator
    /// (never actually treated as authoritative; only `rollDice`'s real
    /// resolver method uses `rand::rng()`, research.md §3).
    struct StepRng(u64);

    impl StepRng {
        fn new(start: u64, _step: u64) -> Self {
            StepRng(start)
        }
    }

    impl rand::TryRng for StepRng {
        type Error = std::convert::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            self.0 = self.0.wrapping_add(1);
            Ok(self.0 as u32)
        }
        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(self.try_next_u32()? as u64)
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
            for b in dest.iter_mut() {
                *b = self.try_next_u32()? as u8;
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn non_member_is_rejected_before_any_roll_happens() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let outsider_id = insert_test_user(&mut conn);
        drop(conn);

        let mut rng = StepRng::new(0, 1);
        let result = roll_dice_impl(
            &state,
            outsider_id,
            RollDiceInput { world_id, formula: "1d20".to_string(), bindings: None },
            &mut rng,
        )
        .await;

        assert!(result.is_err());

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 0, "no roll should have been recorded for a rejected caller");
    }

    #[tokio::test]
    async fn member_can_roll_and_a_record_is_persisted() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let mut rng = StepRng::new(0, 1);
        let resolution = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput { world_id, formula: "1d20".to_string(), bindings: None },
            &mut rng,
        )
        .await
        .expect("a world member should be able to roll");

        assert_eq!(resolution.dice.len(), 1);

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn malformed_formula_produces_zero_rows() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let mut rng = StepRng::new(0, 1);
        let result = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput { world_id, formula: "1d20 +".to_string(), bindings: None },
            &mut rng,
        )
        .await;
        assert!(result.is_err());

        let mut conn = state.db_pool.get().unwrap();
        let count: i64 = world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn two_rolls_from_different_users_are_independent_records() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let other_id = insert_test_user(&mut conn);
        crate::test_support::insert_test_world_member(&mut conn, world_id, other_id, "Player");
        drop(conn);

        let mut rng_a = StepRng::new(0, 1);
        roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput { world_id, formula: "1d20".to_string(), bindings: None },
            &mut rng_a,
        )
        .await
        .unwrap();

        let mut rng_b = StepRng::new(0, 1);
        roll_dice_impl(
            &state,
            other_id,
            RollDiceInput { world_id, formula: "1d20".to_string(), bindings: None },
            &mut rng_b,
        )
        .await
        .unwrap();

        let mut conn = state.db_pool.get().unwrap();
        let records: Vec<crate::models::RollRecord> = world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .load(&mut conn)
            .unwrap();
        assert_eq!(records.len(), 2);
        assert_ne!(records[0].triggered_by, records[1].triggered_by);
    }

    /// US3 (T021): `RollDiceInput.bindings` correctly maps into
    /// `resolve()`'s `PlaceholderBindings`, and a missing placeholder
    /// surfaces as a specific, distinguishable error message rather than
    /// a generic failure.
    #[tokio::test]
    async fn placeholder_bindings_flow_through_and_missing_ones_are_specific_errors() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let mut rng = StepRng::new(0, 1);
        let low = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput {
                world_id,
                formula: "1d20 + STAT".to_string(),
                bindings: Some(vec![PlaceholderBindingInput { name: "STAT".to_string(), value: 3.0 }]),
            },
            &mut rng,
        )
        .await
        .unwrap();

        let mut rng = StepRng::new(0, 1);
        let high = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput {
                world_id,
                formula: "1d20 + STAT".to_string(),
                bindings: Some(vec![PlaceholderBindingInput { name: "STAT".to_string(), value: 8.0 }]),
            },
            &mut rng,
        )
        .await
        .unwrap();

        assert_eq!(high.result_value - low.result_value, 5.0);

        let mut rng = StepRng::new(0, 1);
        let err = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput { world_id, formula: "1d20 + STAT".to_string(), bindings: None },
            &mut rng,
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("STAT"), "error should name the missing placeholder: {}", err.message);
    }

    /// US3 (T022, SC-005): a spec 013 Item Effect-style formula (attack
    /// roll with a stat + flat modifiers placeholder) resolves through
    /// `rollDice` with no schema changes needed on either side.
    #[tokio::test]
    async fn spec_013_item_effect_formula_resolves_unchanged() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        // A "Longsword" damage effect's stored formula (spec 013).
        let mut rng = StepRng::new(0, 1);
        let damage = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput { world_id, formula: "2d8".to_string(), bindings: None },
            &mut rng,
        )
        .await
        .unwrap();
        assert_eq!(damage.dice.len(), 2);

        // An attack-roll effect's stored formula.
        let mut rng = StepRng::new(0, 1);
        let attack = roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput {
                world_id,
                formula: "1d20 + STAT + MODIFIERS".to_string(),
                bindings: Some(vec![
                    PlaceholderBindingInput { name: "STAT".to_string(), value: 3.0 },
                    PlaceholderBindingInput { name: "MODIFIERS".to_string(), value: 2.0 },
                ]),
            },
            &mut rng,
        )
        .await
        .unwrap();
        assert_eq!(attack.dice.len(), 1);
    }
}
