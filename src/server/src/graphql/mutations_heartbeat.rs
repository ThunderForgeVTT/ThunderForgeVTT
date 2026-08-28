//! Session heartbeat: who is still here, and who quietly went away.
//!
//! # Why a heartbeat rather than the socket
//!
//! The obvious source of "is this client connected" is the WebSocket, and it
//! is the wrong one twice over.
//!
//! `graphql-ws` is a **lazy** client: it drops its connection when it has no
//! subscriptions to serve and opens a new one on demand. A closed socket
//! therefore means "nothing is subscribed right now", not "this person is
//! gone" — and a client that keyed offline behaviour on socket liveness would
//! start queueing edits during an ordinary idle moment.
//!
//! And the socket says nothing to anyone else. A Game Master watching a
//! player's token has no way to learn that the player's laptop shut its lid,
//! because the absence of a connection is not an event anybody publishes.
//!
//! A heartbeat answers both. The client says "still here" on a fixed
//! interval; silence past a threshold is a disconnection, which is a fact the
//! server can act on and tell other people about. It also gives the client a
//! deterministic signal of its own — a heartbeat that fails is a connection
//! that is down, whatever the socket happens to be doing.
//!
//! # What it deliberately is not
//!
//! Not a keep-alive for the WebSocket, which manages its own. Not a liveness
//! probe for the server. And not a place to carry game state: it says who,
//! where, and when, and everything else travels through the mutations that
//! own it.

use async_graphql::{Context, Object, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::{GraphQLResult, app_state, authenticated_user};

/// How long silence lasts before a client is considered gone.
///
/// Three missed beats at the client's five-second interval. One missed beat
/// is a garbage collection pause or a train tunnel; three is someone who
/// stopped being there. Erring long is right here because the cost of being
/// wrong is asymmetric: announcing a player has dropped when they have not is
/// visible to the whole table and momentarily wrong in public, while noticing
/// fifteen seconds late costs nothing anyone can see.
pub const PRESENCE_TIMEOUT_SECS: i64 = 15;

/// One participant's presence, as everyone else sees it.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
pub struct GraphQLPresence {
    pub user_id: Uuid,
    /// Which scene they are looking at, when they have said.
    pub scene_id: Option<Uuid>,
    /// Seconds since their last heartbeat.
    pub seconds_since_seen: i32,
    /// Whether that silence has passed [`PRESENCE_TIMEOUT_SECS`].
    pub connected: bool,
}

/// Decide whether a participant counts as present.
///
/// Pure, and separated from the query so the rule can be tested without a
/// database or a clock — the two things that make presence bugs hard to
/// reproduce.
pub fn is_connected(seconds_since_seen: i64) -> bool {
    seconds_since_seen <= PRESENCE_TIMEOUT_SECS
}

#[derive(Default)]
pub struct HeartbeatMutation;

#[Object]
impl HeartbeatMutation {
    /// Report that this client is still present in a world.
    ///
    /// Membership is checked on every beat rather than once at connect: a
    /// heartbeat is exactly the moment to notice that someone's access was
    /// revoked while they sat idle, and refusing it is how their client finds
    /// out.
    async fn heartbeat(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        scene_id: Option<Uuid>,
    ) -> GraphQLResult<bool> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        require_world_member(&mut conn, user_id, world_id)
            .map_err(|_| async_graphql::Error::new("Not a member of this world"))?;

        use crate::schema::players_online;
        let now = chrono::Utc::now().naive_utc();

        // Upsert, because a heartbeat is the *only* thing that establishes
        // presence now. Requiring a separate "connect" first would mean a
        // client whose first beat lands after a server restart is invisible
        // until it happens to reconnect a socket.
        diesel::insert_into(players_online::table)
            .values(&crate::models::NewPlayersOnline {
                player_id: user_id,
                world_id,
                scene_id,
                connected_at: now,
                last_seen: now,
                idle_duration_secs: 0,
                created_at: now,
                updated_at: now,
            })
            .on_conflict((players_online::player_id, players_online::world_id))
            .do_update()
            .set((
                players_online::last_seen.eq(now),
                players_online::scene_id.eq(scene_id),
                players_online::idle_duration_secs.eq(0),
                players_online::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .map_err(|e| async_graphql::Error::new(format!("Failed to record presence: {e}")))?;

        Ok(true)
    }
}

#[derive(Default)]
pub struct PresenceQuery;

#[Object]
impl PresenceQuery {
    /// Who is present in a world, and who has gone quiet.
    ///
    /// Returns everyone with a presence row, including those past the
    /// timeout, with `connected: false`. Filtering them out here would make a
    /// player who dropped simply vanish from the Game Master's list, which is
    /// indistinguishable from them never having been there — and "they were
    /// here and stopped" is the whole thing worth reporting.
    async fn world_presence(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLPresence>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| async_graphql::Error::new("Failed to get DB connection"))?;

        // Presence is world-scoped information about other people. Only
        // members may see it — the same rule `may_watch_world` applies to the
        // event stream, asked again here rather than assumed.
        require_world_member(&mut conn, auth_user.user_id, world_id)
            .map_err(|_| async_graphql::Error::new("Not a member of this world"))?;

        use crate::schema::players_online;
        let rows: Vec<(Uuid, Option<Uuid>, chrono::NaiveDateTime)> = players_online::table
            .filter(players_online::world_id.eq(world_id))
            .select((
                players_online::player_id,
                players_online::scene_id,
                players_online::last_seen,
            ))
            .load(&mut conn)
            .map_err(|e| async_graphql::Error::new(format!("Failed to read presence: {e}")))?;

        let now = chrono::Utc::now().naive_utc();
        Ok(rows
            .into_iter()
            .map(|(player_id, scene_id, last_seen)| {
                let seconds = (now - last_seen).num_seconds().max(0);
                GraphQLPresence {
                    user_id: player_id,
                    scene_id,
                    seconds_since_seen: i32::try_from(seconds).unwrap_or(i32::MAX),
                    connected: is_connected(seconds),
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The threshold is three missed beats at the client's five-second
    /// interval. One missed beat is a garbage collection pause or a tunnel.
    #[test]
    fn a_recent_beat_is_present_and_a_long_silence_is_not() {
        assert!(is_connected(0), "a beat just now");
        assert!(is_connected(5), "one interval");
        assert!(is_connected(PRESENCE_TIMEOUT_SECS), "exactly at the edge");
        assert!(!is_connected(PRESENCE_TIMEOUT_SECS + 1), "past the edge");
        assert!(!is_connected(600), "gone for ten minutes");
    }

    /// A clock that stepped backwards must not make someone look present
    /// forever, nor panic the conversion. Negative ages are clamped at the
    /// call site; the rule itself simply treats them as recent.
    #[test]
    fn a_clock_that_went_backwards_still_reads_as_present() {
        assert!(is_connected(-5));
    }

    #[tokio::test]
    async fn a_heartbeat_records_presence_and_a_second_one_refreshes_it() {
        use crate::schema::players_online;
        let state = crate::test_support::test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = crate::test_support::insert_test_user(&mut conn);
        let world = crate::test_support::insert_test_world(&mut conn, owner);
        let scene = crate::test_support::insert_test_scene(&mut conn, world, owner);
        let now = chrono::Utc::now().naive_utc();

        // Stand in for the mutation's body, which needs a GraphQL context.
        let beat = |conn: &mut PgConnection, at: chrono::NaiveDateTime| {
            diesel::insert_into(players_online::table)
                .values(&crate::models::NewPlayersOnline {
                    player_id: owner,
                    world_id: world,
                    scene_id: Some(scene),
                    connected_at: at,
                    last_seen: at,
                    idle_duration_secs: 0,
                    created_at: at,
                    updated_at: at,
                })
                .on_conflict((players_online::player_id, players_online::world_id))
                .do_update()
                .set((
                    players_online::last_seen.eq(at),
                    players_online::idle_duration_secs.eq(0),
                ))
                .execute(conn)
                .expect("heartbeat should record presence");
        };

        // An old beat, then a fresh one. The second must *update* rather than
        // insert a second row — a heartbeat every five seconds would
        // otherwise grow the table without bound.
        beat(&mut conn, now - chrono::Duration::seconds(120));
        beat(&mut conn, now);

        let rows: Vec<chrono::NaiveDateTime> = players_online::table
            .filter(players_online::world_id.eq(world))
            .filter(players_online::player_id.eq(owner))
            .select(players_online::last_seen)
            .load(&mut conn)
            .expect("presence should be readable");

        assert_eq!(rows.len(), 1, "a heartbeat must upsert, never accumulate");
        let age = (now - rows[0]).num_seconds();
        assert!(is_connected(age), "the refreshed beat reads as present");
    }
}
