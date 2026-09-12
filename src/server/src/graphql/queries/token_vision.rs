//! `tokenVision(sceneId)` — how far each token in a scene can see.
//!
//! A sibling query keyed by token id, like `tokenAttributes`, rather than a
//! field on the token type. Same reason: the inputs are a system's manifest
//! and every actor's stored sheet, loaded once for a whole scene, and hanging
//! that off each token would turn one read into one per token.
//!
//! Spec 045 US6 and owner decision 2: the *system* says how its creatures see.
//! The server resolves it because the alternative is shipping each client the
//! manifest and every sheet and asking them all to agree.

use async_graphql::{Context, Error, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use std::collections::HashMap;
use uuid::Uuid;

use crate::declared_values::ActorSlots;
use crate::graphql::{app_state, authenticated_user};
use crate::vision_profiles::{cells_to_world, resolve, vision_declaration_for_system};

/// One token's sight, in world units — what the engine's `set_token_vision`
/// takes, so no client has to know what a cell is worth.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLTokenVision {
    pub token_id: Uuid,
    /// How far this token sees in darkness. Zero means ordinary sight.
    pub darkvision: f64,
    /// The bright reach of a light this character carries. Zero means none.
    pub carried_bright: f64,
    /// Its dim reach.
    pub carried_dim: f64,
}

#[derive(Default)]
pub struct TokenVisionQuery;

#[Object]
impl TokenVisionQuery {
    /// Vision for every token in a scene whose system declares any and whose
    /// character has some.
    ///
    /// A token that resolves to ordinary sight is **omitted**, not returned as
    /// zeroes. The list is "who sees unusually", and an entry saying "this
    /// creature sees nothing special" is the same claim as no entry, with more
    /// rows.
    async fn token_vision(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLTokenVision>> {
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

            // Membership before anything is read, exactly as `tokenAttributes`
            // does: what a creature can see is world-scoped information about
            // somebody else's character.
            let (world_id, grid_size): (Uuid, i32) = scenes::table
                .filter(scenes::scene_id.eq(scene_id))
                .select((scenes::world_id, scenes::grid_size))
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

            let declaration = match system_id.as_deref() {
                Some(id) => vision_declaration_for_system(&systems_dir, id),
                // A world with no system declares no vision. Not an error.
                None => return Ok(Vec::new()),
            };
            // Nothing declared, nothing to resolve — and no reason to read a
            // single sheet.
            if declaration == Default::default() {
                return Ok(Vec::new());
            }

            let rows: Vec<(Uuid, Option<Uuid>)> = tokens::table
                .filter(tokens::scene_id.eq(scene_id))
                .select((tokens::token_id, tokens::actor_id))
                .load(&mut conn)
                .map_err(|e| Error::new(format!("Failed to load tokens: {e}")))?;

            let actor_ids: Vec<Uuid> = rows.iter().filter_map(|(_, a)| *a).collect();

            type SlotRow = (
                Uuid,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
                Option<serde_json::Value>,
            );
            // One read for the whole scene, not one per token.
            let slots: HashMap<Uuid, ActorSlots> = world_actor_system_data::table
                .filter(world_actor_system_data::actor_id.eq_any(&actor_ids))
                .select((
                    world_actor_system_data::actor_id,
                    world_actor_system_data::ability_data,
                    world_actor_system_data::resource_data,
                    world_actor_system_data::proficiency_data,
                    world_actor_system_data::trait_data,
                ))
                .load::<SlotRow>(&mut conn)
                .unwrap_or_default()
                .into_iter()
                .map(
                    |(id, ability_data, resource_data, proficiency_data, trait_data)| {
                        (
                            id,
                            ActorSlots {
                                ability_data,
                                resource_data,
                                proficiency_data,
                                trait_data,
                            },
                        )
                    },
                )
                .collect();

            let mut out = Vec::new();
            for (token_id, actor_id) in rows {
                // A token bound to no actor is a marker, not a creature.
                let Some(slot) = actor_id.and_then(|id| slots.get(&id)) else {
                    continue;
                };
                let resolved = resolve(slot, &declaration);
                if resolved.is_ordinary() {
                    continue;
                }
                out.push(GraphQLTokenVision {
                    token_id,
                    darkvision: cells_to_world(resolved.darkvision, grid_size) as f64,
                    carried_bright: cells_to_world(resolved.carried_bright, grid_size) as f64,
                    carried_dim: cells_to_world(resolved.carried_dim, grid_size) as f64,
                });
            }

            Ok(out)
        })
        .await
        .map_err(|e| Error::new(format!("Task failed: {e}")))?
    }
}
