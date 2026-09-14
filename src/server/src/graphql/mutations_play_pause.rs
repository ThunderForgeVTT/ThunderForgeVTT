//! Spec 051: an operator pauses a world's play (contracts/graphql.md).
//!
//! Every field here is `admin_user`-guarded and none calls the pause gate: an
//! operator acting on a paused world is the point, not a case to refuse
//! (contracts/live-play-lock.md, the `OPERATOR` table). The work is
//! `play_pause`'s; this file is its wire.
//!
//! A world's Owner is refused here as any non-operator is (FR-006, FR-040).
//! No world role reaches this file, which is ADR-100 decision 1.

use async_graphql::{Context, Object, SimpleObject};
use uuid::Uuid;

use crate::graphql::queries::play_pause::{GraphQLPlayPause, play_pauses_for_graphql};
use crate::graphql::{Error, GraphQLResult, admin_user, app_state};
use crate::play_pause::{self, PauseError, TriggerDetail};

/// What `pauseWorldPlay` did.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PauseOutcome")]
pub struct GraphQLPauseOutcome {
    /// The world's active pause: the one just made, or the one already there.
    pub pause: GraphQLPlayPause,
    /// True when the world was already paused, and these grounds were added
    /// to that pause as an `OPERATOR` trigger (FR-036).
    pub already_paused: bool,
}

#[derive(Default)]
pub struct PlayPauseMutation;

#[Object]
impl PlayPauseMutation {
    /// Pause `worldId`'s play, on `grounds`.
    ///
    /// Refuses blank grounds with `GROUNDS_REQUIRED` and a world that does not
    /// exist with `WORLD_NOT_FOUND`. Nothing else is needed after it returns:
    /// the event it records moves honest clients within a moment, and every
    /// open stream into the world ends on its next tick.
    async fn pause_world_play(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        grounds: String,
    ) -> GraphQLResult<GraphQLPauseOutcome> {
        let operator = admin_user(ctx)?.user_id;
        let mut conn = app_state(ctx)?
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            let outcome = play_pause::pause_world(
                &mut conn,
                operator,
                world_id,
                &grounds,
                TriggerDetail::operator(),
            )?;
            let pause = play_pauses_for_graphql(&mut conn, vec![outcome.pause])
                .map_err(PauseError::from)?
                .pop()
                .ok_or_else(|| PauseError::Database("the pause did not read back".into()))?;
            Ok(GraphQLPauseOutcome {
                pause,
                already_paused: outcome.already_paused,
            })
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
        .map_err(|e: PauseError| e.into())
    }
}

#[cfg(test)]
#[path = "mutations_play_pause_tests.rs"]
mod mutations_play_pause_tests;
