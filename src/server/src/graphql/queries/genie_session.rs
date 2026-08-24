//! Spec 018 (User Story 7): `genieSession(worldId)` and
//! `genieResourceHoldings(sessionId, actorId)` — read-side of the Genie
//! session loop. See `contracts/genie-session-loop.md`.

use async_graphql::{Context, Error, Result as GraphQLResult};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::mutations_genie_session::{
    require_caller_controls_actor, GraphQLGenieResourceHolding, GraphQLGenieSession,
    GraphQLGenieTradeProposal,
};
use crate::graphql::{app_state, authenticated_user};
use crate::models::{GenieResourceHolding, GenieSession, GeniePuzzleClock, GenieTradeProposal};
use crate::schema::{
    world_genie_puzzle_clocks, world_genie_resource_holdings, world_genie_sessions,
    world_genie_trade_proposals,
};
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

/// Testable core of `GenieSessionQuery::genie_trade_proposals`. Spec 019:
/// the read-side `proposeResourceTrade`/`acceptResourceTrade` were
/// missing — a player had no way to discover a trade proposed to them.
/// Scoped to proposals still awaiting a response (`status = "pending"`;
/// `accept_resource_trade_impl` flips accepted ones to `"accepted"` via
/// UPDATE, never deletes), naming `actor_id` as the *recipient*
/// (`to_actor_id`) — only that actor's controller (or the world's GM) may
/// see what's been proposed to them, mirroring the same
/// `require_caller_controls_actor` check `acceptResourceTrade` itself
/// already enforces.
pub async fn genie_trade_proposals_impl(
    state: &AppState,
    user_id: Uuid,
    is_admin: bool,
    actor_id: Uuid,
) -> GraphQLResult<Vec<GraphQLGenieTradeProposal>> {
    require_caller_controls_actor(state, user_id, is_admin, actor_id).await?;

    let mut conn = state.db_pool.get().map_err(|_| Error::new("Failed to get DB connection"))?;

    let proposals = tokio::task::spawn_blocking(move || -> Result<Vec<GenieTradeProposal>, String> {
        world_genie_trade_proposals::table
            .filter(world_genie_trade_proposals::to_actor_id.eq(actor_id))
            .filter(world_genie_trade_proposals::status.eq("pending"))
            .order(world_genie_trade_proposals::created_at.desc())
            .select(GenieTradeProposal::as_select())
            .load::<GenieTradeProposal>(&mut conn)
            .map_err(|e| format!("Failed to load trade proposals: {e}"))
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(Error::new)?;

    Ok(proposals.into_iter().map(GraphQLGenieTradeProposal::from).collect())
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

    async fn genie_trade_proposals(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLGenieTradeProposal>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        genie_trade_proposals_impl(state, auth_user.user_id, auth_user.is_admin, actor_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphql::mutations_genie_session::{
        propose_resource_trade_impl, start_genie_session_impl, StartGenieSessionInput,
    };
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    };

    fn insert_test_actor(conn: &mut PgConnection, world_id: Uuid, scene_id: Uuid, owner_id: Uuid) -> Uuid {
        use crate::schema::world_actors;
        let now = chrono::Utc::now().naive_utc();
        let actor_id = Uuid::now_v7();
        diesel::insert_into(world_actors::table)
            .values((
                world_actors::id.eq(actor_id),
                world_actors::world_id.eq(world_id),
                world_actors::scene_id.eq(scene_id),
                world_actors::actor_type.eq("character"),
                world_actors::game_system_id.eq("genie"),
                world_actors::label.eq("Test Actor"),
                world_actors::created_by.eq(owner_id),
                world_actors::owned_by.eq(owner_id),
                world_actors::is_public.eq(false),
                world_actors::is_npc.eq(false),
                world_actors::created_at.eq(now),
                world_actors::updated_at.eq(now),
                world_actors::available_for_claim.eq(false),
            ))
            .execute(conn)
            .expect("failed to insert test actor");
        actor_id
    }

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

    #[tokio::test]
    async fn genie_trade_proposals_lists_only_pending_proposals_addressed_to_the_actor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let from_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let unrelated_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        // A different owner so accept_resource_trade_impl's self-accept
        // rejection (caller == proposal.created_by) doesn't fire below —
        // all proposals here are created by `owner_id`.
        let player_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player_id, "Player");
        let to_actor = insert_test_actor(&mut conn, world_id, scene_id, player_id);
        drop(conn);

        let session = start_genie_session_impl(
            &state,
            owner_id,
            false,
            StartGenieSessionInput { world_id, doom_clock_max: 6 },
        )
        .await
        .unwrap();

        // Not addressed to `to_actor` — must not show up.
        propose_resource_trade_impl(
            &state, owner_id, false, session.id, from_actor, "insight".to_string(), 1, unrelated_actor,
            "favor".to_string(), 1,
        )
        .await
        .unwrap();

        let pending = propose_resource_trade_impl(
            &state, owner_id, false, session.id, from_actor, "insight".to_string(), 2, to_actor,
            "favor".to_string(), 1,
        )
        .await
        .unwrap();

        let accepted = propose_resource_trade_impl(
            &state, owner_id, false, session.id, from_actor, "insight".to_string(), 3, to_actor,
            "essence".to_string(), 1,
        )
        .await
        .unwrap();
        // Flip status directly rather than driving the real
        // acceptResourceTrade mutation (which also enforces holdings
        // sufficiency) — irrelevant to what this query's own "pending
        // only" filter is testing.
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(world_genie_trade_proposals::table.filter(world_genie_trade_proposals::id.eq(accepted.id)))
            .set(world_genie_trade_proposals::status.eq("accepted"))
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let result = genie_trade_proposals_impl(&state, owner_id, false, to_actor).await.unwrap();

        assert_eq!(result.len(), 1, "only the still-pending proposal addressed to to_actor should be listed");
        assert_eq!(result[0].id, pending.id);
    }

    #[tokio::test]
    async fn genie_trade_proposals_rejects_a_caller_who_does_not_control_the_actor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
        let to_actor = insert_test_actor(&mut conn, world_id, scene_id, owner_id);
        let stranger_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, stranger_id, "Player");
        drop(conn);

        let result = genie_trade_proposals_impl(&state, stranger_id, false, to_actor).await;
        assert!(result.is_err());
    }
}
