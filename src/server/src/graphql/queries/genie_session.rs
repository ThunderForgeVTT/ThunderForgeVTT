//! Spec 018 (User Story 7): `genieSession(worldId)` and
//! `genieResourceHoldings(sessionId, actorId)` — read-side of the Genie
//! session loop. See `contracts/genie-session-loop.md`.

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::mutations_genie_session::{
    GraphQLGenieResourceHolding, GraphQLGenieSession,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{GenieResourceHolding, GenieSession, GeniePuzzleClock};
use crate::schema::{world_genie_puzzle_clocks, world_genie_resource_holdings, world_genie_sessions};
use crate::state::AppState;

fn build_graphql_session(session: GenieSession, clocks: Vec<GeniePuzzleClock>) -> GraphQLGenieSession {
    // Local re-implementation of mutations_genie_session's private
    // builder (kept private there); trivial enough to duplicate rather
    // than widen that module's visibility for one call site.
    use crate::graphql::mutations_genie_session::GraphQLGeniePuzzleClock;
    GraphQLGenieSession {
        id: session.id,
        world_id: session.world_id,
        wishes_remaining: session.wishes_remaining,
        doom_clock_current: session.doom_clock_current,
        doom_clock_max: session.doom_clock_max,
        status: match session.status.as_str() {
            "won" => crate::graphql::mutations_genie_session::GenieSessionStatus::Won,
            "lost" => crate::graphql::mutations_genie_session::GenieSessionStatus::Lost,
            _ => crate::graphql::mutations_genie_session::GenieSessionStatus::Active,
        },
        puzzle_clocks: clocks.into_iter().map(GraphQLGeniePuzzleClock::from).collect(),
    }
}

/// Testable core of `GenieSessionQuery::genie_session`. Returns the
/// world's active session, if any — any world member may read it (not
/// GM-only; every connected player needs to see the shared state).
pub async fn genie_session_impl(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
) -> GraphQLResult<Option<GraphQLGenieSession>> {
    let mut conn = state.db_pool.get().map_err(|_| Error::new("Failed to get DB connection"))?;

    let result = tokio::task::spawn_blocking(move || -> Result<Option<(GenieSession, Vec<GeniePuzzleClock>)>, String> {
        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You must be a member of this world".to_string())?;

        let session = world_genie_sessions::table
            .filter(world_genie_sessions::world_id.eq(world_id))
            .filter(world_genie_sessions::status.eq("active"))
            .order(world_genie_sessions::created_at.desc())
            .select(GenieSession::as_select())
            .first::<GenieSession>(&mut conn)
            .optional()
            .map_err(|e| format!("Failed to load Genie session: {e}"))?;

        let Some(session) = session else {
            return Ok(None);
        };

        let clocks = world_genie_puzzle_clocks::table
            .filter(world_genie_puzzle_clocks::session_id.eq(session.id))
            .order(world_genie_puzzle_clocks::created_at.asc())
            .select(GeniePuzzleClock::as_select())
            .load::<GeniePuzzleClock>(&mut conn)
            .map_err(|e| format!("Failed to load Puzzle Clocks: {e}"))?;

        Ok(Some((session, clocks)))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(result.map(|(session, clocks)| build_graphql_session(session, clocks)))
}

/// Testable core of `GenieSessionQuery::genie_resource_holdings`.
/// Optionally scoped to a single actor; any world member may read (a
/// player needs to see the whole party's holdings to negotiate trades).
pub async fn genie_resource_holdings_impl(
    state: &AppState,
    user_id: Uuid,
    session_id: Uuid,
    actor_id: Option<Uuid>,
) -> GraphQLResult<Vec<GraphQLGenieResourceHolding>> {
    let mut conn = state.db_pool.get().map_err(|_| Error::new("Failed to get DB connection"))?;

    let holdings = tokio::task::spawn_blocking(move || -> Result<Vec<GenieResourceHolding>, String> {
        let world_id = world_genie_sessions::table
            .filter(world_genie_sessions::id.eq(session_id))
            .select(world_genie_sessions::world_id)
            .first::<Uuid>(&mut conn)
            .map_err(|_| "Genie session not found".to_string())?;

        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| "You must be a member of this world".to_string())?;

        let mut query = world_genie_resource_holdings::table
            .filter(world_genie_resource_holdings::session_id.eq(session_id))
            .into_boxed();
        if let Some(actor_id) = actor_id {
            query = query.filter(world_genie_resource_holdings::actor_id.eq(actor_id));
        }

        query
            .select(GenieResourceHolding::as_select())
            .load::<GenieResourceHolding>(&mut conn)
            .map_err(|e| format!("Failed to load resource holdings: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(holdings.into_iter().map(GraphQLGenieResourceHolding::from).collect())
}

#[derive(Default)]
pub struct GenieSessionQuery;

#[async_graphql::Object]
impl GenieSessionQuery {
    async fn genie_session(&self, ctx: &Context<'_>, world_id: Uuid) -> GraphQLResult<Option<GraphQLGenieSession>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        genie_session_impl(state, auth_user.user_id, world_id).await
    }

    async fn genie_resource_holdings(
        &self,
        ctx: &Context<'_>,
        session_id: Uuid,
        actor_id: Option<Uuid>,
    ) -> GraphQLResult<Vec<GraphQLGenieResourceHolding>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        genie_resource_holdings_impl(state, auth_user.user_id, session_id, actor_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_genie_session::{start_genie_session_impl, StartGenieSessionInput};
    use crate::test_support::{insert_test_user, insert_test_world, insert_test_world_member, test_app_state};

    #[tokio::test]
    async fn genie_session_returns_none_when_no_active_session_exists() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let result = genie_session_impl(&state, owner_id, world_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn genie_session_visible_to_any_world_member_not_just_the_gm() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        drop(conn);

        start_genie_session_impl(&state, owner_id, false, StartGenieSessionInput { world_id, doom_clock_max: 6 })
            .await
            .unwrap();

        let result = genie_session_impl(&state, player_id, world_id).await.unwrap();
        assert!(result.is_some(), "a player, not just the GM, should be able to read the session's shared state");
        assert_eq!(result.unwrap().wishes_remaining, 3);
    }
}
