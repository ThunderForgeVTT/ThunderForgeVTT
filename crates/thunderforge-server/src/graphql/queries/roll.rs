//! Spec 014: `worldRollRecords` (DM-only roll history, FR-014) and
//! `validateDiceFormula` (pure parse-only check, any caller); spec 081's
//! `worldRoll` and `worldRolls`, the table's own view of its rolls. See
//! `specs/014-dice-rolling-engine/contracts/graphql-roll.md`.

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use std::collections::HashMap;

use crate::auth::world_membership::{actor_in_world, is_dm_of_world, require_world_member};
use crate::graphql::types::{GraphQLRollRecord, MaskedRoll, WorldRoll, WorldRollEntry};
use crate::graphql::{app_state, authenticated_user};
use crate::models::RollRecord;
use crate::rolls::visibility::{RollFacts, RollView, Viewer, Visibility, view_of};
use crate::schema::{users, world_roll_records};
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
        return Err(Error::new(
            "Only the DM (Owner or GM) may view roll history",
        ));
    }

    let take = limit
        .map(|n| n as i64)
        .unwrap_or(DEFAULT_ROLL_RECORD_LIMIT)
        .clamp(1, 500);

    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    let records: Vec<RollRecord> = tokio::task::spawn_blocking(move || {
        world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .order(world_roll_records::created_at.desc())
            .limit(take)
            .select(RollRecord::as_select())
            .load::<RollRecord>(&mut conn)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to load roll history"))?;

    Ok(records.into_iter().map(GraphQLRollRecord::from).collect())
}

// ============================================================================
// Spec 081: the table's view of its rolls.
// ============================================================================

const DEFAULT_FEED_LIMIT: i64 = 50;

/// Who is asking, as the visibility rule needs it. A member, or an admin who
/// may read any world; anyone else is refused.
pub fn viewer_in_world(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
) -> GraphQLResult<Viewer> {
    if !is_admin && require_world_member(conn, user_id, world_id).is_err() {
        return Err(Error::new("You must be a member of this world"));
    }
    Ok(Viewer {
        user_id,
        is_gm: actor_in_world(conn, user_id, false, world_id).runs_the_world(),
        is_admin,
    })
}

/// Usernames for these accounts, the name chat shows a speaker by.
pub fn usernames(conn: &mut PgConnection, ids: &[Uuid]) -> QueryResult<HashMap<Uuid, String>> {
    Ok(users::table
        .filter(users::id.eq_any(ids))
        .select((users::id, users::username))
        .load::<(Uuid, String)>(conn)?
        .into_iter()
        .collect())
}

/// One row as this viewer may see it, or nothing (FR-003).
pub fn entry_for(
    row: RollRecord,
    viewer: Viewer,
    names: &HashMap<Uuid, String>,
) -> Option<WorldRollEntry> {
    let facts = RollFacts {
        roller: row.triggered_by,
        visibility: Visibility::parse(&row.visibility),
        revealed: row.revealed_at.is_some(),
    };
    let name = |id: Uuid| names.get(&id).cloned().unwrap_or_default();
    match view_of(facts, viewer) {
        RollView::Whole => {
            let roller = name(row.triggered_by);
            let revealer = row.revealed_by.map(name);
            Some(WorldRollEntry::WorldRoll(Box::new(WorldRoll::from_row(
                row, roller, revealer,
            ))))
        }
        RollView::Masked => Some(WorldRollEntry::MaskedRoll(MaskedRoll::new(
            row.id,
            name(row.triggered_by),
            row.created_at,
            facts.visibility,
        ))),
        RollView::Hidden => None,
    }
}

/// Rows and the names they mention, turned into what this viewer may see.
fn entries(
    conn: &mut PgConnection,
    rows: Vec<RollRecord>,
    viewer: Viewer,
) -> QueryResult<Vec<WorldRollEntry>> {
    let ids: Vec<Uuid> = rows
        .iter()
        .flat_map(|row| std::iter::once(row.triggered_by).chain(row.revealed_by))
        .collect();
    let names = usernames(conn, &ids)?;
    Ok(rows
        .into_iter()
        .filter_map(|row| entry_for(row, viewer, &names))
        .collect())
}

/// `worldRoll`: one roll as the caller may see it. A roll hidden from the
/// caller answers exactly as one that does not exist (FR-005a).
pub async fn world_roll_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    roll_id: Uuid,
) -> GraphQLResult<Option<WorldRollEntry>> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        let viewer = viewer_in_world(&mut conn, user_id, is_admin, world_id)?;
        let row = world_roll_records::table
            .filter(world_roll_records::id.eq(roll_id))
            .filter(world_roll_records::world_id.eq(world_id))
            .select(RollRecord::as_select())
            .first::<RollRecord>(&mut conn)
            .optional()
            .map_err(|_| Error::new("Failed to load the roll"))?;
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(entries(&mut conn, vec![row], viewer)
            .map_err(|_| Error::new("Failed to load the roll"))?
            .pop())
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// `worldRolls`: the table's rolls, newest first, before `before`. A roll
/// hidden from the caller is left out in the query, so a page is never short
/// for having held one (FR-005).
pub async fn world_rolls_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    world_id: Uuid,
    before: Option<String>,
    limit: Option<i32>,
) -> GraphQLResult<Vec<WorldRollEntry>> {
    let before = match before {
        Some(at) => Some(
            chrono::DateTime::parse_from_rfc3339(&at)
                .map_err(|_| Error::new("`before` is not a time"))?
                .with_timezone(&chrono::Utc),
        ),
        None => None,
    };
    let take = limit
        .map(i64::from)
        .unwrap_or(DEFAULT_FEED_LIMIT)
        .clamp(1, 100);
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;
    tokio::task::spawn_blocking(move || {
        let viewer = viewer_in_world(&mut conn, user_id, is_admin, world_id)?;
        let mut query = world_roll_records::table
            .filter(world_roll_records::world_id.eq(world_id))
            .select(RollRecord::as_select())
            .order(world_roll_records::created_at.desc())
            .limit(take)
            .into_boxed();
        if let Some(before) = before {
            query = query.filter(world_roll_records::created_at.lt(before));
        }
        if !(viewer.is_gm || viewer.is_admin) {
            query = query.filter(
                world_roll_records::visibility
                    .ne(Visibility::GmOnly.as_str())
                    .or(world_roll_records::revealed_at.is_not_null())
                    .or(world_roll_records::triggered_by.eq(user_id)),
            );
        }
        let rows = query
            .load::<RollRecord>(&mut conn)
            .map_err(|_| Error::new("Failed to load rolls"))?;
        entries(&mut conn, rows, viewer).map_err(|_| Error::new("Failed to load rolls"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
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
        world_roll_records_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            limit,
        )
        .await
    }

    /// Spec 081: one roll as the caller may see it; null when it does not
    /// exist or is hidden from the caller.
    async fn world_roll(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        roll_id: Uuid,
    ) -> GraphQLResult<Option<WorldRollEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        world_roll_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            roll_id,
        )
        .await
    }

    /// Spec 081: the table's rolls as the caller may see them, newest first,
    /// before `before` (an RFC 3339 time, exclusive). Any member. `limit`
    /// 1–100, default 50.
    async fn world_rolls(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        before: Option<String>,
        limit: Option<i32>,
    ) -> GraphQLResult<Vec<WorldRollEntry>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        world_rolls_impl(
            state,
            auth_user.user_id,
            auth_user.is_admin,
            world_id,
            before,
            limit,
        )
        .await
    }

    async fn validate_dice_formula(
        &self,
        _ctx: &Context<'_>,
        formula: String,
    ) -> GraphQLResult<bool> {
        Ok(validate_dice_formula_impl(&formula))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_roll::{RollDiceInput, roll_dice_impl};
    use crate::test_support::{
        insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

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
            RollDiceInput {
                world_id,
                formula: "1d20".to_string(),
                bindings: None,
                visibility: None,
                label: None,
            },
            &mut rng,
        )
        .await
        .unwrap();

        let history = world_roll_records_impl(&state, owner_id, false, world_id, None)
            .await
            .unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].resolution.dice.len(), 1);
    }
}
