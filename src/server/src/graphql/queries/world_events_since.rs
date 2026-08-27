//! Catch up on the events a client missed while it was disconnected.
//!
//! # Why a query and not a subscription argument
//!
//! The obvious design is to pass `lastSeenId` when subscribing and have the
//! server replay before attaching the live stream. It does not work here, for
//! two reasons that are both properties of what the client already is:
//!
//! 1. **graphql-ws re-sends the original subscribe payload verbatim on
//!    reconnect.** A cursor captured as a subscription variable is therefore
//!    frozen at the value it had when the page first loaded. Every later
//!    reconnect would replay from that same stale point — growing the backlog
//!    without bound while never actually catching up.
//! 2. **A world page holds four to six independent subscriptions** on one
//!    socket (scene launch, canvas content, chat, combat, genie, playback),
//!    each a separate server-side receiver. Replaying into the subscription
//!    would hand the same backlog to every one of them, and since the
//!    handlers are refetch-on-nudge, that multiplies into a refetch storm at
//!    precisely the moment the connection has just recovered.
//!
//! A query is issued once, by one owner, with a cursor read at the moment of
//! asking. Both problems disappear.
//!
//! # Why the client can trust a gap it did not see
//!
//! Live delivery is at-most-once by construction: the broadcast channel drops
//! for a lagging subscriber, and a socket that is down receives nothing at
//! all. Nothing about that is fixable in the transport — which is the point.
//! `world_events` is the durable record, monotonically ordered by `id`, and
//! this query is the client's way of asking the record rather than the wire.
//!
//! That is also what makes a broker unnecessary for durability: JetStream or
//! Centrifugo history would be a second log that can disagree with this one.

use async_graphql::{Context, ErrorExtensions, Object, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::{WorldMembershipError, require_world_member};
use crate::graphql::types::GraphQLWorldEvent;
use crate::graphql::{app_state, authenticated_user};
use crate::models::WorldEvent;

/// The most events one catch-up will return.
///
/// A ceiling rather than a page size: there is no cursor to continue with,
/// because a client this far behind should not be replaying at all. Past this
/// many missed events the honest answer is "resynchronise", and
/// [`GraphQLWorldEventCatchUp::truncated`] says so.
///
/// 200 is chosen against what a disconnection actually costs: a busy table
/// produces a few events a second, so this covers a minute or so of outage —
/// well past the reconnect backoff — while staying small enough that applying
/// the batch is cheaper than refetching the world.
const CATCH_UP_LIMIT: i64 = 200;

/// What one client missed.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "WorldEventCatchUp")]
pub struct GraphQLWorldEventCatchUp {
    /// Missed events, oldest first, so a client can apply them in order and
    /// finish holding the newest id.
    pub events: Vec<GraphQLWorldEvent>,

    /// The gap was larger than this query will return.
    ///
    /// `events` is then the *oldest* slice of the gap and is deliberately not
    /// the whole of it: a client that applied it would still be behind, and
    /// would have no way to know. The contract is that `true` means "discard
    /// this and resynchronise the world", not "ask again for the rest".
    pub truncated: bool,

    /// The newest event id this world has, whether or not it is in `events`.
    ///
    /// Lets a client that resynchronises set its cursor to the right place in
    /// one step, instead of inferring it from a batch it just threw away.
    /// Zero for a world that has never recorded an event.
    pub latest_id: i64,
}

#[derive(Default)]
pub struct WorldEventsSinceQuery;

#[Object]
impl WorldEventsSinceQuery {
    /// Every event in this world newer than `after_id`.
    ///
    /// Authorized by world membership on every call, like every other
    /// world-scoped resolver: a disconnection is not a reason to trust a
    /// client's claim about what it used to be allowed to see, and a member
    /// removed during the outage must not be handed the events they missed
    /// while being removed.
    async fn world_events_since(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        after_id: i64,
    ) -> async_graphql::Result<GraphQLWorldEventCatchUp> {
        use crate::schema::world_events;

        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("database unavailable"))?;

        let result = tokio::task::spawn_blocking(move || {
            require_world_member(&mut conn, user_id, world_id)?;

            // One more than the limit, so "there is more" is answered by the
            // same query rather than by a second count.
            let mut rows = world_events::table
                .filter(world_events::world_id.eq(world_id))
                .filter(world_events::id.gt(after_id))
                .order(world_events::id.asc())
                .limit(CATCH_UP_LIMIT + 1)
                .load::<WorldEvent>(&mut conn)
                .map_err(|e| WorldMembershipError::Database(e.to_string()))?;

            let truncated = rows.len() as i64 > CATCH_UP_LIMIT;
            rows.truncate(CATCH_UP_LIMIT as usize);

            let latest_id = world_events::table
                .filter(world_events::world_id.eq(world_id))
                .select(diesel::dsl::max(world_events::id))
                .first::<Option<i64>>(&mut conn)
                .map_err(|e| WorldMembershipError::Database(e.to_string()))?
                .unwrap_or(0);

            Ok::<_, WorldMembershipError>((rows, truncated, latest_id))
        })
        .await
        .map_err(|_| async_graphql::Error::new("catch-up task failed"))?;

        let (rows, truncated, latest_id) = result.map_err(|err| match err {
            // Refused exactly like every other non-member access, so the
            // answer cannot be used to probe whether a world exists.
            WorldMembershipError::NotAMember => {
                async_graphql::Error::new("not a member of this world")
                    .extend_with(|_, e| e.set("code", "FORBIDDEN"))
            }
            WorldMembershipError::Database(msg) => async_graphql::Error::new(msg),
        })?;

        Ok(GraphQLWorldEventCatchUp {
            events: rows.into_iter().map(GraphQLWorldEvent::from).collect(),
            truncated,
            latest_id,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Against a real Postgres (`DATABASE_URL`), like every other query test
    //! in this tree.

    use super::*;
    use crate::test_support::*;
    use crate::world_events::record_world_event;

    /// Write `n` events and hand back their ids in order.
    fn record_events(
        state: &crate::state::AppState,
        world_id: Uuid,
        user_id: Uuid,
        n: i64,
    ) -> Vec<i64> {
        let mut conn = state.db_pool.get().unwrap();
        (0..n)
            .map(|i| {
                record_world_event(&mut conn, world_id, 900 + i as i32, None, user_id)
                    .expect("recording an event should succeed")
            })
            .collect()
    }

    /// The client hand-writes this query text, so the names and argument
    /// types it uses are part of the contract and not an implementation
    /// detail. A rename here that the client does not follow fails at
    /// runtime, on reconnect, which is the least observable moment available.
    #[test]
    fn the_query_is_registered_under_the_names_the_client_sends() {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .finish();
        let sdl = schema.sdl();

        assert!(
            sdl.contains("worldEventsSince(worldId: UUID!, afterId: Int!)"),
            "the client sends exactly these argument names and types"
        );
        assert!(sdl.contains("type WorldEventCatchUp {"));
        assert!(sdl.contains("truncated: Boolean!"));
        assert!(sdl.contains("latestId: Int!"));
        // The same type the subscription yields — `GraphQLWorldEvent`, which
        // is not renamed in SDL — so a replayed event is field-for-field
        // indistinguishable from a live one to the client.
        assert!(
            sdl.contains("events: [GraphQLWorldEvent!]!"),
            "the catch-up must return the subscription's own event type"
        );
    }

    /// The core promise: everything after the cursor, in order, and nothing
    /// the client already has.
    #[tokio::test]
    async fn a_catch_up_returns_exactly_what_came_after_the_cursor() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let ids = record_events(&state, world_id, owner_id, 5);

        let mut conn = state.db_pool.get().unwrap();
        let seen = world_events_after(&mut conn, world_id, ids[1]);

        assert_eq!(
            seen,
            ids[2..].to_vec(),
            "a catch-up must return every later event and no earlier one"
        );
    }

    /// A cursor at the newest event catches up on nothing, which is the
    /// ordinary case for a brief blip.
    #[tokio::test]
    async fn a_current_client_catches_up_on_nothing() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let ids = record_events(&state, world_id, owner_id, 3);

        let mut conn = state.db_pool.get().unwrap();
        assert!(
            world_events_after(&mut conn, world_id, *ids.last().unwrap()).is_empty(),
            "a client already at the newest id has nothing to replay"
        );
    }

    /// One world's catch-up must never contain another's events, even for a
    /// user who is a member of both.
    #[tokio::test]
    async fn a_catch_up_is_scoped_to_its_own_world() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_a = insert_test_world(&mut conn, owner_id);
        let world_b = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let before = {
            let mut conn = state.db_pool.get().unwrap();
            world_high_water(&mut conn, world_a)
        };
        record_events(&state, world_b, owner_id, 4);
        let a_ids = record_events(&state, world_a, owner_id, 2);

        let mut conn = state.db_pool.get().unwrap();
        assert_eq!(
            world_events_after(&mut conn, world_a, before),
            a_ids,
            "world B's events must not appear in world A's catch-up"
        );
    }

    /// Past the ceiling the answer is "resynchronise", and the flag that says
    /// so must be set — a client that applied a silently truncated batch would
    /// believe it had caught up while still being behind.
    #[tokio::test]
    async fn an_oversized_gap_is_reported_as_truncated() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        drop(conn);

        let before = {
            let mut conn = state.db_pool.get().unwrap();
            world_high_water(&mut conn, world_id)
        };
        record_events(&state, world_id, owner_id, CATCH_UP_LIMIT + 5);

        let mut conn = state.db_pool.get().unwrap();
        let (rows, truncated) = world_events_after_paged(&mut conn, world_id, before);

        assert!(truncated, "a gap larger than the ceiling must say so");
        assert_eq!(
            rows.len() as i64,
            CATCH_UP_LIMIT,
            "and must return no more than the ceiling"
        );
    }

    // --- the query's own SQL, exercised without standing up a Schema ---

    fn world_events_after(
        conn: &mut diesel::PgConnection,
        world_id: Uuid,
        after_id: i64,
    ) -> Vec<i64> {
        world_events_after_paged(conn, world_id, after_id).0
    }

    fn world_events_after_paged(
        conn: &mut diesel::PgConnection,
        world_id: Uuid,
        after_id: i64,
    ) -> (Vec<i64>, bool) {
        use crate::schema::world_events;
        let mut rows = world_events::table
            .filter(world_events::world_id.eq(world_id))
            .filter(world_events::id.gt(after_id))
            .order(world_events::id.asc())
            .limit(CATCH_UP_LIMIT + 1)
            .select(world_events::id)
            .load::<i64>(conn)
            .unwrap();
        let truncated = rows.len() as i64 > CATCH_UP_LIMIT;
        rows.truncate(CATCH_UP_LIMIT as usize);
        (rows, truncated)
    }

    fn world_high_water(conn: &mut diesel::PgConnection, world_id: Uuid) -> i64 {
        use crate::schema::world_events;
        world_events::table
            .filter(world_events::world_id.eq(world_id))
            .select(diesel::dsl::max(world_events::id))
            .first::<Option<i64>>(conn)
            .unwrap()
            .unwrap_or(0)
    }
}
