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
    /// What everyone who does not run the world sees for this resource.
    ///
    /// Present only for a viewer who *does* run it. A Game Master's own
    /// `disclosure` is always `visible`, so it cannot tell them what the table
    /// is under — and a control that guessed would show a setting nobody is
    /// actually playing with. Absent for a player, who has no business knowing
    /// which of the four states they are being shown through.
    pub configured: Option<String>,
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
                out.push(to_graphql(resolved, runs_the_world));
            }

            Ok(out)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }
}

/// Resolve one token for one viewer.
///
/// The single-token counterpart of the scene query. Extracted rather than
/// duplicated so the mutation's response cannot drift from what the query
/// would have returned for the same token — two resolvers answering the same
/// question differently is the exact shape of a bug this project has shipped
/// before.
fn resolve_one_token(
    conn: &mut diesel::PgConnection,
    systems_dir: &str,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    token_id: Uuid,
) -> GraphQLResult<GraphQLTokenStatus> {
    use crate::schema::{scenes, tokens, world_actor_system_data, world_actors, worlds};

    let world_id: Uuid = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first(conn)
        .map_err(|_| Error::new("Scene not found"))?;

    let actor = crate::auth::world_membership::actor_in_world(conn, user_id, is_admin, world_id);
    let runs_the_world = actor.runs_the_world();

    let system_id: Option<String> = worlds::table
        .filter(worlds::id.eq(world_id))
        .select(worlds::game_system_id)
        .first(conn)
        .optional()
        .map_err(|e| Error::new(format!("Failed to read world: {e}")))?
        .flatten();

    let declarations = match system_id.as_deref() {
        Some(id) => declarations_for_system(systems_dir, id),
        None => Vec::new(),
    };

    let (actor_id, owner): (Option<Uuid>, Option<Uuid>) = tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select((tokens::actor_id, tokens::owner_user_id))
        .first(conn)
        .map_err(|_| Error::new("Token not found"))?;

    let is_npc = match actor_id {
        Some(id) => world_actors::table
            .filter(world_actors::id.eq(id))
            .select(world_actors::is_npc)
            .first::<bool>(conn)
            .unwrap_or(true),
        None => true,
    };

    let stored: serde_json::Value = match actor_id {
        Some(id) => world_actor_system_data::table
            .filter(world_actor_system_data::actor_id.eq(id))
            .select(world_actor_system_data::resource_data)
            .first::<Option<serde_json::Value>>(conn)
            .ok()
            .flatten()
            .unwrap_or_else(|| serde_json::json!({})),
        None => serde_json::json!({}),
    };

    let overrides = overrides_for_tokens(conn, &[token_id])
        .map_err(|e| Error::new(format!("Failed to load disclosure: {e}")))?
        .remove(&token_id)
        .unwrap_or_default();

    Ok(to_graphql(
        resolve_token(
            token_id,
            runs_the_world,
            subject_for(user_id, is_npc, owner),
            &declarations,
            &stored,
            &overrides,
        ),
        runs_the_world,
    ))
}

fn to_graphql(
    status: crate::status_display::TokenStatus,
    runs_the_world: bool,
) -> GraphQLTokenStatus {
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
                    configured: runs_the_world.then(|| {
                        match resource.configured {
                            thunderforge_canvas_core::resource_display::DisclosureState::Visible => "visible",
                            thunderforge_canvas_core::resource_display::DisclosureState::Greyed => "greyed",
                            thunderforge_canvas_core::resource_display::DisclosureState::Percentage => "percentage",
                            thunderforge_canvas_core::resource_display::DisclosureState::Chunked => "chunked",
                        }
                        .to_string()
                    }),
                }
            })
            .collect(),
    }
}

/// Which state a Game Master is setting.
#[derive(async_graphql::Enum, Copy, Clone, Debug, Eq, PartialEq)]
pub enum GraphQLDisclosureState {
    Visible,
    Greyed,
    Percentage,
    Chunked,
}

impl GraphQLDisclosureState {
    fn as_stored(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::Greyed => "greyed",
            Self::Percentage => "percentage",
            Self::Chunked => "chunked",
        }
    }
}

#[derive(async_graphql::InputObject, Debug, Clone)]
pub struct SetTokenDisclosureInput {
    pub token_id: Uuid,
    pub resource_id: String,
    pub state: GraphQLDisclosureState,
}

#[derive(Default)]
pub struct TokenDisclosureMutation;

#[Object]
impl TokenDisclosureMutation {
    /// Set how much one token discloses about one resource.
    ///
    /// Per **token**, not per actor: two tokens of the same creature can
    /// legitimately differ, and the Game Master sets this on the one standing
    /// in front of the players.
    ///
    /// Returns the caller's own resolved view, which is always exact — a Game
    /// Master sees the truth regardless of what they have just hidden, because
    /// they still have to run the fight. The response must therefore not be
    /// mistaken for what a player will now see.
    async fn set_token_disclosure(
        &self,
        ctx: &Context<'_>,
        input: SetTokenDisclosureInput,
    ) -> GraphQLResult<GraphQLTokenStatus> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let systems_dir = state.directories.systems_dir.clone();

        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        let resolved = tokio::task::spawn_blocking(move || {
            use crate::schema::{token_resource_disclosure as trd, tokens};

            // Authority to change what other people know follows the world
            // role — Owner or Game Master, never a Player. Reuses
            // `is_dm_of_scene` rather than adding a parallel check.
            let scene_id: Uuid = tokens::table
                .filter(tokens::token_id.eq(input.token_id))
                .select(tokens::scene_id)
                .first(&mut conn)
                .map_err(|_| Error::new("Token not found"))?;

            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )
            .map_err(|e| Error::new(format!("Failed to check authority: {e}")))?
            {
                return Err(Error::new(
                    "Only the Owner or a Game Master may change what a token discloses",
                ));
            }

            let now = chrono::Utc::now().naive_utc();
            diesel::insert_into(trd::table)
                .values((
                    trd::token_id.eq(input.token_id),
                    trd::resource_id.eq(&input.resource_id),
                    trd::state.eq(input.state.as_stored()),
                    trd::created_by.eq(user_id),
                    trd::updated_by.eq(user_id),
                    trd::created_at.eq(now),
                    trd::updated_at.eq(now),
                ))
                .on_conflict((trd::token_id, trd::resource_id))
                .do_update()
                .set((
                    trd::state.eq(input.state.as_stored()),
                    trd::updated_by.eq(user_id),
                    trd::updated_at.eq(now),
                ))
                .execute(&mut conn)
                .map_err(|e| Error::new(format!("Failed to store disclosure: {e}")))?;

            // Tell connected clients. A player watching this token needs the
            // change without reloading — a Game Master revealing a boss
            // mid-fight cannot ask the table to refresh.
            if let Ok(world_id) = crate::world_events::world_id_for_scene(&mut conn, scene_id) {
                let _ = crate::world_events::record_world_event(
                    &mut conn,
                    world_id,
                    crate::world_events::EVENT_CODE_TOKEN_DISCLOSURE_CHANGED,
                    // The payload deliberately carries the token and nothing
                    // else. What *changed* about it depends on who is asking,
                    // so a client re-reads through the resolver that knows how
                    // to coarsen rather than being handed a value here.
                    Some(serde_json::json!({ "tokenId": input.token_id })),
                    user_id,
                );
            }

            Ok(scene_id)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))??;

        // The GM's own view of the token they just changed.
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;
        let token_id = input.token_id;

        tokio::task::spawn_blocking(move || {
            resolve_one_token(
                &mut conn,
                &systems_dir,
                user_id,
                is_admin,
                resolved,
                token_id,
            )
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }
}
