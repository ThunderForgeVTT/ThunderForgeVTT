use crate::schema::{policies, world_events, worlds};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Insertable)]
#[diesel(table_name = worlds)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct World {
    pub id: uuid::Uuid,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

use crate::db_types::PolicyEffectEnum;

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = policies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Policy {
    pub id: uuid::Uuid,
    pub effect: PolicyEffectEnum,
    pub resources: Vec<Option<String>>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TokenEvent {
    id: String,
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

#[derive(Queryable, Selectable, Insertable, Debug)]
#[diesel(table_name = world_events)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct WorldEvent {
    pub id: i64,
    pub world_id: uuid::Uuid,
    pub event_code: i32,
    pub token_event: Option<serde_json::Value>,
    pub created_at: chrono::NaiveDateTime,
}
