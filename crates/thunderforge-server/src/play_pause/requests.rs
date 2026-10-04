//! Requests to pause a world, and an operator's decision on one (research R4).
//!
//! A request pauses nothing (FR-032). It gathers what led to it — every
//! trigger raised for its world while it waits — and an operator approves it,
//! which pauses the world as a direct pause would, or declines it, which
//! nobody in the world can see.
//!
//! # Races, settled by the database
//!
//! - **Raising.** `world_play_pause_requests_one_pending` allows one pending
//!   request per world. A raise inserts one and does nothing on conflict, then
//!   attaches its trigger to whichever pending request is there (FR-033). The
//!   trigger's own unique constraints make a retried raise record once.
//! - **Deciding.** A single `UPDATE … WHERE id = $1 AND state = 'Pending'`.
//!   The second operator's update waits on the first's row lock, finds the row
//!   no longer pending, and changes nothing; they read back who decided
//!   (FR-035). Approval pauses in the same transaction, so a request is never
//!   approved without a pause, nor a pause made for a request left pending.
//!
//! # Generic on purpose
//!
//! [`raise`] takes any [`TriggerDetail`], so an abuse intake can raise requests
//! the day one exists (FR-037). Only [`raise_for_takedown`] knows about
//! takedowns, and only in that it asks first whether anyone is playing.

use chrono::Utc;
use diesel::prelude::*;
use uuid::Uuid;

use super::live_play::in_live_play;
use super::models::{
    NewPauseTrigger, PauseRequest, PauseRequestState, PauseTrigger, PauseTriggerKind, PlayPause,
};
use super::{PauseError, PauseOrigin, TriggerDetail, grounds_of, operator_name, pause_in};
use crate::models::ContentModerationAction;
use crate::moderation::action_type;
use crate::moderation::reach::DisabledCopy;
use crate::schema::{
    world_play_pause_requests, world_play_pause_triggers, world_play_pauses, worlds,
};
use crate::state::AppState;

/// What [`raise`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaiseOutcome {
    /// The world is not in live play, so nothing was raised (US3 AS4).
    NotLive,
    /// The trigger is on this pending request — new, or already waiting.
    Requested { request_id: Uuid },
    /// The world was already paused; the trigger is on that pause, and no
    /// request was made (FR-036).
    AddedToPause { pause_id: Uuid },
}

fn active_pause_id(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<Option<Uuid>> {
    world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(world_id))
        .filter(world_play_pauses::lifted_at.is_null())
        .select(world_play_pauses::id)
        .first(conn)
        .optional()
}

/// Record `trigger` on a request or a pause. Idempotent per moderation action
/// and owner: the conflict is `world_play_pause_triggers_once_per_*`.
fn insert_trigger(
    conn: &mut PgConnection,
    request_id: Option<Uuid>,
    pause_id: Option<Uuid>,
    trigger: &TriggerDetail,
) -> QueryResult<()> {
    diesel::insert_into(world_play_pause_triggers::table)
        .values(NewPauseTrigger {
            id: Uuid::now_v7(),
            request_id,
            pause_id,
            kind: trigger.kind,
            moderation_action_id: trigger.moderation_action_id,
            entity_type: trigger.entity_type.as_deref(),
            entity_id: trigger.entity_id,
            note: trigger.note.as_deref(),
            // Raised by the system: a takedown's claimant is nobody's account.
            created_by: None,
        })
        .on_conflict_do_nothing()
        .execute(conn)
        .map(|_| ())
}

/// Ask for `world_id`'s play to be paused, because of `trigger`.
///
/// A paused world takes the trigger onto its pause (FR-036). Otherwise the
/// world's pending request takes it, made first if there is none (FR-033).
/// Never pauses anything (FR-032).
pub fn raise(
    conn: &mut PgConnection,
    world_id: Uuid,
    trigger: &TriggerDetail,
) -> Result<RaiseOutcome, PauseError> {
    conn.transaction(|conn| {
        let world_name: String = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::name)
            .first(conn)
            .optional()?
            .ok_or(PauseError::WorldNotFound)?;

        // Twice at most, as `pause_in` does: a pending request decided in the
        // moment between the insert and the read leaves either a pause to
        // attach to or room for a new request on the second pass.
        for _ in 0..2 {
            if let Some(pause_id) = active_pause_id(conn, world_id)? {
                insert_trigger(conn, None, Some(pause_id), trigger)?;
                return Ok(RaiseOutcome::AddedToPause { pause_id });
            }

            diesel::insert_into(world_play_pause_requests::table)
                .values((
                    world_play_pause_requests::id.eq(Uuid::now_v7()),
                    world_play_pause_requests::world_id.eq(world_id),
                    world_play_pause_requests::world_name.eq(&world_name),
                ))
                // The only unique constraint a fresh v7 id can meet is
                // `world_play_pause_requests_one_pending`.
                .on_conflict_do_nothing()
                .execute(conn)?;

            let pending: Option<Uuid> = world_play_pause_requests::table
                .filter(world_play_pause_requests::world_id.eq(world_id))
                .filter(world_play_pause_requests::state.eq(PauseRequestState::Pending))
                .select(world_play_pause_requests::id)
                .first(conn)
                .optional()?;

            if let Some(request_id) = pending {
                insert_trigger(conn, Some(request_id), None, trigger)?;
                return Ok(RaiseOutcome::Requested { request_id });
            }
        }

        Err(PauseError::Database(format!(
            "world {world_id} had neither a pause nor a pending request to take a trigger; \
             operators are deciding it as it is raised"
        )))
    })
}

/// One world a takedown reached, and the moderation action that reached it:
/// the notice's `content_disabled` for the target's own world, or a child
/// case's `content_disabled_as_copy` for a copy's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakedownReach {
    pub world_id: Uuid,
    pub moderation_action_id: Uuid,
    pub entity_type: String,
    pub entity_id: Uuid,
}

/// Worlds whose raise fails, so a test can prove a failing hook leaves the
/// takedown standing. Per world, so tests running beside it are untouched.
#[cfg(test)]
pub(crate) static FAIL_RAISES_FOR: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashSet<Uuid>>,
> = std::sync::LazyLock::new(Default::default);

/// Raise for a takedown on content of `reach.world_id`, if that world is in
/// live play (FR-030, US3 AS4). A takedown on content nobody is playing needs
/// no pause: the takedown alone withholds it from the next load.
pub fn raise_for_takedown(
    conn: &mut PgConnection,
    reach: &TakedownReach,
) -> Result<RaiseOutcome, PauseError> {
    #[cfg(test)]
    if FAIL_RAISES_FOR
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .contains(&reach.world_id)
    {
        return Err(PauseError::Database("forced by a test".into()));
    }

    if !in_live_play(conn, reach.world_id)? {
        return Ok(RaiseOutcome::NotLive);
    }
    raise(
        conn,
        reach.world_id,
        &TriggerDetail {
            kind: PauseTriggerKind::Takedown,
            moderation_action_id: Some(reach.moderation_action_id),
            entity_type: Some(reach.entity_type.clone()),
            entity_id: Some(reach.entity_id),
            note: None,
        },
    )
}

/// [`raise_for_takedown`] for every world a takedown reached, after the
/// takedown has taken effect.
///
/// Never fails. A takedown must not be undone, or reported as failed, because
/// a request could not be written; each failure is logged at `error` with the
/// moderation action id, so an operator can raise it by hand. Returns the
/// lines it logged.
pub fn raise_for_takedowns(conn: &mut PgConnection, reached: &[TakedownReach]) -> Vec<String> {
    let mut logged = Vec::new();
    for reach in reached {
        if let Err(error) = raise_for_takedown(conn, reach) {
            let line = format!(
                "could not ask for a pause of world {} after moderation action {}: {}",
                reach.world_id,
                reach.moderation_action_id,
                error.message(),
            );
            tracing::error!(
                moderation_action_id = %reach.moderation_action_id,
                world_id = %reach.world_id,
                "{line}"
            );
            logged.push(line);
        }
    }
    logged
}

/// Spec 051 FR-030: a takedown on content of a world in live play asks an
/// operator to pause that world — the target's own, and every world a copy
/// was disabled in.
///
/// Like the lore hook, it runs after the takedown has taken effect and **it
/// cannot fail the takedown**: content already disabled must not be reported
/// as not, because a request could not be written. Each failure is logged at
/// `error` with the moderation action id, so an
/// operator can raise it by hand. Returns what was logged, for the tests.
pub async fn ask_for_pauses(
    state: &AppState,
    events: &[ContentModerationAction],
    copies: &[DisabledCopy],
) -> Vec<String> {
    let reached: Vec<TakedownReach> = events
        .iter()
        .filter(|e| e.action_type == action_type::CONTENT_DISABLED)
        .map(|e| TakedownReach {
            world_id: e.world_id,
            moderation_action_id: e.id,
            entity_type: e.entity_type.clone(),
            entity_id: e.entity_id,
        })
        .chain(copies.iter().map(|copy| TakedownReach {
            world_id: copy.world_id,
            moderation_action_id: copy.action_id,
            entity_type: copy.entity_type.clone(),
            entity_id: copy.entity_id,
        }))
        .collect();
    if reached.is_empty() {
        return Vec::new();
    }

    let mut conn = match state.db_pool.get() {
        Ok(conn) => conn,
        Err(error) => {
            let lines: Vec<String> = reached
                .iter()
                .map(|reach| {
                    format!(
                        "could not ask for a pause of world {} after moderation action {}:                          no database connection: {error}",
                        reach.world_id, reach.moderation_action_id
                    )
                })
                .collect();
            for line in &lines {
                tracing::error!("{line}");
            }
            return lines;
        }
    };
    let actions: Vec<Uuid> = reached.iter().map(|r| r.moderation_action_id).collect();
    tokio::task::spawn_blocking(move || raise_for_takedowns(&mut conn, &reached))
        .await
        .unwrap_or_else(|error| {
            let line = format!(
                "the pause request hook for moderation actions {actions:?} did not finish: {error}"
            );
            tracing::error!("{line}");
            vec![line]
        })
}

/// An operator's decision on a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Approve,
    Decline,
}

/// What [`decide`] did.
#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    /// The request as decided — here, or by whoever got there first.
    pub request: PauseRequest,
    /// False when another operator decided first (FR-035). `request` then
    /// carries their decision, name and time.
    pub decided_here: bool,
    /// The pause, when the request was approved, here or by the winner.
    pub pause: Option<PlayPause>,
}

/// The triggers a request gathered, oldest first.
pub fn triggers_of_request(
    conn: &mut PgConnection,
    request_id: Uuid,
) -> QueryResult<Vec<PauseTrigger>> {
    world_play_pause_triggers::table
        .filter(world_play_pause_triggers::request_id.eq(request_id))
        .order((
            world_play_pause_triggers::recorded_at.asc(),
            world_play_pause_triggers::id.asc(),
        ))
        .select(PauseTrigger::as_select())
        .load(conn)
}

/// The pause an approved request led to: the one naming it, or — when it was
/// approved onto a world already paused — that world's active pause.
fn pause_of_approved(
    conn: &mut PgConnection,
    request: &PauseRequest,
) -> QueryResult<Option<PlayPause>> {
    let named = world_play_pauses::table
        .filter(world_play_pauses::request_id.eq(request.id))
        .select(PlayPause::as_select())
        .first(conn)
        .optional()?;
    if named.is_some() {
        return Ok(named);
    }
    world_play_pauses::table
        .filter(world_play_pauses::world_id.eq(request.world_id))
        .filter(world_play_pauses::lifted_at.is_null())
        .select(PlayPause::as_select())
        .first(conn)
        .optional()
}

/// Decide `request_id`, as `operator`, with `note`.
///
/// **Approve** pauses the world in this transaction, the note as its grounds
/// and `request_id` set, recording event 28 as a direct pause does. A world
/// already paused takes the request's triggers onto that pause instead
/// (FR-036). **Decline** records the decision on the request and nothing
/// else: no event, and nothing a member of the world can read (FR-034).
///
/// The caller is responsible for `operator` being an operator.
pub fn decide(
    conn: &mut PgConnection,
    operator: Uuid,
    request_id: Uuid,
    decision: Decision,
    note: &str,
) -> Result<DecisionOutcome, PauseError> {
    let note = grounds_of(note)?;

    conn.transaction(|conn| {
        let operator_name = operator_name(conn, operator)?;
        let now = Utc::now().naive_utc();
        let state = match decision {
            Decision::Approve => PauseRequestState::Approved,
            Decision::Decline => PauseRequestState::Declined,
        };

        let decided: Option<PauseRequest> = diesel::update(
            world_play_pause_requests::table
                .filter(world_play_pause_requests::id.eq(request_id))
                .filter(world_play_pause_requests::state.eq(PauseRequestState::Pending)),
        )
        .set((
            world_play_pause_requests::state.eq(state),
            world_play_pause_requests::decided_by.eq(operator),
            world_play_pause_requests::decided_by_name.eq(&operator_name),
            world_play_pause_requests::decided_at.eq(now),
            world_play_pause_requests::decision_note.eq(note),
            world_play_pause_requests::updated_by.eq(operator),
            world_play_pause_requests::updated_at.eq(now),
        ))
        .returning(PauseRequest::as_returning())
        .get_result(conn)
        .optional()?;

        if let Some(request) = decided {
            let pause = match decision {
                Decision::Decline => None,
                Decision::Approve => {
                    let triggers = triggers_of_request(conn, request.id)?;
                    let outcome = pause_in(
                        conn,
                        operator,
                        request.world_id,
                        note,
                        PauseOrigin::Request {
                            id: request.id,
                            triggers: &triggers,
                        },
                    )?;
                    Some(outcome.pause)
                }
            };
            return Ok(DecisionOutcome {
                request,
                decided_here: true,
                pause,
            });
        }

        // Nothing updated: no such request, or somebody decided it first.
        let request: PauseRequest = world_play_pause_requests::table
            .filter(world_play_pause_requests::id.eq(request_id))
            .select(PauseRequest::as_select())
            .first(conn)
            .optional()?
            .ok_or(PauseError::RequestNotFound)?;
        if request.state == PauseRequestState::Pending {
            return Err(PauseError::Database(format!(
                "request {request_id} is pending and could not be decided"
            )));
        }
        let pause = match request.state {
            PauseRequestState::Approved => pause_of_approved(conn, &request)?,
            _ => None,
        };
        Ok(DecisionOutcome {
            request,
            decided_here: false,
            pause,
        })
    })
}
