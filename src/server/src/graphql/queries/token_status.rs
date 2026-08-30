//! `tokenStatus(sceneId)` — what this viewer may know about each token.
//!
//! Spec 029. The resolution rules live in `crate::status_display`, which is
//! unit-tested; this module loads what they need and shapes the answer for
//! GraphQL.
//!
//! # The one thing to get right
//!
//! Emit only the field the disclosure state permits. `CHUNKED` carries a
//! quarter index and nothing else; `PERCENTAGE` carries a proportion and **no
//! maximum**; `GREYED` carries neither. Everything else in this file is
//! bookkeeping around that.
//!
//! A test asserting the rendered screen would pass against a client that
//! received the value and chose not to draw it, so the assertions for this live
//! at the payload — see `status_display`'s
//! `no_coarse_resolution_carries_the_exact_figure`.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use thunderforge_canvas_core::resource_display::Disclosed;

use crate::graphql::{app_state, authenticated_user};
use crate::status_display::{
    DeclaredResource, declarations_for_system, overrides_for_tokens, resolve_token, subject_for,
};

/// One resource, as one viewer receives it.
///
/// The three payload fields are mutually exclusive and only one is ever
/// populated. GraphQL cannot express a discriminated union over scalars, so
/// the exclusivity is enforced where the value is produced (`disclose`) rather
/// than by the schema — and the type it comes from *can* express it, which is
/// why the coarsening happens there and not here.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLResolvedResource {
    pub definition_id: String,
    pub label: String,
    /// `bar` or `counter`.
    pub kind: String,
    /// `visible`, `greyed`, `percentage` or `chunked`.
    pub disclosure: String,
    /// Populated only when `disclosure` is `visible`.
    pub entries: Option<Vec<GraphQLResourceEntry>>,
    /// Populated only when `disclosure` is `percentage`. No maximum is sent.
    pub proportion: Option<f64>,
    /// Populated only when `disclosure` is `chunked`. 0-4.
    pub quarter: Option<i32>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLResourceEntry {
    pub current: i32,
    pub max: Option<i32>,
    pub label: Option<String>,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLTokenStatus {
    pub token_id: Uuid,
    pub resources: Vec<GraphQLResolvedResource>,
}

#[derive(Default)]
pub struct TokenStatusQuery;

#[Object]
impl TokenStatusQuery {
    /// Every token on a scene, with each one's resources reduced to what this
    /// viewer is entitled to see.
    async fn token_status(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLTokenStatus>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let systems_dir = state.directories.systems_dir.clone();

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens, world_actor_system_data, world_actors, worlds};

            // Membership first, before anything is read. Status is
            // world-scoped information about other people.
            let world_id: Uuid = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select(scenes::world_id)
                .first(&mut conn)
                .map_err(|_| Error::new("Scene not found"))?;

            let actor = crate::auth::world_membership::actor_in_world(
                &mut conn, user_id, is_admin, world_id,
            );
            if actor.role.is_none() && !actor.is_site_admin {
                return Err(Error::new("Not a member of this world"));
            }
            let runs_the_world = actor.runs_the_world();

            // Which system this world plays, and therefore which resources
            // exist at all.
            let system_id: Option<String> = worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first(&mut conn)
                .optional()
                .map_err(|e| Error::new(format!("Failed to read world: {e}")))?
                .flatten();

            let declarations: Vec<DeclaredResource> = match system_id.as_deref() {
                Some(id) => declarations_for_system(&systems_dir, id),
                // A world with no system declares no resources, so its tokens
                // carry no bars. Not an error.
                None => Vec::new(),
            };
            if declarations.is_empty() {
                return Ok(Vec::new());
            }

            let rows: Vec<(Uuid, Option<Uuid>, Option<Uuid>)> = tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .select((tokens::token_id, tokens::actor_id, tokens::owner_user_id))
                .load(&mut conn)
                .map_err(|e| Error::new(format!("Failed to load tokens: {e}")))?;

            let token_ids: Vec<Uuid> = rows.iter().map(|(id, _, _)| *id).collect();
            let overrides = overrides_for_tokens(&mut conn, &token_ids)
                .map_err(|e| Error::new(format!("Failed to load disclosure: {e}")))?;

            // Actor facts, in one read rather than per token.
            let actor_ids: Vec<Uuid> = rows.iter().filter_map(|(_, a, _)| *a).collect();
            let npc_flags: HashMap<Uuid, bool> = world_actors::table
                .filter(world_actors::id.eq_any(&actor_ids))
                .select((world_actors::id, world_actors::is_npc))
                .load::<(Uuid, bool)>(&mut conn)
                .unwrap_or_default()
                .into_iter()
                .collect();

            let stored: HashMap<Uuid, serde_json::Value> = world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
                .select((
                    world_actor_system_data::actor_id,
                    world_actor_system_data::resource_data,
                ))
                .load::<(Uuid, Option<serde_json::Value>)>(&mut conn)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(id, data)| data.map(|d| (id, d)))
                .collect();

            let empty = serde_json::json!({});
            let mut out = Vec::new();

            for (token_id, actor_id, owner) in rows {
                // A token bound to no actor has no resources. It is a marker,
                // not a creature.
                let Some(actor_id) = actor_id else {
                    continue;
                };

                let is_npc = npc_flags.get(&actor_id).copied().unwrap_or(true);
                let data = stored.get(&actor_id).unwrap_or(&empty);
                let token_overrides = overrides.get(&token_id).cloned().unwrap_or_default();

                let resolved = resolve_token(
                    token_id,
                    runs_the_world,
                    subject_for(user_id, is_npc, owner),
                    &declarations,
                    data,
                    &token_overrides,
                );

                if resolved.resources.is_empty() {
                    continue;
                }
                out.push(to_graphql(resolved));
            }

            Ok(out)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }
}

fn to_graphql(status: crate::status_display::TokenStatus) -> GraphQLTokenStatus {
    GraphQLTokenStatus {
        token_id: status.token_id,
        resources: status
            .resources
            .into_iter()
            .map(|resource| {
                let kind = match resource.definition.kind {
                    thunderforge_canvas_core::resource_display::ResourceKind::Bar => "bar",
                    thunderforge_canvas_core::resource_display::ResourceKind::Counter => "counter",
                };

                // Exactly one payload field, matching the state. The `match`
                // is exhaustive on purpose: a new disclosure state cannot be
                // added without deciding what it puts on the wire.
                let (disclosure, entries, proportion, quarter) = match resource.disclosed {
                    Disclosed::Visible { entries } => (
                        "visible",
                        Some(
                            entries
                                .into_iter()
                                .map(|e| GraphQLResourceEntry {
                                    current: e.current,
                                    max: e.max,
                                    label: e.label,
                                })
                                .collect(),
                        ),
                        None,
                        None,
                    ),
                    Disclosed::Greyed => ("greyed", None, None, None),
                    Disclosed::Percentage { proportion } => {
                        ("percentage", None, Some(proportion as f64), None)
                    }
                    Disclosed::Chunked { quarter } => ("chunked", None, None, Some(quarter as i32)),
                };

                GraphQLResolvedResource {
                    definition_id: resource.definition.id,
                    label: resource.definition.label,
                    kind: kind.to_string(),
                    disclosure: disclosure.to_string(),
                    entries,
                    proportion,
                    quarter,
                }
            })
            .collect(),
    }
}
