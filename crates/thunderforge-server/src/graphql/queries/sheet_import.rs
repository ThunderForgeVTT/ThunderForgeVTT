//! Spec 048: the reads behind bringing a character in — the plan, made
//! without writing anything, and an actor's import history
//! (contracts/graphql-sheet-import.md).

use async_graphql::{Context, Json, Object, Result as GraphQLResult};
use serde_json::Value;
use uuid::Uuid;

use crate::graphql::mutations_sheet_import::{GraphQLActorImportRecord, GraphQLSheetImportPlan};
use crate::graphql::{app_state, authenticated_user};
use crate::sheet_import::error::SheetImportError;
use crate::sheet_import::preview::sheet_import_preview_impl;
use crate::sheet_import::records::actor_imports_impl;

#[derive(Default)]
pub struct SheetImportQuery;

#[Object]
impl SheetImportQuery {
    /// What bringing this reading onto the actor would change. Writes
    /// nothing; its `planHash` is what `applySheetImport` must match.
    async fn sheet_import_preview(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
        reading: Json<Value>,
        corrections: Option<Json<Value>>,
    ) -> GraphQLResult<GraphQLSheetImportPlan> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();
        let plan = sheet_import_preview_impl(
            state,
            &systems_dir,
            user.user_id,
            user.is_admin,
            actor_id,
            &reading.0,
            corrections.as_ref().map(|Json(value)| value),
        )
        .await
        .map_err(SheetImportError::into_graphql)?;
        Ok((&plan).into())
    }

    /// Every import and rollback on the actor, newest first.
    async fn actor_imports(
        &self,
        ctx: &Context<'_>,
        actor_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLActorImportRecord>> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        let records = actor_imports_impl(state, user.user_id, user.is_admin, actor_id)
            .await
            .map_err(SheetImportError::into_graphql)?;
        Ok(records.into_iter().map(Into::into).collect())
    }
}
