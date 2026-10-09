//! Content read from a sheet that the world does not have yet (spec 048
//! FR-033).
//!
//! A staged piece is uploaded by origin and waits for the GM or a Trusted
//! Player to adopt or decline it. Until it is adopted, the actor that brought
//! it may show it but not use it; each attempt to use it is recorded for the
//! GM (FR-034).

use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::compendium::origin::ContentOrigin;
use crate::schema::{world_staged_content, world_unadopted_use_attempts};

/// Where a staged piece stands. `Adopted` is final in spec 048.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::StagedState"]
pub enum StagedState {
    Pending,
    Adopted,
    Declined,
}

#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = world_staged_content)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct StagedContent {
    pub id: Uuid,
    pub world_id: Uuid,
    pub player_user_id: Uuid,
    /// A vocabulary type, or `item`.
    pub kind: String,
    pub name: String,
    pub normalized_name: String,
    pub content_hash: String,
    pub field_values: Value,
    pub origin: ContentOrigin,
    pub state: StagedState,
    pub differs_from: Option<Uuid>,
    pub first_actor_id: Option<Uuid>,
    pub adopted_ability_id: Option<Uuid>,
    pub adopted_item_id: Option<Uuid>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

/// A new staged piece. Its origin is not a field: staged content is uploaded
/// by definition, and the table refuses anything else.
#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = world_staged_content)]
pub struct NewStagedContent<'a> {
    pub world_id: Uuid,
    pub player_user_id: Uuid,
    pub kind: &'a str,
    pub name: &'a str,
    pub normalized_name: &'a str,
    pub content_hash: &'a str,
    pub field_values: &'a Value,
    pub origin: ContentOrigin,
    pub differs_from: Option<Uuid>,
    pub first_actor_id: Option<Uuid>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

/// Attempts to use one staged piece through one actor, within a window.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = world_unadopted_use_attempts)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct UnadoptedUseAttempt {
    pub id: Uuid,
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub staged_id: Uuid,
    pub user_id: Uuid,
    pub operation: String,
    pub first_at: chrono::NaiveDateTime,
    pub last_at: chrono::NaiveDateTime,
    pub attempts: i32,
    pub reported: bool,
    pub chat_message_id: Option<Uuid>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = world_unadopted_use_attempts)]
pub struct NewUnadoptedUseAttempt<'a> {
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub staged_id: Uuid,
    pub user_id: Uuid,
    pub operation: &'a str,
    pub reported: bool,
    pub chat_message_id: Option<Uuid>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}
