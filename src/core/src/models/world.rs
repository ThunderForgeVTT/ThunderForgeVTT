//! World, token, and game event models

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

/// A virtual tabletop game world
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct World {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

/// A game token (piece, character, etc.) within a world
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldToken {
    // Base data (always transmitted)
    pub id: String,
    pub world_id: Uuid,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub label: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub schema_version: i32, // For future migrations

    // Derived data (calculated locally, never transmitted)
    #[serde(skip)]
    pub health_percentage: Option<f32>,
    #[serde(skip)]
    pub is_alive: bool,
}

impl WorldToken {
    /// Calculate derived data from base fields
    /// Called after receiving from server or loading from cache
    pub fn prepare_derived_data(&mut self) {
        if let (Some(health), Some(max_health)) = (self.health, self.max_health) {
            self.health_percentage = Some((health as f32 / max_health as f32) * 100.0);
            self.is_alive = health > 0;
        } else {
            self.health_percentage = None;
            self.is_alive = true; // Assume alive if no health tracking
        }
    }
}

/// A recorded game event (delta for audit trail and replay)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEvent {
    pub id: i64,
    pub world_id: Uuid,
    pub event_code: i32,
    pub token_event: Option<JsonValue>, // Flexible JSON payload
    pub created_at: chrono::NaiveDateTime,
    pub schema_version: i32, // Track schema version of payload
}

impl WorldEvent {
    /// Migrate legacy event payloads to current schema
    /// 
    /// # Arguments
    /// * `raw_event` - Serialized JSON payload from database
    /// * `from_version` - Schema version of the payload
    ///
    /// # Returns
    /// Updated WorldEvent with payload migrated to current schema
    pub fn migrate_data(mut self, from_version: i32) -> Self {
        if from_version < CURRENT_EVENT_SCHEMA_VERSION {
            if let Some(ref mut payload) = self.token_event {
                // Version 1 → 2: Convert flat "level" to nested "progress.level"
                if from_version <= 1 {
                    if let Some(level) = payload.get("level") {
                        payload["progress"] = serde_json::json!({
                            "level": level.clone()
                        });
                        payload.as_object_mut().map(|m| m.remove("level"));
                    }
                }

                // Version 2 → 3: Rename "token_position" → "position"
                if from_version <= 2 {
                    if let Some(pos) = payload.get("token_position") {
                        payload["position"] = pos.clone();
                        payload.as_object_mut().map(|m| m.remove("token_position"));
                    }
                }

                // Add more migrations as schema evolves
            }
            self.schema_version = CURRENT_EVENT_SCHEMA_VERSION;
        }
        self
    }
}

/// Current schema version for WorldEvent payloads
pub const CURRENT_EVENT_SCHEMA_VERSION: i32 = 3;

/// Game event codes (event types)
#[repr(i32)]
pub enum WorldEventCode {
    TokenCreated = 1,
    TokenMoved = 2,
    TokenUpdated = 3,
    TokenDeleted = 4,
    EffectApplied = 10,
    CombatStarted = 20,
    CombatEnded = 21,
    TurnChanged = 22,
}

impl From<WorldEventCode> for i32 {
    fn from(code: WorldEventCode) -> Self {
        code as i32
    }
}

/// Mutation result for GraphQL responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutationResult<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> MutationResult<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(error.into()),
        }
    }
}
