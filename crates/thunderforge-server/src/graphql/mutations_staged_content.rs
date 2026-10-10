//! Spec 048: what players brought, as the GM's queue reads and decides it
//! (contracts/graphql-sheet-import.md, `stagedContent` and the four
//! decisions). The rules live in `crate::staged_content::decide`; this file
//! only converts.

use async_graphql::{Context, Json, Object, Result as GraphQLResult, SimpleObject};
use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use super::mutations_sheet_import::GraphQLUserSummary;
use super::queries::sheet_import::GraphQLStagedState;
use super::{app_state, authenticated_user};
use crate::sheet_import::error::SheetImportError;
use crate::staged_content::StagedState;
use crate::staged_content::decide::{self, StagedView};

impl From<GraphQLStagedState> for StagedState {
    fn from(state: GraphQLStagedState) -> Self {
        match state {
            GraphQLStagedState::Pending => Self::Pending,
            GraphQLStagedState::Adopted => Self::Adopted,
            GraphQLStagedState::Declined => Self::Declined,
        }
    }
}

/// An actor that links a piece.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ActorSummary")]
pub struct GraphQLActorSummary {
    pub id: Uuid,
    pub label: String,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "StagedContent")]
pub struct GraphQLStagedContent {
    pub id: Uuid,
    /// A vocabulary type, or `item`.
    pub kind: String,
    pub name: String,
    pub field_values: Json<Value>,
    pub state: GraphQLStagedState,
    pub brought_by: GraphQLUserSummary,
    pub actors: Vec<GraphQLActorSummary>,
    /// The piece of the same name this one differs from.
    pub differs_from: Option<Uuid>,
    pub decided_by: Option<GraphQLUserSummary>,
    pub decided_at: Option<DateTime<Utc>>,
    /// What it became, once adopted.
    pub adopted_ability_id: Option<Uuid>,
    pub adopted_item_id: Option<Uuid>,
}

impl From<StagedView> for GraphQLStagedContent {
    fn from(view: StagedView) -> Self {
        let piece = view.piece;
        Self {
            id: piece.id,
            kind: piece.kind,
            name: piece.name,
            field_values: Json(piece.field_values),
            state: piece.state.into(),
            brought_by: view.brought_by.into(),
            actors: view
                .actors
                .into_iter()
                .map(|(id, label)| GraphQLActorSummary { id, label })
                .collect(),
            differs_from: piece.differs_from,
            decided_by: view.decided_by.map(Into::into),
            decided_at: piece.decided_at.map(|at| at.and_utc()),
            adopted_ability_id: piece.adopted_ability_id,
            adopted_item_id: piece.adopted_item_id,
        }
    }
}

fn one(mut views: Vec<StagedView>) -> GraphQLResult<GraphQLStagedContent> {
    views
        .pop()
        .map(Into::into)
        .ok_or_else(|| SheetImportError::Database("the piece vanished".into()).into_graphql())
}

fn all(views: Vec<StagedView>) -> Vec<GraphQLStagedContent> {
    views.into_iter().map(Into::into).collect()
}

#[derive(Default)]
pub struct StagedContentQuery;

#[Object]
impl StagedContentQuery {
    /// What players brought into the world. The GM and a Trusted Player
    /// see everything; a Player sees only their own.
    async fn staged_content(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        state: Option<GraphQLStagedState>,
        player_id: Option<Uuid>,
    ) -> GraphQLResult<Vec<GraphQLStagedContent>> {
        let app = app_state(ctx)?;
        let viewer = authenticated_user(ctx)?.user_id;
        let state = state.map(StagedState::from);
        decide::blocking(app, move |conn| {
            decide::list(conn, viewer, world_id, state, player_id)
        })
        .await
        .map(all)
        .map_err(SheetImportError::into_graphql)
    }
}

#[derive(Default)]
pub struct StagedContentMutation;

#[Object]
impl StagedContentMutation {
    /// Take a piece into the world's books. Every character using it now
    /// uses the world's row.
    async fn adopt_staged_content(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<GraphQLStagedContent> {
        let app = app_state(ctx)?;
        let user = authenticated_user(ctx)?.user_id;
        decide::blocking(app, move |conn| {
            decide::adopt(conn, user, id).map(|p| vec![p])
        })
        .await
        .map_err(SheetImportError::into_graphql)
        .and_then(one)
    }

    /// Adopt everything one player has pending, as it stands now.
    async fn adopt_all_staged_content(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        player_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLStagedContent>> {
        let app = app_state(ctx)?;
        let user = authenticated_user(ctx)?.user_id;
        decide::blocking(app, move |conn| {
            decide::adopt_all(conn, user, world_id, player_id)
        })
        .await
        .map(all)
        .map_err(SheetImportError::into_graphql)
    }

    /// Keep a piece out of play. It can be revisited.
    async fn decline_staged_content(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
    ) -> GraphQLResult<GraphQLStagedContent> {
        let app = app_state(ctx)?;
        let user = authenticated_user(ctx)?.user_id;
        decide::blocking(app, move |conn| {
            decide::decline(conn, user, id).map(|p| vec![p])
        })
        .await
        .map_err(SheetImportError::into_graphql)
        .and_then(one)
    }

    /// Move a declined piece back to pending, or straight to adopted.
    async fn revisit_staged_content(
        &self,
        ctx: &Context<'_>,
        id: Uuid,
        state: GraphQLStagedState,
    ) -> GraphQLResult<GraphQLStagedContent> {
        let app = app_state(ctx)?;
        let user = authenticated_user(ctx)?.user_id;
        let to = StagedState::from(state);
        decide::blocking(app, move |conn| {
            decide::revisit(conn, user, id, to).map(|p| vec![p])
        })
        .await
        .map_err(SheetImportError::into_graphql)
        .and_then(one)
    }
}

#[cfg(test)]
#[path = "mutations_staged_content_tests.rs"]
mod tests;
