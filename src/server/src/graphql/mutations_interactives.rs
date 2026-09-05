//! Authoring, activation and approval for interactive elements (spec 030).
//!
//! # The rule this file exists to get right
//!
//! *A player cannot open a locked door.* The tempting implementation is to not
//! draw the button, which passes every test that looks at a screen and fails
//! the moment somebody calls the mutation directly. Principle III puts
//! authorization at the data boundary, and this is the boundary.
//!
//! So every refusal below is decided here, from stored state, by
//! `canvas-core`'s activation table — not inferred from what a client chose to
//! offer. The engine may apply the visible change optimistically for
//! responsiveness; it is never a second authority on whether the change was
//! allowed.

use async_graphql::{Context, Error, InputObject, Json, Result as GraphQLResult, SimpleObject};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use thunderforge_canvas_core::interaction::{ActivationOutcome, FireMode, validate_draft};

use crate::graphql::{app_state, authenticated_user};
use crate::world_events::{
    EVENT_CODE_INTERACTION_REQUEST, EVENT_CODE_INTERACTIVE_CHANGED, record_world_event,
    world_id_for_scene,
};

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLCreateInteractiveInput {
    pub scene_id: Uuid,
    /// `prop`, `door` or `region`.
    pub subject_kind: String,
    /// The token for a prop, the wall for a door. Omitted for a region.
    pub subject_ref: Option<Uuid>,
    /// The area, for a region. Omitted otherwise.
    pub geometry: Option<Json<serde_json::Value>>,
    /// Omitted for scenery, which is legitimate.
    pub effect_id: Option<String>,
    pub effect_config: Option<Json<serde_json::Value>>,
    /// `click` or `enter`.
    pub trigger: String,
    /// `anyone`, `gm_only` or `requires_approval`.
    pub activation: String,
    /// `always` or `once`.
    pub fire_mode: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct GraphQLUpdateInteractiveInput {
    pub geometry: Option<Json<serde_json::Value>>,
    pub effect_id: Option<String>,
    pub effect_config: Option<Json<serde_json::Value>>,
    pub trigger: Option<String>,
    pub activation: Option<String>,
    pub fire_mode: Option<String>,
    /// Explicitly clear the effect, turning an interactive back into scenery.
    ///
    /// Needed because an absent `effectId` means "leave it alone" in a partial
    /// update, and there would otherwise be no way to say "remove it".
    pub clear_effect: Option<bool>,
}

/// What happened, as a tagged outcome rather than a boolean.
///
/// "It did not run" covers four genuinely different situations, and a player
/// told only "no" cannot tell a locked door from a broken product.
#[derive(SimpleObject, Debug, Clone)]
pub struct GraphQLActivationResult {
    /// `performed`, `requested`, `refused`, `unavailable` or `noEffect`.
    pub outcome: String,
    /// `gmOnly`, `locked` or `alreadyFired`. Present only when refused.
    pub reason: Option<String>,
    /// Present only when a request was raised.
    pub request_id: Option<Uuid>,
    /// The effect to run and its configuration, for the client to apply
    /// optimistically. Present only when the effect actually ran.
    pub effect_id: Option<String>,
    pub effect_config: Option<Json<serde_json::Value>>,
    /// Things the Game Master should know about what just ran.
    ///
    /// Empty for a player, always. These are notes about the *authoring* — a
    /// switch naming a lamp that has been deleted — and a player has no use
    /// for one and no way to act on it.
    pub notices: Vec<String>,
}

impl GraphQLActivationResult {
    fn from_outcome(outcome: ActivationOutcome) -> Self {
        let (name, reason) = match outcome {
            ActivationOutcome::Performed => ("performed", None),
            ActivationOutcome::Requested => ("requested", None),
            ActivationOutcome::Refused { reason } => ("refused", Some(reason.as_str().to_string())),
            ActivationOutcome::Unavailable => ("unavailable", None),
            ActivationOutcome::NoEffect => ("noEffect", None),
        };
        Self {
            outcome: name.to_string(),
            reason,
            request_id: None,
            effect_id: None,
            effect_config: None,
            notices: Vec::new(),
        }
    }
}

#[derive(Default)]
pub struct InteractiveMutation;

/// The resolvers are deliberately thin.
///
/// Each one reads the caller out of the GraphQL context and hands off to an
/// `_impl` free function below. That is not ceremony: `Context<'_>` cannot be
/// constructed in a test, so a rule living inside a resolver can only ever be
/// tested through a running server. The rules here — a player cannot author,
/// a player cannot open a locked door — are exactly the ones that must be
/// provable at the server rather than through a screen.
#[async_graphql::Object]
impl InteractiveMutation {
    /// Author an interactive. Game Master only (FR-005).
    async fn create_interactive(
        &self,
        ctx: &Context<'_>,
        input: GraphQLCreateInteractiveInput,
    ) -> GraphQLResult<super::queries::interactives::GraphQLInteractive> {
        let auth_user = authenticated_user(ctx)?;
        create_interactive_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            input,
        )
        .await
    }

    /// Edit an interactive. Game Master only.
    async fn update_interactive(
        &self,
        ctx: &Context<'_>,
        interactive_id: Uuid,
        input: GraphQLUpdateInteractiveInput,
    ) -> GraphQLResult<super::queries::interactives::GraphQLInteractive> {
        let auth_user = authenticated_user(ctx)?;
        update_interactive_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            interactive_id,
            input,
        )
        .await
    }

    /// Remove an interactive. Game Master only.
    async fn delete_interactive(
        &self,
        ctx: &Context<'_>,
        interactive_id: Uuid,
    ) -> GraphQLResult<bool> {
        let auth_user = authenticated_user(ctx)?;
        delete_interactive_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            interactive_id,
        )
        .await
    }

    /// Let a `once` interactive fire again (FR-031). Game Master only.
    async fn reset_interactive(
        &self,
        ctx: &Context<'_>,
        interactive_id: Uuid,
    ) -> GraphQLResult<super::queries::interactives::GraphQLInteractive> {
        let auth_user = authenticated_user(ctx)?;
        reset_interactive_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            interactive_id,
        )
        .await
    }

    /// Run a request the Game Master has approved.
    ///
    /// The effect runs **now**, with the permission it has **now** — not the
    /// permission it had when the player asked. A GM who locked the door after
    /// the request was raised has contradicted themselves, and the lock wins.
    async fn approve_request(
        &self,
        ctx: &Context<'_>,
        request_id: Uuid,
    ) -> GraphQLResult<GraphQLActivationResult> {
        let auth_user = authenticated_user(ctx)?;
        decide_request_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            request_id,
            true,
        )
        .await
    }

    /// Turn a request down. The requester is told (FR-028).
    async fn refuse_request(
        &self,
        ctx: &Context<'_>,
        request_id: Uuid,
    ) -> GraphQLResult<GraphQLActivationResult> {
        let auth_user = authenticated_user(ctx)?;
        decide_request_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            request_id,
            false,
        )
        .await
    }

    /// Make a wall a door, or stop it being one (FR-007). Game Master only.
    ///
    /// Designating also gives the door an interactive, so it can be opened by
    /// clicking it without the Game Master having to author one — a door
    /// nobody can touch is not what "designate a door" means to anybody.
    async fn set_door_designation(
        &self,
        ctx: &Context<'_>,
        wall_id: Uuid,
        is_door: bool,
    ) -> GraphQLResult<bool> {
        let auth_user = authenticated_user(ctx)?;
        set_door_designation_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            wall_id,
            is_door,
        )
        .await
    }

    /// Lock or unlock a door (FR-013). Game Master only.
    async fn set_door_lock(
        &self,
        ctx: &Context<'_>,
        wall_id: Uuid,
        locked: bool,
    ) -> GraphQLResult<bool> {
        let auth_user = authenticated_user(ctx)?;
        set_door_flag_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            wall_id,
            DoorFlag::Locked(locked),
        )
        .await
    }

    /// Hide or reveal a secret door. Game Master only.
    async fn set_door_secret(
        &self,
        ctx: &Context<'_>,
        wall_id: Uuid,
        secret: bool,
    ) -> GraphQLResult<bool> {
        let auth_user = authenticated_user(ctx)?;
        set_door_flag_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            wall_id,
            DoorFlag::Secret(secret),
        )
        .await
    }

    /// The one mutation a player calls.
    ///
    /// Every refusal is decided at the server, from stored state. A client
    /// that draws the button anyway is told no.
    async fn activate_interactive(
        &self,
        ctx: &Context<'_>,
        interactive_id: Uuid,
    ) -> GraphQLResult<GraphQLActivationResult> {
        let auth_user = authenticated_user(ctx)?;
        activate_interactive_impl(
            app_state(ctx)?,
            auth_user.user_id,
            auth_user.is_admin,
            interactive_id,
        )
        .await
    }
}

pub(crate) async fn create_interactive_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    input: GraphQLCreateInteractiveInput,
) -> GraphQLResult<super::queries::interactives::GraphQLInteractive> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let draft = draft_from_create(&input)?;
    validate_draft(&draft, crate::interaction::registry())
        .map_err(|errors| Error::new(describe(&errors)))?;

    let scene_id = input.scene_id;
    let now = Utc::now().naive_utc();
    let interactive_id = Uuid::now_v7();
    let subject_kind = input.subject_kind.clone();
    let subject_ref = input.subject_ref;
    let geometry = input.geometry.map(|j| j.0);
    let effect_id = input.effect_id.clone();
    let effect_config = input.effect_config.map(|j| j.0);
    let trigger = input.trigger.clone();
    let activation = input.activation.clone();
    let fire_mode = input.fire_mode.unwrap_or_else(|| String::from("always"));

    let row = tokio::task::spawn_blocking(move || {
        use crate::schema::interactives;

        // 🔐 Authoring follows the world role — the Owner and any GM,
        // never a Player.
        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }

        let row: crate::models::Interactive = diesel::insert_into(interactives::table)
            .values((
                interactives::interactive_id.eq(interactive_id),
                interactives::scene_id.eq(scene_id),
                interactives::subject_kind.eq(&subject_kind),
                interactives::subject_ref.eq(subject_ref),
                interactives::geometry.eq(&geometry),
                interactives::effect_id.eq(&effect_id),
                interactives::effect_config.eq(&effect_config),
                interactives::trigger.eq(&trigger),
                interactives::activation.eq(&activation),
                interactives::fire_mode.eq(&fire_mode),
                interactives::created_by.eq(user_id),
                interactives::updated_by.eq(user_id),
                interactives::created_at.eq(now),
                interactives::updated_at.eq(now),
            ))
            .returning(crate::models::Interactive::as_returning())
            .get_result(&mut conn)?;

        announce(&mut conn, scene_id, "created", interactive_id, user_id);
        Ok(row)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to create interactive (scene not found or not yours)"))?;

    Ok(gm_view(row))
}

pub(crate) async fn update_interactive_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    interactive_id: Uuid,
    input: GraphQLUpdateInteractiveInput,
) -> GraphQLResult<super::queries::interactives::GraphQLInteractive> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let row = tokio::task::spawn_blocking(move || {
        use crate::schema::interactives;

        let existing: crate::models::Interactive = interactives::table
            .filter(interactives::interactive_id.eq(interactive_id))
            .select(crate::models::Interactive::as_select())
            .first(&mut conn)?;

        if !crate::auth::world_membership::is_dm_of_scene(
            &mut conn,
            user_id,
            is_admin,
            existing.scene_id,
        )? {
            return Err(DieselError::NotFound);
        }

        // Validate the *result*, not the patch. A partial edit that leaves
        // an interactive in a shape authoring would have refused is the
        // same invalid state, arrived at in two steps.
        let merged = merge(&existing, &input);
        let draft = draft_from_row(&merged)?;
        validate_draft(&draft, crate::interaction::registry())
            .map_err(|_| DieselError::RollbackTransaction)?;

        let clear = input.clear_effect.unwrap_or(false);
        let now = Utc::now().naive_utc();
        let row: crate::models::Interactive = diesel::update(
            interactives::table.filter(interactives::interactive_id.eq(interactive_id)),
        )
        .set((
            interactives::geometry.eq(merged.geometry.clone()),
            interactives::effect_id.eq(if clear {
                None
            } else {
                merged.effect_id.clone()
            }),
            interactives::effect_config.eq(if clear {
                None
            } else {
                merged.effect_config.clone()
            }),
            interactives::trigger.eq(merged.trigger.clone()),
            interactives::activation.eq(merged.activation.clone()),
            interactives::fire_mode.eq(merged.fire_mode.clone()),
            interactives::updated_by.eq(user_id),
            interactives::updated_at.eq(now),
        ))
        .returning(crate::models::Interactive::as_returning())
        .get_result(&mut conn)?;

        announce(&mut conn, row.scene_id, "updated", interactive_id, user_id);
        Ok(row)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to update interactive (not found, not yours, or invalid)"))?;

    Ok(gm_view(row))
}

pub(crate) async fn delete_interactive_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    interactive_id: Uuid,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::interactives;

        let scene_id: Uuid = interactives::table
            .filter(interactives::interactive_id.eq(interactive_id))
            .select(interactives::scene_id)
            .first(&mut conn)?;

        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }

        diesel::delete(interactives::table.filter(interactives::interactive_id.eq(interactive_id)))
            .execute(&mut conn)?;

        announce(&mut conn, scene_id, "deleted", interactive_id, user_id);
        Ok(true)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to delete interactive (not found or not yours)"))
}

pub(crate) async fn reset_interactive_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    interactive_id: Uuid,
) -> GraphQLResult<super::queries::interactives::GraphQLInteractive> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    let row = tokio::task::spawn_blocking(move || {
        use crate::schema::interactives;

        let scene_id: Uuid = interactives::table
            .filter(interactives::interactive_id.eq(interactive_id))
            .select(interactives::scene_id)
            .first(&mut conn)?;

        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }

        let now = Utc::now().naive_utc();
        let row: crate::models::Interactive = diesel::update(
            interactives::table.filter(interactives::interactive_id.eq(interactive_id)),
        )
        .set((
            interactives::fired_at.eq(None::<chrono::NaiveDateTime>),
            interactives::updated_by.eq(user_id),
            interactives::updated_at.eq(now),
        ))
        .returning(crate::models::Interactive::as_returning())
        .get_result(&mut conn)?;

        announce(&mut conn, scene_id, "reset", interactive_id, user_id);
        Ok(row)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to reset interactive (not found or not yours)"))?;

    Ok(gm_view(row))
}

pub(crate) async fn activate_interactive_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    interactive_id: Uuid,
) -> GraphQLResult<GraphQLActivationResult> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        let loaded = crate::interaction::load(&mut conn, interactive_id)
            .map_err(|_| Error::new("Interactive not found"))?;

        let world_id = world_id_for_scene(&mut conn, loaded.row.scene_id)?;
        let actor =
            crate::auth::world_membership::actor_in_world(&mut conn, user_id, is_admin, world_id);
        if actor.role.is_none() && !actor.is_site_admin {
            return Err(Error::new("Not a member of this world"));
        }
        let runs_the_world = actor.runs_the_world();

        let outcome = loaded.outcome(runs_the_world);
        let mut result = GraphQLActivationResult::from_outcome(outcome);

        match outcome {
            ActivationOutcome::Requested => {
                let request_id = crate::interaction::raise_request(
                    &mut conn,
                    interactive_id,
                    loaded.row.scene_id,
                    user_id,
                )
                .map_err(|e| Error::new(format!("Failed to raise request: {e}")))?;
                result.request_id = Some(request_id);
                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_INTERACTION_REQUEST,
                    Some(serde_json::json!({
                        "action": "raised",
                        "request_id": request_id,
                        "interactive_id": interactive_id,
                        "scene_id": loaded.row.scene_id,
                    })),
                    user_id,
                );
            }
            ActivationOutcome::Performed => {
                // Claim the single firing *conditionally*, so two players
                // clicking one `once` interactive resolve to one outcome
                // rather than both believing they were first (SC-005).
                if loaded.fire_mode() == FireMode::Once
                    && !claim_firing(&mut conn, interactive_id, user_id)?
                {
                    return Ok(GraphQLActivationResult::from_outcome(
                        ActivationOutcome::Refused {
                            reason:
                                thunderforge_canvas_core::interaction::RefusalReason::AlreadyFired,
                        },
                    ));
                }
                result.effect_id = loaded.row.effect_id.clone();
                result.effect_config = loaded.row.effect_config.clone().map(Json);

                // The authoritative half. The engine applies the same change
                // locally for responsiveness, but a door that only swung in
                // one browser is a door that closes again on reload.
                let changed_subject = match &loaded.row.effect_id {
                    Some(effect_id) => crate::interaction::perform(
                        &mut conn,
                        effect_id,
                        loaded
                            .row
                            .effect_config
                            .as_ref()
                            .unwrap_or(&serde_json::Value::Null),
                        loaded.row.scene_id,
                    )
                    .map_err(|e| Error::new(format!("Failed to perform effect: {e}")))?,
                    None => crate::interaction::Performed::default(),
                };
                if let Some(subject) = changed_subject.door {
                    let _ = record_world_event(
                        &mut conn,
                        world_id,
                        crate::world_events::EVENT_CODE_DOOR_CHANGED,
                        Some(serde_json::json!({
                            "action": "changed",
                            "wall_id": subject,
                            "scene_id": loaded.row.scene_id,
                        })),
                        user_id,
                    );
                }
                if runs_the_world {
                    result.notices = changed_subject.notices.clone();
                }
                if changed_subject.lights_changed {
                    // The existing light code, reused rather than replaced.
                    // A switch changing lighting and a Game Master editing a
                    // lamp are the same fact to a client: re-read the scene's
                    // lights and re-resolve shadows.
                    let _ = record_world_event(
                        &mut conn,
                        world_id,
                        crate::world_events::EVENT_CODE_LIGHT_SOURCE_CHANGED,
                        Some(serde_json::json!({
                            "action": "updated",
                            "scene_id": loaded.row.scene_id,
                        })),
                        user_id,
                    );
                }

                let _ = record_world_event(
                    &mut conn,
                    world_id,
                    EVENT_CODE_INTERACTIVE_CHANGED,
                    Some(serde_json::json!({
                        "action": "activated",
                        "interactive_id": interactive_id,
                        "scene_id": loaded.row.scene_id,
                        "effect_id": loaded.row.effect_id,
                    })),
                    user_id,
                );
            }
            // Refused, Unavailable and NoEffect change nothing and are
            // announced to nobody. An interactive that did not run is not
            // news for the rest of the table.
            _ => {}
        }

        Ok(result)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

#[path = "mutations_interactives_support.rs"]
mod support;
use support::*;

#[cfg(test)]
#[path = "mutations_interactives_tests.rs"]
mod tests;
