use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

#[derive(DbEnum, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[ExistingTypePath = "crate::schema::sql_types::PolicyEffect"]
pub enum PolicyEffectEnum {
    Allow,
    Deny,
}

/// Spec 002: distinguishes a map-import background image from a
/// paste-to-canvas image within the shared `canvas_image_assets` table
/// (FR-018 — one storage mechanism, not two).
#[derive(DbEnum, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[ExistingTypePath = "crate::schema::sql_types::CanvasImageAssetKind"]
// diesel-derive-enum defaults to snake_case DB values; the migration
// (`CREATE TYPE "CanvasImageAssetKind" AS ENUM ('Background', 'Pasted')`)
// uses PascalCase to match this codebase's existing Postgres-enum
// convention (see `PolicyEffect`'s 'Allow'/'Deny'), so this must be
// declared explicitly or every insert/select fails at the DB with
// "invalid input value for enum" (caught by this feature's own
// integration tests, not by `cargo check` — enum string mismatches are
// invisible to the type checker).
#[DbValueStyle = "PascalCase"]
pub enum CanvasImageAssetKindEnum {
    Background,
    Pasted,
}
