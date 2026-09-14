//! The four tables, as they are stored (data-model.md).

use chrono::NaiveDateTime;
use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::{
    world_live_play, world_play_pause_requests, world_play_pause_triggers, world_play_pauses,
};

/// Where a request stands (FR-032 to FR-035).
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::PauseRequestState"]
// The migration writes PascalCase, for the reason `ContentOrigin` records:
// without this every insert fails at the database, invisibly to the compiler.
#[DbValueStyle = "PascalCase"]
pub enum PauseRequestState {
    /// Waiting for an operator. At most one per world.
    Pending,
    /// An operator paused the world on it.
    Approved,
    /// An operator decided not to. Never shown to the world's members.
    Declined,
}

/// What led to a request or a pause (FR-037).
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::PauseTriggerKind"]
#[DbValueStyle = "PascalCase"]
pub enum PauseTriggerKind {
    /// A takedown on content of a world in live play. Names its moderation
    /// action.
    Takedown,
    /// An operator acting directly, including a second operator pausing a
    /// world already paused (FR-036).
    Operator,
    /// Reserved for an abuse intake. Nothing raises it yet.
    AbuseReport,
}

/// A world's live play stopped by an operator. Active while `lifted_at` is
/// `None`.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = world_play_pauses)]
pub struct PlayPause {
    pub id: Uuid,
    pub world_id: Uuid,
    /// The world's name when it was paused. Kept when the world is not.
    pub world_name: String,
    pub paused_by: Uuid,
    pub paused_by_name: String,
    pub paused_at: NaiveDateTime,
    /// Operator-only. No member-facing read selects this column.
    pub grounds: String,
    pub request_id: Option<Uuid>,
    pub lifted_by: Option<Uuid>,
    pub lifted_by_name: Option<String>,
    pub lifted_at: Option<NaiveDateTime>,
    pub lift_grounds: Option<String>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
    pub updated_at: NaiveDateTime,
}

impl PlayPause {
    pub fn is_active(&self) -> bool {
        self.lifted_at.is_none()
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = world_play_pauses)]
pub struct NewPlayPause<'a> {
    pub id: Uuid,
    pub world_id: Uuid,
    pub world_name: &'a str,
    pub paused_by: Uuid,
    pub paused_by_name: &'a str,
    pub grounds: &'a str,
    pub request_id: Option<Uuid>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

/// A proposal to pause a world, waiting for an operator.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = world_play_pause_requests)]
pub struct PauseRequest {
    pub id: Uuid,
    pub world_id: Uuid,
    pub world_name: String,
    pub raised_at: NaiveDateTime,
    pub state: PauseRequestState,
    pub decided_by: Option<Uuid>,
    pub decided_by_name: Option<String>,
    pub decided_at: Option<NaiveDateTime>,
    pub decision_note: Option<String>,
    /// `None` when the system raised it.
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub updated_at: NaiveDateTime,
}

/// What led to a request or a pause. Belongs to exactly one of them.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = world_play_pause_triggers)]
pub struct PauseTrigger {
    pub id: Uuid,
    pub request_id: Option<Uuid>,
    pub pause_id: Option<Uuid>,
    pub kind: PauseTriggerKind,
    pub moderation_action_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub note: Option<String>,
    pub recorded_at: NaiveDateTime,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Insertable)]
#[diesel(table_name = world_play_pause_triggers)]
pub struct NewPauseTrigger<'a> {
    pub id: Uuid,
    pub request_id: Option<Uuid>,
    pub pause_id: Option<Uuid>,
    pub kind: PauseTriggerKind,
    pub moderation_action_id: Option<Uuid>,
    pub entity_type: Option<&'a str>,
    pub entity_id: Option<Uuid>,
    pub note: Option<&'a str>,
    pub created_by: Option<Uuid>,
}

/// Whether a world has been played in the last minute (research R3).
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = world_live_play)]
pub struct WorldLivePlay {
    pub world_id: Uuid,
    pub last_beat_at: NaiveDateTime,
}
