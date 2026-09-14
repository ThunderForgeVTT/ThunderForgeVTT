//! Spec 051, ADR-100: an operator can pause a world's play.
//!
//! A pause is a site-level lock on one world's live play. Only an operator
//! creates or lifts one, and no world role — the Owner's included — can undo
//! it. This module owns the record and the lock; the resolvers that expose
//! them to operators, and the calls to [`gate`] at every world-scoped entry
//! point, live beside the resolvers they guard.
//!
//! # Kept apart from `moderation`
//!
//! A pause and a takedown are independent (ADR-100 decision 6). Lifting a
//! pause restores play, not content, and restoring content does not lift a
//! pause. Nothing in this module names a moderation table in a write, and
//! nothing in `moderation` imports this module on restoration, so the rule can
//! be checked by reading imports rather than by trusting discipline.
//!
//! # Races are settled by the database
//!
//! Two operators pausing one world at once both reach the partial unique index
//! `world_play_pauses_one_active`. One inserts; the other's insert does
//! nothing, and its grounds are attached to the winner as an `Operator`
//! trigger (FR-036). Two operators lifting one pause both issue the same
//! conditional update; one gets the row back, and the other is told who lifted
//! it and when. Neither case is a check followed by a write.
//!
//! # What a member can learn
//!
//! *That and when.* The event a pause records carries `pausedAt` only, and
//! [`gate::PlayPaused`] has no field for anything else. Grounds, names and
//! triggers are read by operator-only queries and by nothing a member reaches.

pub mod gate;
pub mod models;

use async_graphql::{Error, ErrorExtensions as _};
use chrono::NaiveDateTime;
use diesel::prelude::*;
use uuid::Uuid;

use crate::schema::{users, world_play_pause_triggers, world_play_pauses, worlds};
use crate::world_events::{EVENT_CODE_WORLD_PLAY_PAUSED, record_world_event};
use models::{NewPauseTrigger, NewPlayPause, PauseTriggerKind, PlayPause};

/// Why a pause or a lift was refused. Each carries the extension code the
/// GraphQL contract fixes.
///
/// Not `Display`, deliberately: async-graphql converts anything `Display` into
/// an `Error` by its message alone, which is exactly how a code gets lost on
/// the way out. The only conversion is the one below that sets it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseError {
    /// FR-004: a pause, and a lift, says why.
    GroundsRequired,
    WorldNotFound,
    PauseNotFound,
    /// Another operator lifted it first. Says who and when, so the second
    /// operator is not left wondering whether their lift did anything.
    AlreadyLifted {
        lifted_by: Uuid,
        lifted_by_name: String,
        lifted_at: NaiveDateTime,
    },
    Database(String),
}

impl PauseError {
    pub fn message(&self) -> String {
        match self {
            PauseError::GroundsRequired => "Grounds are required.".to_string(),
            PauseError::WorldNotFound => "No such world.".to_string(),
            PauseError::PauseNotFound => "No such pause.".to_string(),
            PauseError::AlreadyLifted { lifted_by_name, .. } => {
                format!("This pause was already lifted by {lifted_by_name}.")
            }
            PauseError::Database(detail) => format!("database error: {detail}"),
        }
    }
}

impl From<diesel::result::Error> for PauseError {
    fn from(e: diesel::result::Error) -> Self {
        PauseError::Database(e.to_string())
    }
}

impl From<Error> for PauseError {
    fn from(e: Error) -> Self {
        PauseError::Database(e.message)
    }
}

impl From<PauseError> for Error {
    fn from(e: PauseError) -> Self {
        let message = e.message();
        match e {
            PauseError::GroundsRequired => {
                Error::new(message).extend_with(|_, ext| ext.set("code", "GROUNDS_REQUIRED"))
            }
            PauseError::WorldNotFound => {
                Error::new(message).extend_with(|_, ext| ext.set("code", "WORLD_NOT_FOUND"))
            }
            PauseError::PauseNotFound => {
                Error::new(message).extend_with(|_, ext| ext.set("code", "PAUSE_NOT_FOUND"))
            }
            PauseError::AlreadyLifted {
                lifted_by,
                lifted_by_name,
                lifted_at,
            } => Error::new(message).extend_with(|_, ext| {
                ext.set("code", "PAUSE_ALREADY_LIFTED");
                ext.set(
                    "liftedBy",
                    async_graphql::indexmap::IndexMap::from([
                        (
                            async_graphql::Name::new("id"),
                            async_graphql::Value::from(lifted_by.to_string()),
                        ),
                        (
                            async_graphql::Name::new("name"),
                            async_graphql::Value::from(lifted_by_name.clone()),
                        ),
                    ]),
                );
                ext.set("liftedAt", lifted_at.and_utc().to_rfc3339());
            }),
            PauseError::Database(_) => Error::new(message),
        }
    }
}

/// What led to a pause, as the caller describes it.
///
/// `Operator` for a pause an operator asked for directly, which is every pause
/// until requests arrive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerDetail {
    pub kind: PauseTriggerKind,
    pub moderation_action_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub note: Option<String>,
}

impl TriggerDetail {
    pub fn operator() -> Self {
        TriggerDetail {
            kind: PauseTriggerKind::Operator,
            moderation_action_id: None,
            entity_type: None,
            entity_id: None,
            note: None,
        }
    }
}

impl Default for TriggerDetail {
    fn default() -> Self {
        Self::operator()
    }
}

/// What [`pause_world`] did.
#[derive(Debug, Clone)]
pub struct PauseOutcome {
    /// The world's active pause: the one just made, or the one already there.
    pub pause: PlayPause,
    /// FR-036: the world was already paused, and this call's grounds were
    /// added to that pause as a trigger rather than making a second one.
    pub already_paused: bool,
}

/// Grounds with their surrounding whitespace removed, or refused if nothing
/// is left. The table's CHECK says the same; saying it here first gives the
/// operator a code rather than a constraint name.
fn grounds_of(grounds: &str) -> Result<&str, PauseError> {
    let trimmed = grounds.trim();
    if trimmed.is_empty() {
        Err(PauseError::GroundsRequired)
    } else {
        Ok(trimmed)
    }
}

/// The name an operator is recorded under. A snapshot: the record keeps it
/// when the account is renamed or gone.
fn operator_name(conn: &mut PgConnection, operator: Uuid) -> Result<String, PauseError> {
    users::table
        .filter(users::id.eq(operator))
        .select(users::username)
        .first(conn)
        .optional()?
        .ok_or_else(|| PauseError::Database(format!("operator {operator} has no account")))
}

fn attach_trigger(
    conn: &mut PgConnection,
    operator: Uuid,
    pause_id: Uuid,
    trigger: &TriggerDetail,
    note: Option<&str>,
) -> Result<(), PauseError> {
    diesel::insert_into(world_play_pause_triggers::table)
        .values(NewPauseTrigger {
            id: Uuid::now_v7(),
            request_id: None,
            pause_id: Some(pause_id),
            kind: trigger.kind,
            moderation_action_id: trigger.moderation_action_id,
            entity_type: trigger.entity_type.as_deref(),
            entity_id: trigger.entity_id,
            note,
            created_by: Some(operator),
        })
        .execute(conn)?;
    Ok(())
}

/// Pause `world_id`'s play, on `grounds`, as `operator`.
///
/// One transaction: the pause, its trigger and world event code 28, or none of
/// them. The event is the fast path for honest clients (research R1) and
/// carries `pausedAt` and nothing else. What makes the pause hold is the gate
/// and the stream poll, which read the row this writes.
///
/// A world that is already paused is not an error (FR-036). The call's grounds
/// are attached to the active pause as a trigger — its `note`, unless the
/// trigger already has one — and the outcome says `already_paused`.
///
/// The caller is responsible for `operator` being an operator. This function
/// takes an id rather than a context so the approval path and the tests reach
/// the same code the mutation does.
pub fn pause_world(
    conn: &mut PgConnection,
    operator: Uuid,
    world_id: Uuid,
    grounds: &str,
    trigger: TriggerDetail,
) -> Result<PauseOutcome, PauseError> {
    let grounds = grounds_of(grounds)?;

    conn.transaction(|conn| {
        let world_name: String = worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::name)
            .first(conn)
            .optional()?
            .ok_or(PauseError::WorldNotFound)?;
        let operator_name = operator_name(conn, operator)?;

        // Twice at most. An insert that loses to an active pause finds that
        // pause on the next statement — unless it was lifted in the moment
        // between, in which case the world is not paused and the second insert
        // will stand.
        for _ in 0..2 {
            let inserted: Option<PlayPause> = diesel::insert_into(world_play_pauses::table)
                .values(NewPlayPause {
                    id: Uuid::now_v7(),
                    world_id,
                    world_name: &world_name,
                    paused_by: operator,
                    paused_by_name: &operator_name,
                    grounds,
                    request_id: None,
                    created_by: operator,
                    updated_by: operator,
                })
                // The only unique constraint a fresh v7 id can meet is
                // `world_play_pauses_one_active`.
                .on_conflict_do_nothing()
                .returning(PlayPause::as_returning())
                .get_result(conn)
                .optional()?;

            if let Some(pause) = inserted {
                attach_trigger(conn, operator, pause.id, &trigger, trigger.note.as_deref())?;
                record_world_event(
                    conn,
                    world_id,
                    EVENT_CODE_WORLD_PLAY_PAUSED,
                    Some(serde_json::json!({
                        "pausedAt": pause.paused_at.and_utc().to_rfc3339(),
                    })),
                    operator,
                )?;
                return Ok(PauseOutcome {
                    pause,
                    already_paused: false,
                });
            }

            let existing: Option<PlayPause> = world_play_pauses::table
                .filter(world_play_pauses::world_id.eq(world_id))
                .filter(world_play_pauses::lifted_at.is_null())
                .select(PlayPause::as_select())
                .first(conn)
                .optional()?;

            if let Some(pause) = existing {
                let note = trigger.note.as_deref().unwrap_or(grounds);
                attach_trigger(conn, operator, pause.id, &trigger, Some(note))?;
                return Ok(PauseOutcome {
                    pause,
                    already_paused: true,
                });
            }
        }

        Err(PauseError::Database(format!(
            "world {world_id} was neither paused nor pausable; another operator is pausing and lifting it at once"
        )))
    })
}

/// Lift `pause_id`, on `grounds`, as `operator`.
///
/// A single conditional update: it lifts a pause that is active, and does
/// nothing to one that is not. Zero rows back is either a pause somebody else
/// already lifted — reported with who and when — or no pause at all.
///
/// Sets the lift columns and nothing else. No content, membership or
/// moderation row is touched (FR-041, FR-042): a scene taken down while the
/// world was paused stays taken down.
pub fn lift_pause(
    conn: &mut PgConnection,
    operator: Uuid,
    pause_id: Uuid,
    grounds: &str,
) -> Result<PlayPause, PauseError> {
    let grounds = grounds_of(grounds)?;
    let operator_name = operator_name(conn, operator)?;
    let now = chrono::Utc::now().naive_utc();

    let lifted: Option<PlayPause> = diesel::update(
        world_play_pauses::table
            .filter(world_play_pauses::id.eq(pause_id))
            .filter(world_play_pauses::lifted_at.is_null()),
    )
    .set((
        world_play_pauses::lifted_by.eq(operator),
        world_play_pauses::lifted_by_name.eq(&operator_name),
        world_play_pauses::lifted_at.eq(now),
        world_play_pauses::lift_grounds.eq(grounds),
        world_play_pauses::updated_by.eq(operator),
        world_play_pauses::updated_at.eq(now),
    ))
    .returning(PlayPause::as_returning())
    .get_result(conn)
    .optional()?;

    if let Some(pause) = lifted {
        return Ok(pause);
    }

    let existing: Option<PlayPause> = world_play_pauses::table
        .filter(world_play_pauses::id.eq(pause_id))
        .select(PlayPause::as_select())
        .first(conn)
        .optional()?;

    match existing {
        Some(PlayPause {
            lifted_by: Some(lifted_by),
            lifted_by_name: Some(lifted_by_name),
            lifted_at: Some(lifted_at),
            ..
        }) => Err(PauseError::AlreadyLifted {
            lifted_by,
            lifted_by_name,
            lifted_at,
        }),
        // Lifted columns are all-or-nothing by CHECK, so a row here is lifted;
        // an unlifted one would have been updated above.
        Some(_) => Err(PauseError::Database(format!(
            "pause {pause_id} is neither active nor fully lifted"
        ))),
        None => Err(PauseError::PauseNotFound),
    }
}

/// Serialises the tests in this module against `migration_tests`, whose
/// `down.sql` cycle takes exclusive locks on every table here. Not a
/// correctness lock for production code; there is none.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
#[path = "migration_tests.rs"]
mod migration_tests;

#[cfg(test)]
#[path = "gate_tests.rs"]
mod gate_tests;

#[cfg(test)]
#[path = "pause_tests.rs"]
mod pause_tests;
