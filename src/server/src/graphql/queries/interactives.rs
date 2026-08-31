//! `effectRegistry` and `interactives(sceneId)` — what a build can perform,
//! and what is authored on a scene.
//!
//! # Two different answers to one query
//!
//! A Game Master receives the authoring view: subject, effect, configuration,
//! fire state, and whether the effect is still available. A player receives
//! which subjects are interactive and whether they may activate them — not the
//! effect, not its configuration, not what it targets.
//!
//! That split is not a security boundary. Per the spec, secrets are a table
//! concern and the geometry reaches every client anyway. It is an *interface*
//! boundary: a player has no use for an effect's configuration, and sending it
//! would invite some future client to render it.

use async_graphql::{Context, Error, Json, Object, Result as GraphQLResult, SimpleObject};
use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::interaction::{ActivationOutcome, ConfigFieldKind};

use crate::graphql::{app_state, authenticated_user};

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLChoiceOption {
    pub value: String,
    pub label: String,
}

/// One field the authoring form renders.
///
/// Flattened rather than expressed as a GraphQL union because the shapes
/// differ only in which single extra field they carry, and a four-member union
/// would cost every client an inline fragment to read one string.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLConfigField {
    pub key: String,
    pub label: String,
    /// `boolean`, `choice`, `reference` or `referenceList`.
    pub kind: String,
    /// What a reference points at — `wall`, `light`, `loreEntry`, `scene`.
    /// Present for `reference` and `referenceList`.
    pub reference_of: Option<String>,
    /// Present for `choice`.
    pub options: Option<Vec<GraphQLChoiceOption>>,
    pub required: bool,
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLEffectDeclaration {
    pub id: String,
    pub label: String,
    pub description: String,
    /// `prop`, `door`, `region`.
    pub subject_kinds: Vec<String>,
    pub config: Vec<GraphQLConfigField>,
}

/// One authored interactive, reduced to what this viewer has a use for.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLInteractive {
    pub interactive_id: Uuid,
    pub scene_id: Uuid,
    pub subject_kind: String,
    pub subject_ref: Option<Uuid>,
    /// The region's area. Game Master only — a region is not an annotation and
    /// players are never shown one.
    pub geometry: Option<Json<serde_json::Value>>,
    pub trigger: String,
    /// Game Master only.
    pub effect_id: Option<String>,
    /// Game Master only.
    pub effect_config: Option<Json<serde_json::Value>>,
    /// Game Master only.
    pub activation: Option<String>,
    /// Game Master only.
    pub fire_mode: Option<String>,
    /// Game Master only.
    pub fired_at: Option<chrono::NaiveDateTime>,
    /// Whether this build still has the subsystem that performs the effect.
    ///
    /// Game Master only, and shown as a state rather than repaired (FR-041).
    /// A player is told nothing, because to them it is simply not interactive.
    pub available: Option<bool>,
    /// Whether *this* viewer's activation would do anything.
    ///
    /// The client uses it to decide whether to offer a cursor. It is a hint,
    /// not a permission: the server refuses on its own account when the
    /// mutation is called, because a client that draws the button anyway must
    /// still be told no.
    pub can_activate: bool,
}

/// One request awaiting a Game Master's decision.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLInteractionRequest {
    pub request_id: Uuid,
    pub interactive_id: Uuid,
    pub scene_id: Uuid,
    pub requested_by: Uuid,
    /// The player's display name, so the queue reads as people rather than ids.
    pub requested_by_name: Option<String>,
    /// What would run if it were approved, in the GM's language.
    pub proposed: Option<String>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Default)]
pub struct InteractiveQuery;

#[Object]
impl InteractiveQuery {
    /// What this build can perform.
    ///
    /// Drives the authoring form, so a Game Master is offered exactly what
    /// exists and never an option that would silently do nothing (FR-038).
    async fn effect_registry(
        &self,
        ctx: &Context<'_>,
    ) -> GraphQLResult<Vec<GraphQLEffectDeclaration>> {
        // Authenticated, and no more. Which effects a build compiled in is not
        // a secret about anybody's world.
        let _ = authenticated_user(ctx)?;
        Ok(crate::interaction::registry()
            .all()
            .map(declaration_to_graphql)
            .collect())
    }

    /// What is waiting on a decision in this scene. Game Master only.
    ///
    /// A player is not shown the queue. Their own outcome reaches them
    /// directly, and the rest of it is a list of what other people asked for —
    /// which is the Game Master's business and, at some tables, information
    /// the GM is deliberately not sharing yet.
    async fn pending_interaction_requests(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLInteractionRequest>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::{interactives, users};

            if !crate::auth::world_membership::is_dm_of_scene(
                &mut conn, user_id, is_admin, scene_id,
            )
            .map_err(|_| Error::new("Failed to check permission"))?
            {
                return Err(Error::new("Only the Game Master sees the queue"));
            }

            let rows = crate::interaction::pending_for_scene(&mut conn, scene_id)
                .map_err(|e| Error::new(format!("Failed to load requests: {e}")))?;

            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let requested_by_name: Option<String> = users::table
                    .filter(users::id.eq(row.requested_by))
                    .select(users::username)
                    .first(&mut conn)
                    .optional()
                    .ok()
                    .flatten();

                // What the effect is, rather than which effect it is. A GM
                // deciding mid-session should not have to translate
                // `nav.request_scene` in their head.
                let proposed = interactives::table
                    .filter(interactives::interactive_id.eq(row.interactive_id))
                    .select(interactives::effect_id)
                    .first::<Option<String>>(&mut conn)
                    .optional()
                    .ok()
                    .flatten()
                    .flatten()
                    .and_then(|id| {
                        crate::interaction::registry()
                            .get(&id)
                            .map(|d| d.label.clone())
                    });

                out.push(GraphQLInteractionRequest {
                    request_id: row.request_id,
                    interactive_id: row.interactive_id,
                    scene_id: row.scene_id,
                    requested_by: row.requested_by,
                    requested_by_name,
                    proposed,
                    created_at: row.created_at,
                });
            }
            Ok(out)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }

    /// Every interactive on a scene, as this viewer may know it.
    async fn interactives(
        &self,
        ctx: &Context<'_>,
        scene_id: Uuid,
    ) -> GraphQLResult<Vec<GraphQLInteractive>> {
        let state = app_state(ctx)?;
        let auth_user = authenticated_user(ctx)?;
        let user_id = auth_user.user_id;
        let is_admin = auth_user.is_admin;
        let mut conn = state
            .db_pool
            .get()
            .map_err(|_| Error::new("Failed to get DB connection"))?;

        tokio::task::spawn_blocking(move || {
            use crate::schema::{scenes, walls};

            // Membership before anything is read. What a scene contains is
            // world-scoped.
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

            let rows = crate::interaction::for_scene(&mut conn, scene_id)
                .map_err(|e| Error::new(format!("Failed to load interactives: {e}")))?;

            // Every locked wall in the scene, in one read rather than per row.
            let locked_walls: std::collections::HashSet<Uuid> = walls::table
                .filter(walls::scene_id.eq(scene_id))
                .filter(walls::locked.eq(true))
                .select(walls::wall_id)
                .load::<Uuid>(&mut conn)
                .unwrap_or_default()
                .into_iter()
                .collect();

            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let subject_locked = row
                    .subject_ref
                    .is_some_and(|id| row.subject_kind == "door" && locked_walls.contains(&id));
                let loaded = crate::interaction::LoadedInteractive {
                    row,
                    subject_locked,
                };
                let can_activate = matches!(
                    loaded.outcome(runs_the_world),
                    ActivationOutcome::Performed | ActivationOutcome::Requested
                );

                // A region is invisible to players, always. Nothing about it
                // is sent, not even that it exists.
                if !runs_the_world && loaded.row.subject_kind == "region" {
                    continue;
                }

                out.push(to_graphql(loaded, runs_the_world, can_activate));
            }
            Ok(out)
        })
        .await
        .map_err(|_| Error::new("Failed to spawn blocking task"))?
    }
}

fn to_graphql(
    loaded: crate::interaction::LoadedInteractive,
    runs_the_world: bool,
    can_activate: bool,
) -> GraphQLInteractive {
    let available = crate::interaction::is_available(loaded.row.effect_id.as_deref());
    let row = loaded.row;
    GraphQLInteractive {
        interactive_id: row.interactive_id,
        scene_id: row.scene_id,
        subject_kind: row.subject_kind,
        subject_ref: row.subject_ref,
        geometry: if runs_the_world {
            row.geometry.map(Json)
        } else {
            None
        },
        trigger: row.trigger,
        effect_id: if runs_the_world { row.effect_id } else { None },
        effect_config: if runs_the_world {
            row.effect_config.map(Json)
        } else {
            None
        },
        activation: runs_the_world.then_some(row.activation),
        fire_mode: runs_the_world.then_some(row.fire_mode),
        fired_at: if runs_the_world { row.fired_at } else { None },
        available: runs_the_world.then_some(available),
        can_activate,
    }
}

pub fn declaration_to_graphql(
    declaration: &thunderforge_canvas_core::interaction::EffectDeclaration,
) -> GraphQLEffectDeclaration {
    GraphQLEffectDeclaration {
        id: declaration.id.clone(),
        label: declaration.label.clone(),
        description: declaration.description.clone(),
        subject_kinds: declaration
            .subject_kinds
            .iter()
            .map(|k| k.as_str().to_string())
            .collect(),
        config: declaration
            .config
            .iter()
            .map(|field| {
                let (kind, reference_of, options) = match &field.kind {
                    ConfigFieldKind::Boolean => ("boolean", None, None),
                    ConfigFieldKind::Choice { options } => (
                        "choice",
                        None,
                        Some(
                            options
                                .iter()
                                .map(|o| GraphQLChoiceOption {
                                    value: o.value.clone(),
                                    label: o.label.clone(),
                                })
                                .collect(),
                        ),
                    ),
                    ConfigFieldKind::Reference { of } => ("reference", Some(of.clone()), None),
                    ConfigFieldKind::ReferenceList { of } => {
                        ("referenceList", Some(of.clone()), None)
                    }
                };
                GraphQLConfigField {
                    key: field.key.clone(),
                    label: field.label.clone(),
                    kind: kind.to_string(),
                    reference_of,
                    options,
                    required: field.required,
                }
            })
            .collect(),
    }
}
