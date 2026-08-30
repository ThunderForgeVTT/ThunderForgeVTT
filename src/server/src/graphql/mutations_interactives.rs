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

use thunderforge_canvas_core::interaction::{
    Activation, ActivationOutcome, EffectRegistry, FireMode, InteractiveDraft, RegionGeometry,
    SubjectKind, Trigger, validate_draft,
};

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
                    None => None,
                };
                if let Some(subject) = changed_subject {
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

/// Take the one firing a `once` interactive has, if it is still there.
///
/// A conditional update rather than a read-then-write: the read has already
/// happened, and between it and here another request may have fired the same
/// interactive. `WHERE fired_at IS NULL` makes the database the arbiter, which
/// is the only party both requests agree on.
fn claim_firing(
    conn: &mut PgConnection,
    interactive_id: Uuid,
    user_id: Uuid,
) -> Result<bool, DieselError> {
    use crate::schema::interactives;

    let now = Utc::now().naive_utc();
    let claimed = diesel::update(
        interactives::table
            .filter(interactives::interactive_id.eq(interactive_id))
            .filter(interactives::fired_at.is_null()),
    )
    .set((
        interactives::fired_at.eq(now),
        interactives::updated_by.eq(user_id),
        interactives::updated_at.eq(now),
    ))
    .execute(conn)?;
    Ok(claimed > 0)
}

fn announce(conn: &mut PgConnection, scene_id: Uuid, action: &str, id: Uuid, user_id: Uuid) {
    if let Ok(world_id) = world_id_for_scene(conn, scene_id) {
        let _ = record_world_event(
            conn,
            world_id,
            EVENT_CODE_INTERACTIVE_CHANGED,
            Some(serde_json::json!({
                "action": action,
                "interactive_id": id,
                "scene_id": scene_id,
            })),
            user_id,
        );
    }
}

fn gm_view(row: crate::models::Interactive) -> super::queries::interactives::GraphQLInteractive {
    let available = crate::interaction::is_available(row.effect_id.as_deref());
    super::queries::interactives::GraphQLInteractive {
        interactive_id: row.interactive_id,
        scene_id: row.scene_id,
        subject_kind: row.subject_kind,
        subject_ref: row.subject_ref,
        geometry: row.geometry.map(Json),
        trigger: row.trigger,
        effect_id: row.effect_id,
        effect_config: row.effect_config.map(Json),
        activation: Some(row.activation),
        fire_mode: Some(row.fire_mode),
        fired_at: row.fired_at,
        available: Some(available),
        can_activate: true,
    }
}

fn describe(errors: &[thunderforge_canvas_core::interaction::AuthoringError]) -> String {
    errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

fn parse_geometry(value: &Option<serde_json::Value>) -> Option<RegionGeometry> {
    value
        .as_ref()
        .and_then(|v| serde_json::from_value::<RegionGeometry>(v.clone()).ok())
}

fn draft_from_create(input: &GraphQLCreateInteractiveInput) -> GraphQLResult<InteractiveDraft> {
    Ok(InteractiveDraft {
        subject_kind: SubjectKind::from_str_loose(&input.subject_kind)
            .ok_or_else(|| Error::new("subjectKind must be prop, door or region"))?,
        subject_ref: input.subject_ref.map(|id| id.to_string()),
        geometry: parse_geometry(&input.geometry.as_ref().map(|j| j.0.clone())),
        effect_id: input.effect_id.clone(),
        effect_config: input
            .effect_config
            .as_ref()
            .map(|j| j.0.clone())
            .unwrap_or(serde_json::Value::Null),
        trigger: Trigger::from_str_loose(&input.trigger)
            .ok_or_else(|| Error::new("trigger must be click or enter"))?,
        activation: Activation::from_str_loose(&input.activation)
            .ok_or_else(|| Error::new("activation must be anyone, gm_only or requires_approval"))?,
        fire_mode: input
            .fire_mode
            .as_deref()
            .and_then(FireMode::from_str_loose)
            .unwrap_or_default(),
    })
}

fn draft_from_row(row: &crate::models::Interactive) -> Result<InteractiveDraft, DieselError> {
    Ok(InteractiveDraft {
        subject_kind: SubjectKind::from_str_loose(&row.subject_kind)
            .ok_or(DieselError::RollbackTransaction)?,
        subject_ref: row.subject_ref.map(|id| id.to_string()),
        geometry: parse_geometry(&row.geometry),
        effect_id: row.effect_id.clone(),
        effect_config: row.effect_config.clone().unwrap_or(serde_json::Value::Null),
        trigger: Trigger::from_str_loose(&row.trigger).ok_or(DieselError::RollbackTransaction)?,
        activation: Activation::from_str_loose(&row.activation)
            .ok_or(DieselError::RollbackTransaction)?,
        fire_mode: FireMode::from_str_loose(&row.fire_mode).unwrap_or_default(),
    })
}

/// Apply a partial edit, so the *result* can be validated rather than the patch.
fn merge(
    existing: &crate::models::Interactive,
    input: &GraphQLUpdateInteractiveInput,
) -> crate::models::Interactive {
    let clear = input.clear_effect.unwrap_or(false);
    crate::models::Interactive {
        geometry: input
            .geometry
            .as_ref()
            .map(|j| j.0.clone())
            .or_else(|| existing.geometry.clone()),
        effect_id: if clear {
            None
        } else {
            input
                .effect_id
                .clone()
                .or_else(|| existing.effect_id.clone())
        },
        effect_config: if clear {
            None
        } else {
            input
                .effect_config
                .as_ref()
                .map(|j| j.0.clone())
                .or_else(|| existing.effect_config.clone())
        },
        trigger: input
            .trigger
            .clone()
            .unwrap_or_else(|| existing.trigger.clone()),
        activation: input
            .activation
            .clone()
            .unwrap_or_else(|| existing.activation.clone()),
        fire_mode: input
            .fire_mode
            .clone()
            .unwrap_or_else(|| existing.fire_mode.clone()),
        ..existing.clone()
    }
}

/// Which door property a GM-only mutation is setting.
///
/// One function for both, because the authorization, the announcement and the
/// shape are identical and two copies would be two things to keep right.
#[derive(Debug, Clone, Copy)]
pub(crate) enum DoorFlag {
    Locked(bool),
    Secret(bool),
}

pub(crate) async fn set_door_flag_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    wall_id: Uuid,
    flag: DoorFlag,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::walls;

        let scene_id: Uuid = walls::table
            .filter(walls::wall_id.eq(wall_id))
            .select(walls::scene_id)
            .first(&mut conn)?;

        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }

        let now = Utc::now().naive_utc();
        match flag {
            DoorFlag::Locked(locked) => {
                diesel::update(walls::table.filter(walls::wall_id.eq(wall_id)))
                    .set((
                        walls::locked.eq(locked),
                        walls::updated_by.eq(user_id),
                        walls::updated_at.eq(now),
                    ))
                    .execute(&mut conn)?;
            }
            DoorFlag::Secret(secret) => {
                diesel::update(walls::table.filter(walls::wall_id.eq(wall_id)))
                    .set((
                        walls::secret.eq(secret),
                        walls::updated_by.eq(user_id),
                        walls::updated_at.eq(now),
                    ))
                    .execute(&mut conn)?;
            }
        }

        announce_door(&mut conn, scene_id, wall_id, user_id);
        Ok(true)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to change the door (not found or not yours)"))
}

pub(crate) async fn set_door_designation_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    wall_id: Uuid,
    is_door: bool,
) -> GraphQLResult<bool> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::{interactives, walls};
        use thunderforge_canvas_core::wall::{DoorState, SET_STATE, STATE_KEY, TARGET_KEY};

        let scene_id: Uuid = walls::table
            .filter(walls::wall_id.eq(wall_id))
            .select(walls::scene_id)
            .first(&mut conn)?;

        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)? {
            return Err(DieselError::NotFound);
        }

        let now = Utc::now().naive_utc();
        // A newly designated door starts closed, because a door drawn on a map
        // is a door in a wall — a wall that turned into a hole the moment it
        // became a door would change what the room does.
        let next = if is_door {
            DoorState::Closed
        } else {
            DoorState::None
        };
        diesel::update(walls::table.filter(walls::wall_id.eq(wall_id)))
            .set((
                walls::door_state.eq(next.as_str()),
                walls::updated_by.eq(user_id),
                walls::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        if is_door {
            let already: i64 = interactives::table
                .filter(interactives::subject_ref.eq(wall_id))
                .count()
                .get_result(&mut conn)?;
            if already == 0 {
                // Toggle, targeting itself: clicking the door does what a
                // person at a door does.
                let config = serde_json::json!({
                    TARGET_KEY: wall_id.to_string(),
                    STATE_KEY: "toggle",
                });
                diesel::insert_into(interactives::table)
                    .values((
                        interactives::interactive_id.eq(Uuid::now_v7()),
                        interactives::scene_id.eq(scene_id),
                        interactives::subject_kind.eq("door"),
                        interactives::subject_ref.eq(wall_id),
                        interactives::effect_id.eq(SET_STATE),
                        interactives::effect_config.eq(&config),
                        interactives::trigger.eq("click"),
                        interactives::activation.eq("anyone"),
                        interactives::fire_mode.eq("always"),
                        interactives::created_by.eq(user_id),
                        interactives::updated_by.eq(user_id),
                        interactives::created_at.eq(now),
                        interactives::updated_at.eq(now),
                    ))
                    .execute(&mut conn)?;
            }
        } else {
            // Undesignating removes what the designation created. A door on a
            // wall that is no longer a door is not a thing.
            diesel::delete(interactives::table.filter(interactives::subject_ref.eq(wall_id)))
                .execute(&mut conn)?;
        }

        announce_door(&mut conn, scene_id, wall_id, user_id);
        Ok(true)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
    .map_err(|_| Error::new("Failed to designate the door (not found or not yours)"))
}

fn announce_door(conn: &mut PgConnection, scene_id: Uuid, wall_id: Uuid, user_id: Uuid) {
    if let Ok(world_id) = world_id_for_scene(conn, scene_id) {
        let _ = record_world_event(
            conn,
            world_id,
            crate::world_events::EVENT_CODE_DOOR_CHANGED,
            Some(serde_json::json!({
                "action": "changed",
                "wall_id": wall_id,
                "scene_id": scene_id,
            })),
            user_id,
        );
    }
}

/// Assemble a registry from an explicit contribution set.
///
/// Exposed for the seam's own tests (US7), which need to build a registry
/// *without* a contributor and confirm everything else still works.
pub fn registry_from(
    contributions: Vec<Vec<thunderforge_canvas_core::interaction::EffectDeclaration>>,
) -> EffectRegistry {
    EffectRegistry::assemble(contributions).expect("test contributions collide")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    /// A world with a Game Master and a player, and one scene.
    struct Table {
        state: crate::state::AppState,
        gm: Uuid,
        player: Uuid,
        scene_id: Uuid,
    }

    fn seat_a_table() -> Table {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let gm = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, gm);
        let scene_id = insert_test_scene(&mut conn, world_id, gm);
        let player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world_id, player, "Player");
        drop(conn);
        Table {
            state,
            gm,
            player,
            scene_id,
        }
    }

    fn a_prop(scene_id: Uuid, subject: Uuid) -> GraphQLCreateInteractiveInput {
        GraphQLCreateInteractiveInput {
            scene_id,
            subject_kind: String::from("prop"),
            subject_ref: Some(subject),
            geometry: None,
            effect_id: None,
            effect_config: None,
            trigger: String::from("click"),
            activation: String::from("anyone"),
            fire_mode: None,
        }
    }

    #[tokio::test]
    async fn a_player_cannot_author_edit_delete_or_reset_an_interactive() {
        // FR-005, Principle III. Every one of these is refused *here*, at the
        // data boundary, rather than by a client that declined to show the
        // authoring panel.
        let t = seat_a_table();
        let subject = Uuid::now_v7();

        let refused =
            create_interactive_impl(&t.state, t.player, false, a_prop(t.scene_id, subject)).await;
        assert!(refused.is_err(), "a player must not author an interactive");

        // The Game Master's identical call succeeds, so the refusal above is
        // about who asked and not about the request being malformed.
        let created = create_interactive_impl(&t.state, t.gm, false, a_prop(t.scene_id, subject))
            .await
            .expect("the Game Master authors it");
        let id = created.interactive_id;

        let edited = update_interactive_impl(
            &t.state,
            t.player,
            false,
            id,
            GraphQLUpdateInteractiveInput {
                geometry: None,
                effect_id: None,
                effect_config: None,
                trigger: None,
                activation: Some(String::from("gm_only")),
                fire_mode: None,
                clear_effect: None,
            },
        )
        .await;
        assert!(edited.is_err(), "a player must not edit an interactive");

        let reset = reset_interactive_impl(&t.state, t.player, false, id).await;
        assert!(reset.is_err(), "a player must not reset an interactive");

        let deleted = delete_interactive_impl(&t.state, t.player, false, id).await;
        assert!(deleted.is_err(), "a player must not delete an interactive");

        // And it is still there, because a refused delete must not half-happen.
        let mut conn = t.state.db_pool.get().unwrap();
        assert!(crate::interaction::load(&mut conn, id).is_ok());

        let _ = delete_interactive_impl(&t.state, t.gm, false, id).await;
    }

    #[tokio::test]
    async fn scenery_activates_to_no_effect_rather_than_to_an_error() {
        // An interactive with no effect is a legitimate thing to place (US1
        // scenario 3). A player clicking it should be told nothing happened,
        // not shown a failure.
        let t = seat_a_table();
        let created =
            create_interactive_impl(&t.state, t.gm, false, a_prop(t.scene_id, Uuid::now_v7()))
                .await
                .expect("the Game Master places a table");

        let result = activate_interactive_impl(&t.state, t.player, false, created.interactive_id)
            .await
            .expect("clicking scenery is not an error");
        assert_eq!(result.outcome, "noEffect");
        assert!(result.reason.is_none());

        let _ = delete_interactive_impl(&t.state, t.gm, false, created.interactive_id).await;
    }

    #[tokio::test]
    async fn a_stranger_cannot_activate_anything_in_a_world_they_are_not_in() {
        let t = seat_a_table();
        let created =
            create_interactive_impl(&t.state, t.gm, false, a_prop(t.scene_id, Uuid::now_v7()))
                .await
                .expect("authored");

        let stranger = {
            let mut conn = t.state.db_pool.get().unwrap();
            insert_test_user(&mut conn)
        };
        let refused =
            activate_interactive_impl(&t.state, stranger, false, created.interactive_id).await;
        assert!(
            refused.is_err(),
            "membership is checked before anything else"
        );

        let _ = delete_interactive_impl(&t.state, t.gm, false, created.interactive_id).await;
    }

    #[tokio::test]
    async fn an_effect_no_contributor_declares_is_refused_at_authoring_time() {
        // The other half of FR-041: an interactive can only *become* stale, it
        // cannot be authored stale.
        let t = seat_a_table();
        let mut input = a_prop(t.scene_id, Uuid::now_v7());
        input.effect_id = Some(String::from("audio.play"));

        let refused = create_interactive_impl(&t.state, t.gm, false, input).await;
        assert!(
            refused.is_err(),
            "no audio subsystem exists, so nothing may be authored against it"
        );
    }

    /// A wall on the table's scene, closed and unlocked.
    fn a_wall(t: &Table) -> Uuid {
        use crate::schema::walls;
        let mut conn = t.state.db_pool.get().unwrap();
        let wall_id = Uuid::now_v7();
        let now = Utc::now().naive_utc();
        diesel::insert_into(walls::table)
            .values((
                walls::wall_id.eq(wall_id),
                walls::scene_id.eq(t.scene_id),
                walls::x1.eq(0.0f64),
                walls::y1.eq(0.0f64),
                walls::x2.eq(100.0f64),
                walls::y2.eq(0.0f64),
                walls::blocks_vision.eq(true),
                walls::blocks_movement.eq(true),
                walls::door_state.eq("closed"),
                walls::created_by.eq(t.gm),
                walls::updated_by.eq(t.gm),
                walls::created_at.eq(now),
                walls::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("insert wall");
        wall_id
    }

    fn door_state_of(t: &Table, wall_id: Uuid) -> String {
        use crate::schema::walls;
        let mut conn = t.state.db_pool.get().unwrap();
        walls::table
            .filter(walls::wall_id.eq(wall_id))
            .select(walls::door_state)
            .first(&mut conn)
            .expect("wall exists")
    }

    /// The interactive `setDoorDesignation` creates for a door.
    async fn designate(t: &Table, wall_id: Uuid) -> Uuid {
        use crate::schema::interactives;
        set_door_designation_impl(&t.state, t.gm, false, wall_id, true)
            .await
            .expect("the Game Master designates a door");
        let mut conn = t.state.db_pool.get().unwrap();
        interactives::table
            .filter(interactives::subject_ref.eq(wall_id))
            .select(interactives::interactive_id)
            .first(&mut conn)
            .expect("designating a door gives it an interactive")
    }

    #[tokio::test]
    async fn a_player_cannot_open_a_locked_door_at_the_server() {
        // The rule most likely to be implemented by not drawing the button.
        // A screen test would pass against a server that happily performs the
        // change when asked directly, which is why this asks directly.
        let t = seat_a_table();
        let wall_id = a_wall(&t);
        let interactive_id = designate(&t, wall_id).await;

        // Unlocked: the player opens it, and the change is durable.
        let opened = activate_interactive_impl(&t.state, t.player, false, interactive_id)
            .await
            .expect("an unlocked door opens");
        assert_eq!(opened.outcome, "performed");
        assert_eq!(door_state_of(&t, wall_id), "open");

        set_door_flag_impl(&t.state, t.gm, false, wall_id, DoorFlag::Locked(true))
            .await
            .expect("the Game Master locks it");

        let refused = activate_interactive_impl(&t.state, t.player, false, interactive_id)
            .await
            .expect("a refusal is an outcome, not an error");
        assert_eq!(refused.outcome, "refused");
        assert_eq!(refused.reason.as_deref(), Some("locked"));
        // And nothing moved. A refusal that still performed the effect would
        // pass an outcome assertion and fail the table.
        assert_eq!(door_state_of(&t, wall_id), "open");

        let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
    }

    #[tokio::test]
    async fn a_game_master_can_still_change_a_locked_door() {
        // FR-013. The lock is theirs; it is not a rule against them.
        let t = seat_a_table();
        let wall_id = a_wall(&t);
        let interactive_id = designate(&t, wall_id).await;

        set_door_flag_impl(&t.state, t.gm, false, wall_id, DoorFlag::Locked(true))
            .await
            .expect("locked");

        let performed = activate_interactive_impl(&t.state, t.gm, false, interactive_id)
            .await
            .expect("the Game Master opens their own locked door");
        assert_eq!(performed.outcome, "performed");
        assert_eq!(door_state_of(&t, wall_id), "open");

        let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
    }

    #[tokio::test]
    async fn a_player_cannot_lock_designate_or_reveal_a_door() {
        let t = seat_a_table();
        let wall_id = a_wall(&t);

        assert!(
            set_door_designation_impl(&t.state, t.player, false, wall_id, true)
                .await
                .is_err(),
            "a player must not designate a door"
        );
        assert!(
            set_door_flag_impl(&t.state, t.player, false, wall_id, DoorFlag::Locked(true))
                .await
                .is_err(),
            "a player must not lock a door"
        );
        assert!(
            set_door_flag_impl(&t.state, t.player, false, wall_id, DoorFlag::Secret(true))
                .await
                .is_err(),
            "a player must not hide a door"
        );
    }

    #[tokio::test]
    async fn toggling_a_door_twice_returns_it_to_where_it_started() {
        let t = seat_a_table();
        let wall_id = a_wall(&t);
        let interactive_id = designate(&t, wall_id).await;

        assert_eq!(door_state_of(&t, wall_id), "closed");
        let _ = activate_interactive_impl(&t.state, t.player, false, interactive_id).await;
        assert_eq!(door_state_of(&t, wall_id), "open");
        let _ = activate_interactive_impl(&t.state, t.player, false, interactive_id).await;
        assert_eq!(door_state_of(&t, wall_id), "closed");

        let _ = delete_interactive_impl(&t.state, t.gm, false, interactive_id).await;
    }

    #[tokio::test]
    async fn undesignating_a_door_takes_its_interactive_with_it() {
        // A door on a wall that is no longer a door is not a thing, and an
        // interactive left behind would be a click that does nothing.
        use crate::schema::interactives;
        let t = seat_a_table();
        let wall_id = a_wall(&t);
        designate(&t, wall_id).await;

        set_door_designation_impl(&t.state, t.gm, false, wall_id, false)
            .await
            .expect("undesignated");

        let mut conn = t.state.db_pool.get().unwrap();
        let remaining: i64 = interactives::table
            .filter(interactives::subject_ref.eq(wall_id))
            .count()
            .get_result(&mut conn)
            .unwrap();
        assert_eq!(remaining, 0);
        assert_eq!(door_state_of(&t, wall_id), "none");
    }

    #[tokio::test]
    async fn a_prop_cannot_be_authored_with_an_entry_trigger() {
        let t = seat_a_table();
        let mut input = a_prop(t.scene_id, Uuid::now_v7());
        input.trigger = String::from("enter");

        let refused = create_interactive_impl(&t.state, t.gm, false, input).await;
        assert!(refused.is_err(), "a book cannot be crossed");
    }
}
