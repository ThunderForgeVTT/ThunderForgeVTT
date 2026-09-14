//! Spec 051: an operator pauses a world's play (contracts/graphql.md).
//!
//! Every field here is `admin_user`-guarded and none calls the pause gate: an
//! operator acting on a paused world is the point, not a case to refuse
//! (contracts/live-play-lock.md, the `OPERATOR` table). The work is
//! `play_pause`'s; this file is its wire.
//!
//! A world's Owner is refused here as any non-operator is (FR-006, FR-040).
//! No world role reaches this file, which is ADR-100 decision 1.

use async_graphql::{Context, Enum, Object, SimpleObject};
use uuid::Uuid;

use crate::graphql::queries::play_pause::{
    GraphQLPauseRequest, GraphQLPlayPause, pause_requests_for_graphql, play_pauses_for_graphql,
};
use crate::graphql::{Error, GraphQLResult, admin_user, app_state};
use crate::play_pause::requests::{self, Decision};
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

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "PauseDecision")]
pub enum GraphQLPauseDecision {
    Approve,
    Decline,
}

/// What `decidePlayPauseRequest` did.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "PauseDecisionOutcome")]
pub struct GraphQLPauseDecisionOutcome {
    /// The request as decided, here or by the operator who got there first.
    pub request: GraphQLPauseRequest,
    /// False when another operator decided first (FR-035); `request` then
    /// carries their decision, their name and when.
    pub decided_here: bool,
    /// The pause, when the request was approved, here or by the other
    /// operator.
    pub pause: Option<GraphQLPlayPause>,
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

    /// Approve or decline a request to pause a world, with a `note`.
    ///
    /// Approving pauses the world exactly as `pauseWorldPlay` does, the note
    /// as its grounds. Declining changes nothing any member of the world can
    /// see (FR-034). Refuses a blank note with `GROUNDS_REQUIRED` and an
    /// unknown request with `REQUEST_NOT_FOUND`. A request somebody already
    /// decided is not an error: `decidedHere` is false and the request says
    /// who decided it, and how (FR-035).
    async fn decide_play_pause_request(
        &self,
        ctx: &Context<'_>,
        request_id: Uuid,
        decision: GraphQLPauseDecision,
        note: String,
    ) -> GraphQLResult<GraphQLPauseDecisionOutcome> {
        let operator = admin_user(ctx)?.user_id;
        let mut conn = app_state(ctx)?
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let decision = match decision {
            GraphQLPauseDecision::Approve => Decision::Approve,
            GraphQLPauseDecision::Decline => Decision::Decline,
        };

        tokio::task::spawn_blocking(move || {
            let outcome = requests::decide(&mut conn, operator, request_id, decision, &note)?;
            let request = pause_requests_for_graphql(&mut conn, vec![outcome.request])
                .map_err(PauseError::from)?
                .pop()
                .ok_or_else(|| PauseError::Database("the request did not read back".into()))?;
            let pause = match outcome.pause {
                Some(pause) => play_pauses_for_graphql(&mut conn, vec![pause])
                    .map_err(PauseError::from)?
                    .pop(),
                None => None,
            };
            Ok(GraphQLPauseDecisionOutcome {
                request,
                decided_here: outcome.decided_here,
                pause,
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
