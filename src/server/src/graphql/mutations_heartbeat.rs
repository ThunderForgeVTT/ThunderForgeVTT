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
use uuid::Uuid;

use crate::auth::world_membership::require_world_member;
use crate::graphql::{GraphQLResult, app_state, authenticated_user};

/// One participant's presence, as everyone else sees it.
#[derive(SimpleObject, Clone, Debug, PartialEq, Eq)]
pub struct GraphQLPresence {
    pub user_id: Uuid,
    /// Which scene they are looking at, when they have said.
    pub scene_id: Option<Uuid>,
    /// Seconds since their last heartbeat.
    pub seconds_since_seen: i32,
    /// Whether that silence has passed
    /// [`thunderforge_presence::PRESENCE_TIMEOUT`].
    ///
    /// Three missed beats at the client's five-second interval. One missed
    /// beat is a garbage collection pause or a train tunnel; three is someone
    /// who stopped being there. Erring long is right because the cost of
    /// being wrong is asymmetric: announcing a player has dropped when they
    /// have not is visible to the whole table and momentarily wrong in
    /// public, while noticing fifteen seconds late costs nothing anyone can
    /// see.
    pub connected: bool,
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

        // In memory, not in a row. A beat is a statement about *now*, and
        // its answer is worthless one beat later — writing it to Postgres
        // cost a WAL record, an index update and a dead tuple every five
        // seconds per connected client, for a fact nothing durable needs.
        //
        // The membership check above deliberately still runs on every beat.
        // Caching it would be the obvious next saving and is the wrong one:
        // refusing a beat is how a client learns its access was revoked while
        // it sat idle, and a cache would put a delay on exactly that.
        state
            .presence
            .beat(world_id, user_id, scene_id, std::time::Instant::now());

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

        Ok(state
            .presence
            .in_world(world_id, std::time::Instant::now())
            .into_iter()
            .map(|person| GraphQLPresence {
                user_id: person.user_id,
                scene_id: person.scene_id,
                seconds_since_seen: i32::try_from(person.since_seen.as_secs()).unwrap_or(i32::MAX),
                connected: person.connected,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A second beat refreshes one person rather than adding another.
    ///
    /// This replaces a test that stood in for the mutation's body by running
    /// its `INSERT ... ON CONFLICT` by hand against a live database. It
    /// asserted "a heartbeat must upsert, never accumulate" — a property of a
    /// table that no longer holds presence. The property that matters is the
    /// same one, and it is now about the registry the mutation actually
    /// writes to, so it needs no database and takes microseconds.
    #[test]
    fn a_second_beat_refreshes_one_person_rather_than_adding_another() {
        use std::time::{Duration, Instant};

        let registry = thunderforge_presence::PresenceRegistry::new();
        let world = Uuid::from_u128(1);
        let player = Uuid::from_u128(2);
        let scene = Uuid::from_u128(3);
        let start = Instant::now();

        registry.beat(world, player, Some(scene), start);
        let much_later = start + Duration::from_secs(120);
        registry.beat(world, player, Some(scene), much_later);

        let people = registry.in_world(world, much_later);
        assert_eq!(people.len(), 1, "a beat must refresh, never accumulate");
        assert!(people[0].connected, "the refreshed beat reads as present");
        assert_eq!(people[0].scene_id, Some(scene));
    }
}
