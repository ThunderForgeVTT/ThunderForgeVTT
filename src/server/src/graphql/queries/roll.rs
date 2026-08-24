//! Spec 014: `worldRollRecords` (DM-only roll history, FR-014) and
//! `validateDiceFormula` (pure parse-only check, any caller). See
//! `specs/014-dice-rolling-engine/contracts/graphql-roll.md`.

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::actor_permissions::is_dm_of_world;
use crate::graphql::types::GraphQLRollRecord;
use crate::graphql::{app_state, authenticated_user};
use crate::models::RollRecord;
use crate::schema::world_roll_records;
use crate::state::AppState;
use thunderforge_dice::DiceFormula;

const DEFAULT_ROLL_RECORD_LIMIT: i64 = 50;

/// Testable core of `RollQuery::world_roll_records`. DM-only (this
/// contract's stated floor — contracts/graphql-roll.md), newest first.
pub async fn world_roll_records_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    limit: Option<i32>,
) -> GraphQLResult<Vec<GraphQLRollRecord>> {
    if !is_dm_of_world(state, user_id, is_admin, world_id).await? {
        return Err(Error::new("Only the DM (Owner or GM) may view roll history"));
    }

    let take = limit.map(|n| n as i64).unwrap_or(DEFAULT_ROLL_RECORD_LIMIT).clamp(1, 500);

    let mut conn = state.db_pool.get().map_err(|_| Error::new("Failed to get DB connection"))?;
    let records: Vec<RollRecord> = tokio::task::spawn_blocking(move || {
        world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .order(world_roll_records::created_at.desc())
            .limit(take)
            .load::<RollRecord>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load roll history"))?;

    Ok(records.into_iter().map(GraphQLRollRecord::from).collect())
}

/// Pure parse-only check — no evaluation, no RNG, no persistence
/// (contracts/graphql-roll.md).
pub fn validate_dice_formula_impl(formula: &str) -> bool {
    DiceFormula::parse(formula).is_ok()
}

#[derive(Default)]
pub struct RollQuery;

#[async_graphql::Object]
impl RollQuery {
    async fn world_roll_records(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<GraphQLRollRecord>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        world_roll_records_impl(state, auth_user.user_id, auth_user.is_admin, world_id, limit).await
    }

    async fn validate_dice_formula(&self, _ctx: &Context<'_>, formula: String) -> GraphQLResult<bool> {
        Ok(validate_dice_formula_impl(&formula))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_roll::{RollDiceInput, roll_dice_impl};
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    struct StepRng(u64);
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

    #[test]
    fn validate_dice_formula_accepts_well_formed_and_rejects_malformed() {
        assert!(validate_dice_formula_impl("1d20 + STAT + MODIFIERS"));
        assert!(!validate_dice_formula_impl("1d20 +"));
    }

    #[tokio::test]
    async fn non_dm_cannot_view_roll_history() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        let result = world_roll_records_impl(&state, player_id, false, world_id, None).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn dm_sees_every_prior_roll_with_full_detail() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let mut rng = StepRng(0);
        roll_dice_impl(
            &state,
            owner_id,
            RollDiceInput { world_id, formula: "1d20".to_string(), bindings: None },
            &mut rng,
        )
        .await
        .unwrap();

        let history = world_roll_records_impl(&state, owner_id, false, world_id, None).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].resolution.dice.len(), 1);
    }
}
