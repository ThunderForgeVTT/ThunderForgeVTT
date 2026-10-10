//! Spec 048: bringing a character in from a sheet. The types the plan and
//! the history travel as, and `applySheetImport`
//! (contracts/graphql-sheet-import.md). The rules live in
//! `crate::sheet_import`; this file only converts.

use async_graphql::{Context, Enum, Json, Result as GraphQLResult, SimpleObject, Upload};
use chrono::{DateTime, Utc};
use serde_json::Value;
use thunderforge_sheet_import::plan::{CrossCheck, KeptInPlay};
use thunderforge_sheet_import::{
    ContentChange, FieldChange, ImportPlan, PlanCertainty, Resolution, Source, Unmapped, plan_hash,
};
use uuid::Uuid;

use super::{app_state, authenticated_user};
use crate::sheet_import::apply::{ApplyInput, apply_sheet_import_impl};
use crate::sheet_import::error::SheetImportError;
use crate::sheet_import::records::{ImportRecord, Person, record};

#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "FieldCertainty")]
pub enum GraphQLFieldCertainty {
    Read,
    Uncertain,
    Unread,
    Corrected,
}

impl From<PlanCertainty> for GraphQLFieldCertainty {
    fn from(certainty: PlanCertainty) -> Self {
        match certainty {
            PlanCertainty::Read => Self::Read,
            PlanCertainty::Uncertain => Self::Uncertain,
            PlanCertainty::Unread => Self::Unread,
            PlanCertainty::Corrected => Self::Corrected,
        }
    }
}

#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "ContentResolution")]
pub enum GraphQLContentResolution {
    World,
    StagedExisting,
    StagedNew,
    Differs,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetSource")]
pub struct GraphQLSheetSource {
    pub page: i32,
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub text: String,
}

impl From<&Source> for GraphQLSheetSource {
    fn from(source: &Source) -> Self {
        Self {
            page: source.page as i32,
            x0: source.rect.x0 as f64,
            y0: source.rect.y0 as f64,
            x1: source.rect.x1 as f64,
            y1: source.rect.y1 as f64,
            text: source.text.clone(),
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetFieldChange")]
pub struct GraphQLSheetFieldChange {
    pub path: String,
    pub target: String,
    pub old: Option<Json<Value>>,
    pub new: Option<Json<Value>>,
    pub certainty: GraphQLFieldCertainty,
    pub reason: Option<String>,
    pub source: Option<GraphQLSheetSource>,
    pub play_state: bool,
}

impl From<&FieldChange> for GraphQLSheetFieldChange {
    fn from(field: &FieldChange) -> Self {
        Self {
            path: field.path.clone(),
            target: field.target.clone(),
            old: field.old.clone().map(Json),
            new: field.new.clone().map(Json),
            certainty: field.certainty.into(),
            reason: field.reason.clone(),
            source: field.source.as_ref().map(Into::into),
            play_state: field.play_state,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetContentChange")]
pub struct GraphQLSheetContentChange {
    pub kind: String,
    pub name: String,
    pub resolution: GraphQLContentResolution,
    pub world_id: Option<String>,
    pub staged_id: Option<String>,
    pub removed: bool,
}

impl From<&ContentChange> for GraphQLSheetContentChange {
    fn from(change: &ContentChange) -> Self {
        let (resolution, world_id, staged_id) = match &change.resolution {
            Resolution::World { id } => (GraphQLContentResolution::World, Some(id.clone()), None),
            Resolution::StagedExisting { id } => (
                GraphQLContentResolution::StagedExisting,
                None,
                Some(id.clone()),
            ),
            Resolution::StagedNew => (GraphQLContentResolution::StagedNew, None, None),
            Resolution::Differs { id } => {
                (GraphQLContentResolution::Differs, None, Some(id.clone()))
            }
        };
        Self {
            kind: change.kind.clone(),
            name: change.name.clone(),
            resolution,
            world_id,
            staged_id,
            removed: change.removed,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetUnmapped")]
pub struct GraphQLSheetUnmapped {
    pub path: String,
    pub value: Json<Value>,
    pub goes_to: String,
}

impl From<&Unmapped> for GraphQLSheetUnmapped {
    fn from(row: &Unmapped) -> Self {
        Self {
            path: row.path.clone(),
            value: Json(row.value.clone()),
            goes_to: row.goes_to.clone(),
        }
    }
}

/// A number the sheet prints that the rules derive, where the two differ.
/// Never written; its base field is marked uncertain.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetCrossCheck")]
pub struct GraphQLSheetCrossCheck {
    pub path: String,
    pub sheet: Json<Value>,
    pub derived: Json<Value>,
}

impl From<&CrossCheck> for GraphQLSheetCrossCheck {
    fn from(check: &CrossCheck) -> Self {
        Self {
            path: check.path.clone(),
            sheet: Json(check.sheet.clone()),
            derived: Json(check.derived.clone()),
        }
    }
}

/// A value in play (hit points, slots spent) that a re-import keeps unless
/// the person names it in `overwritePlayState`.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetKeptInPlay")]
pub struct GraphQLSheetKeptInPlay {
    pub target: String,
    pub current: Option<Json<Value>>,
    pub sheet: Option<Json<Value>>,
}

impl From<&KeptInPlay> for GraphQLSheetKeptInPlay {
    fn from(kept: &KeptInPlay) -> Self {
        Self {
            target: kept.target.clone(),
            current: kept.current.clone().map(Json),
            sheet: kept.sheet.clone().map(Json),
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SheetImportPlan")]
pub struct GraphQLSheetImportPlan {
    pub reader_id: String,
    pub reader_version: String,
    pub fields: Vec<GraphQLSheetFieldChange>,
    pub content: Vec<GraphQLSheetContentChange>,
    pub unmapped: Vec<GraphQLSheetUnmapped>,
    pub cross_checks: Vec<GraphQLSheetCrossCheck>,
    pub kept_in_play: Vec<GraphQLSheetKeptInPlay>,
    pub is_reimport: bool,
    pub plan_hash: String,
}

impl From<&ImportPlan> for GraphQLSheetImportPlan {
    fn from(plan: &ImportPlan) -> Self {
        Self {
            reader_id: plan.reader_id.clone(),
            reader_version: plan.reader_version.clone(),
            fields: plan.fields.iter().map(Into::into).collect(),
            content: plan.content.iter().map(Into::into).collect(),
            unmapped: plan.unmapped.iter().map(Into::into).collect(),
            cross_checks: plan.cross_checks.iter().map(Into::into).collect(),
            kept_in_play: plan.kept_in_play.iter().map(Into::into).collect(),
            is_reimport: plan.is_reimport,
            plan_hash: plan_hash(plan),
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "UserSummary")]
pub struct GraphQLUserSummary {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
}

impl From<Person> for GraphQLUserSummary {
    fn from(person: Person) -> Self {
        Self {
            id: person.id,
            username: person.username,
            display_name: person.display_name,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ActorImportRecord")]
pub struct GraphQLActorImportRecord {
    pub id: Uuid,
    /// "import" or "rollback".
    pub kind: String,
    pub applied_at: DateTime<Utc>,
    pub applied_by: GraphQLUserSummary,
    pub version_no: Option<i32>,
    pub restored_from: Option<Uuid>,
    pub corrected_fields: Vec<String>,
    pub file_available: bool,
    /// The version whose file `GET /api/sheet-imports/{id}/file` serves.
    pub version_id: Option<Uuid>,
}

impl From<ImportRecord> for GraphQLActorImportRecord {
    fn from(record: ImportRecord) -> Self {
        let kind = match record.import.kind {
            crate::sheet_import::ActorImportKind::Import => "import",
            crate::sheet_import::ActorImportKind::Rollback => "rollback",
        };
        Self {
            id: record.import.id,
            kind: kind.to_string(),
            applied_at: record.import.applied_at.and_utc(),
            applied_by: record.applied_by.into(),
            version_no: record.version_no,
            restored_from: record.import.restored_from,
            corrected_fields: record.corrected,
            file_available: record.file_available,
            version_id: record.import.version_id,
        }
    }
}

#[derive(Default)]
pub struct SheetImportMutation;

#[async_graphql::Object]
impl SheetImportMutation {
    /// Bring a reviewed sheet onto an actor. The server reads the file
    /// again and refuses with `PLAN_CHANGED` unless its plan hashes to
    /// `planHash`.
    async fn apply_sheet_import(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        file: Upload,
        corrections: Option<Json<Value>>,
        overwrite_play_state: Option<Vec<String>>,
        plan_hash: String,
    ) -> GraphQLResult<GraphQLActorImportRecord> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let upload = file.value(ctx)?;
        let limit = thunderforge_pdf::Limits::default().max_bytes as u64;
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(
            &mut std::io::Read::take(upload.into_read(), limit + 1),
            &mut bytes,
        )
        .map_err(|e| async_graphql::Error::new(format!("failed to read upload: {e}")))?;
        let systems_dir = state.directories.systems_dir.clone();
        let input = ApplyInput {
            actor_id,
            bytes,
            corrections: corrections.map(|Json(value)| value),
            overwrite_play_state: overwrite_play_state.unwrap_or_default(),
            plan_hash,
        };
        let import =
            apply_sheet_import_impl(state, &systems_dir, user.user_id, user.is_admin, input)
                .await
                .map_err(SheetImportError::into_graphql)?;
        let pool = state.db_pool.clone();
        let (user_id, is_admin) = (user.user_id, user.is_admin);
        let record = tokio::task::spawn_blocking(move || {
            let mut conn = pool
                .get()
                .map_err(|e| SheetImportError::Database(e.to_string()))?;
            record(&mut conn, user_id, is_admin, &import)
        })
        .await
        .map_err(|e| async_graphql::Error::new(e.to_string()))?
        .map_err(SheetImportError::into_graphql)?;
        Ok(record.into())
    }
}

#[cfg(test)]
#[path = "mutations_sheet_import_tests.rs"]
mod tests;
