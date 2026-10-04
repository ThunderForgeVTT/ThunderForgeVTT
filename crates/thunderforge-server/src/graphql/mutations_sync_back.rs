//! Syncing a world's changes back to its collection, on the wire (spec 049
//! Phase 15, spec 050 FR-100 to FR-105).
//!
//! Two fields, and the split is FR-103: `worldSyncBackPlan` says everything a
//! sync back would do to the shelf, and `syncBackToCollection` does it only
//! when handed the plan's stamp. There is no field that syncs without a plan,
//! and none that takes a book read in — every rule is
//! [`crate::library::deltas::sync_back`]'s.

use async_graphql::{Context, Enum, Object, SimpleObject};
use uuid::Uuid;

use crate::graphql::mutations_compendium::GraphQLWorldUnattachedDeltas;
use crate::graphql::mutations_library::{
    GraphQLEntryBefore, GraphQLUnattachedDelta, delta_refusal,
};
use crate::graphql::{Error, GraphQLResult, app_state, authenticated_user};
use crate::library::deltas::{self, Before, ShelfChange, SyncPlan};
use crate::state::AppState;

/// What a sync back does to one entry on the shelf.
#[derive(Enum, Copy, Clone, Debug, PartialEq, Eq)]
#[graphql(name = "ShelfChangeKind")]
pub enum GraphQLShelfChangeKind {
    /// The collection's entry takes this world's version of it.
    Rewritten,
    /// This world hid it, so the collection no longer holds it.
    TakenOut,
    /// This world wrote it, so the collection gains it.
    Added,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "ShelfChange")]
pub struct GraphQLShelfChange {
    pub kind: String,
    pub name: String,
    pub change: GraphQLShelfChangeKind,
    /// What the collection says now. `null` for an addition.
    pub before: Option<GraphQLEntryBefore>,
    /// What it will say. `null` for an entry taken out.
    pub after: Option<GraphQLEntryBefore>,
}

fn wire(before: Before) -> GraphQLEntryBefore {
    GraphQLEntryBefore {
        field_values: async_graphql::Json(before.field_values),
        prose_text: before.prose_text,
    }
}

impl From<ShelfChange> for GraphQLShelfChange {
    fn from(change: ShelfChange) -> Self {
        match change {
            ShelfChange::Rewritten {
                kind,
                name,
                before,
                after,
            } => Self {
                kind,
                name,
                change: GraphQLShelfChangeKind::Rewritten,
                before: Some(wire(before)),
                after: Some(wire(after)),
            },
            ShelfChange::TakenOut { kind, name, before } => Self {
                kind,
                name,
                change: GraphQLShelfChangeKind::TakenOut,
                before: Some(wire(before)),
                after: None,
            },
            ShelfChange::Added { kind, name, after } => Self {
                kind,
                name,
                change: GraphQLShelfChangeKind::Added,
                before: None,
                after: Some(wire(after)),
            },
        }
    }
}

/// Everything a sync back would do, before it is confirmed (FR-103).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SyncBackPlan")]
pub struct GraphQLSyncBackPlan {
    pub compendium_id: Uuid,
    pub collection_title: String,
    pub world_name: String,
    /// The version in force; the sync makes the next one, and this one is
    /// kept (FR-104).
    pub base_version: i32,
    pub changes: Vec<GraphQLShelfChange>,
    /// This world's changes that do not attach, which stay in the world.
    pub staying: Vec<GraphQLUnattachedDelta>,
    /// Hand this back to `syncBackToCollection`. If the collection or the
    /// changes move on first, the sync is refused rather than landing
    /// something nobody looked at.
    pub stamp: String,
}

impl From<SyncPlan> for GraphQLSyncBackPlan {
    fn from(plan: SyncPlan) -> Self {
        Self {
            compendium_id: plan.collection_id,
            collection_title: plan.collection_title,
            world_name: plan.world_name,
            base_version: plan.base_version,
            changes: plan.changes.into_iter().map(Into::into).collect(),
            staying: plan
                .staying
                .into_iter()
                .map(GraphQLUnattachedDelta::from)
                .collect(),
            stamp: plan.stamp,
        }
    }
}

/// What a landed sync back did.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "SyncBackOutcome")]
pub struct GraphQLSyncBackOutcome {
    pub plan: GraphQLSyncBackPlan,
    /// The version the collection is at now.
    pub base_version: i32,
    /// Changes in any world that no longer attach to the new version, per
    /// world, as a re-read reports them (FR-105, FR-027). Still stored.
    pub stranded: Vec<GraphQLWorldUnattachedDeltas>,
}

fn connection(
    state: &AppState,
) -> GraphQLResult<
    diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>,
> {
    state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))
}

/// Testable core of `worldSyncBackPlan`.
pub async fn sync_back_plan_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
) -> GraphQLResult<GraphQLSyncBackPlan> {
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        deltas::plan_sync_back(&mut conn, caller, world_id, compendium_id)
            .map(GraphQLSyncBackPlan::from)
            .map_err(delta_refusal)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

/// Testable core of `syncBackToCollection`.
pub async fn sync_back_impl(
    state: &AppState,
    caller: Uuid,
    world_id: Uuid,
    compendium_id: Uuid,
    stamp: String,
) -> GraphQLResult<GraphQLSyncBackOutcome> {
    let mut conn = connection(state)?;
    tokio::task::spawn_blocking(move || {
        let outcome = deltas::sync_back(&mut conn, caller, world_id, compendium_id, &stamp)
            .map_err(delta_refusal)?;
        Ok(GraphQLSyncBackOutcome {
            plan: outcome.plan.into(),
            base_version: outcome.base_version,
            stranded: outcome
                .stranded
                .into_iter()
                .map(|world| GraphQLWorldUnattachedDeltas {
                    world_id: world.world_id,
                    world_name: world.world_name,
                    deltas: world
                        .deltas
                        .into_iter()
                        .map(GraphQLUnattachedDelta::from)
                        .collect(),
                })
                .collect(),
        })
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[derive(Default)]
pub struct SyncBackQuery;

#[Object]
impl SyncBackQuery {
    /// What syncing this world's changes back to a collection would do to
    /// the shelf, without doing it (050 FR-103). The world's owner's alone. A
    /// book read in is refused with the reason (FR-101, FR-102).
    async fn world_sync_back_plan(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
    ) -> GraphQLResult<GraphQLSyncBackPlan> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        sync_back_plan_impl(state, user.user_id, world_id, compendium_id).await
    }
}

#[derive(Default)]
pub struct SyncBackMutation;

#[Object]
impl SyncBackMutation {
    /// Sync this world's changes back to the collection as a new version,
    /// keeping the one it replaces (050 FR-100, FR-104). Only with the stamp
    /// of the plan that was shown (FR-103). The world stops holding what was
    /// synced (FR-105).
    async fn sync_back_to_collection(
        &self,
        ctx: &Context<'_>,
        world_id: Uuid,
        compendium_id: Uuid,
        stamp: String,
    ) -> GraphQLResult<GraphQLSyncBackOutcome> {
        let state = app_state(ctx)?;
        let user = authenticated_user(ctx)?;
        sync_back_impl(state, user.user_id, world_id, compendium_id, stamp).await
    }
}
