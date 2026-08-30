//! `tokenAttributes(sceneId)` — each token's attribute scores, as its own
//! system names them.
//!
//! Deliberately a sibling of `tokenStatus` rather than a field on it. The two
//! answer different questions: status is *what this viewer may know*, and
//! carries a per-viewer coarsening model. Attributes are simply what the
//! character sheet says. Folding them into the status resolver would imply a
//! disclosure model that does not exist for them and would have to be
//! invented on the spot — and the first person to add one would find it
//! half-applied.
//!
//! The resolution rules live in `thunderforge_canvas_core::attributes`, where
//! tests execute; the manifest reading lives in `crate::attributes`. This
//! module loads what they need and shapes the answer.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use thunderforge_canvas_core::attributes::{AttributeDeclaration, attributes_from};

use crate::attributes::attribute_declarations_for_system;
use crate::graphql::{app_state, authenticated_user};

/// One attribute score.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLAttribute {
    /// The system's own identifier — `might`, `strength`, `prowess`.
    pub id: String,
    pub label: String,
    /// Short form where the system offers one; systems differ.
    pub abbreviation: Option<String>,
    pub value: i32,
}

/// Every attribute one token carries.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLTokenAttributes {
    pub token_id: Uuid,
    pub attributes: Vec<GraphQLAttribute>,
}

#[derive(Default)]
pub struct TokenAttributesQuery;

#[Object]
impl TokenAttributesQuery {
    /// Attribute scores for every token in a scene that has any.
    ///
    /// A token with no actor, or an actor whose sheet is unfilled, is omitted
    /// rather than returned empty — an empty list would say "this character
    /// has no attributes", which is a claim about the ruleset rather than
    /// about the sheet.
    async fn token_attributes(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLTokenAttributes>> {
        let user = authenticated_user(ctx)?;
        let user_id = user.user_id;
        let is_admin = user.is_admin;
        let state = app_state(ctx)?;
        let systems_dir = state.directories.systems_dir.clone();

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, tokens, world_actor_system_data, worlds};

            // Membership before anything is read. A character sheet is
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

            let system_id: Option<String> = worlds::table
                .filter(worlds::id.eq(world_id))
                .select(worlds::game_system_id)
                .first(&mut conn)
                .optional()
                .map_err(|e| Error::new(format!("Failed to read world: {e}")))?
                .flatten();

            let declarations: Vec<AttributeDeclaration> = match system_id.as_deref() {
                Some(id) => attribute_declarations_for_system(&systems_dir, id),
                // A world with no system has no attribute set. Not an error.
                None => Vec::new(),
            };
            if declarations.is_empty() {
                return Ok(Vec::new());
            }

            let rows: Vec<(Uuid, Option<Uuid>)> = tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .select((tokens::token_id, tokens::actor_id))
                .load(&mut conn)
                .map_err(|e| Error::new(format!("Failed to load tokens: {e}")))?;

            let actor_ids: Vec<Uuid> = rows.iter().filter_map(|(_, a)| *a).collect();

            // One read for the whole scene rather than one per token.
            let stored: HashMap<Uuid, serde_json::Value> = world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
                .select((
                    world_actor_system_data::actor_id,
                    world_actor_system_data::ability_data,
                ))
                .load::<(Uuid, Option<serde_json::Value>)>(&mut conn)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(id, data)| data.map(|d| (id, d)))
                .collect();

            let mut out = Vec::new();
            for (token_id, actor_id) in rows {
                // A token bound to no actor is a marker, not a creature.
                let Some(actor_id) = actor_id else {
                    continue;
                };
                let Some(data) = stored.get(&actor_id) else {
                    continue;
                };

                let resolved = attributes_from(data, &declarations);
                if resolved.is_empty() {
                    continue;
                }

                out.push(GraphQLTokenAttributes {
                    token_id,
                    attributes: resolved
                        .into_iter()
                        .map(|a| GraphQLAttribute {
                            id: a.id,
                            label: a.label,
                            abbreviation: a.abbreviation,
                            value: a.value,
                        })
                        .collect(),
                });
            }

            Ok(out)
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}
