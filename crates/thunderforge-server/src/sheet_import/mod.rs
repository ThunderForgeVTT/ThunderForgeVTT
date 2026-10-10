//! Bringing a character in from a sheet (spec 048).
//!
//! The neutral engine is `thunderforge-sheet-import`; a system's reader and
//! refine hook arrive through its [`SystemContribution`]. This module joins
//! them to an actor, the world's content and storage.
//!
//! [`SystemContribution`]: thunderforge_canvas_core::system_contribution::SystemContribution

pub mod account;
pub mod apply;
pub mod error;
pub mod index;
pub mod mapping;
pub mod preview;
pub mod records;
pub mod rollback;
pub mod route;
pub mod snapshot;
pub mod staged_links;
pub mod storage;

use diesel::prelude::*;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::schema::{actor_imports, brought_characters, sheet_import_versions};

/// Whether an `actor_imports` row applied a sheet or undid one.
#[derive(DbEnum, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[ExistingTypePath = "crate::schema::sql_types::ActorImportKind"]
pub enum ActorImportKind {
    Import,
    Rollback,
}

/// A character a player brought, held by their account rather than a world
/// (FR-030). Its versions are the files and readings, oldest first.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = brought_characters)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct BroughtCharacter {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub system_id: String,
    pub name: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = brought_characters)]
pub struct NewBroughtCharacter<'a> {
    pub owner_user_id: Uuid,
    pub system_id: &'a str,
    pub name: &'a str,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

/// One uploaded file and the server's own reading of it.
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = sheet_import_versions)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct SheetImportVersion {
    pub id: Uuid,
    pub character_id: Uuid,
    pub version_no: i32,
    pub file_key: String,
    pub file_sha256: String,
    pub file_bytes: i32,
    pub file_pages: i16,
    pub reader_id: String,
    pub reader_version: String,
    pub reading: Value,
    pub corrections: Value,
    pub created_at: chrono::NaiveDateTime,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = sheet_import_versions)]
pub struct NewSheetImportVersion<'a> {
    pub character_id: Uuid,
    pub version_no: i32,
    pub file_key: &'a str,
    pub file_sha256: &'a str,
    pub file_bytes: i32,
    pub file_pages: i16,
    pub reader_id: &'a str,
    pub reader_version: &'a str,
    pub reading: &'a Value,
    pub corrections: &'a Value,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

/// One import applied to an actor, or one rollback of it, with the actor as
/// it was before (FR-040).
#[derive(Queryable, Selectable, Debug, Clone)]
#[diesel(table_name = actor_imports)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct ActorImport {
    pub id: Uuid,
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub version_id: Option<Uuid>,
    pub kind: ActorImportKind,
    pub restored_from: Option<Uuid>,
    pub before_snapshot: Value,
    pub written: Value,
    pub plan_hash: Option<String>,
    pub applied_at: chrono::NaiveDateTime,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

#[derive(Insertable, Debug, Clone)]
#[diesel(table_name = actor_imports)]
pub struct NewActorImport<'a> {
    pub world_id: Uuid,
    pub actor_id: Uuid,
    pub version_id: Option<Uuid>,
    pub kind: ActorImportKind,
    pub restored_from: Option<Uuid>,
    pub before_snapshot: &'a Value,
    pub written: &'a Value,
    pub plan_hash: Option<&'a str>,
    pub created_by: Uuid,
    pub updated_by: Uuid,
}

#[cfg(test)]
#[path = "models_tests.rs"]
mod models_tests;
