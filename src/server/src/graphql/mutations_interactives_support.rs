//! What the interactive mutations are built out of.
//!
//! Split out of `mutations_interactives.rs` so the `#[Object]` impl and the
//! rule it enforces stay readable on their own. Everything here is below that
//! decision, not part of it: taking a `once` firing without racing, turning
//! stored rows into drafts and back, describing validation failures, and
//! announcing what happened on the world event stream.
//!
//! None of it decides whether an activation is *allowed* — that stays with
//! the caller, from stored state, per the parent module's header.

use async_graphql::{Error, Json, Result as GraphQLResult};
use chrono::Utc;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use thunderforge_canvas_core::interaction::{
    Activation, ActivationOutcome, FireMode, InteractiveDraft, RegionGeometry, SubjectKind, Trigger,
};

use super::{
    GraphQLActivationResult, GraphQLCreateInteractiveInput, GraphQLUpdateInteractiveInput,
};
use crate::world_events::{
    EVENT_CODE_INTERACTION_REQUEST, EVENT_CODE_INTERACTIVE_CHANGED, record_world_event,
    world_id_for_scene,
};

/// Take the one firing a `once` interactive has, if it is still there.
///
/// A conditional update rather than a read-then-write: the read has already
/// happened, and between it and here another request may have fired the same
/// interactive. `WHERE fired_at IS NULL` makes the database the arbiter, which
/// is the only party both requests agree on.
pub(super) fn claim_firing(
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

pub(super) fn announce(
    conn: &mut PgConnection,
    scene_id: Uuid,
    action: &str,
    id: Uuid,
    user_id: Uuid,
) {
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

pub(super) fn gm_view(
    row: crate::models::Interactive,
) -> crate::graphql::queries::interactives::GraphQLInteractive {
    let available = crate::interaction::is_available(row.effect_id.as_deref());
    crate::graphql::queries::interactives::GraphQLInteractive {
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

pub(super) fn describe(errors: &[thunderforge_canvas_core::interaction::AuthoringError]) -> String {
    errors
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<_>>()
        .join("; ")
}

pub(super) fn parse_geometry(value: &Option<serde_json::Value>) -> Option<RegionGeometry> {
    value
        .as_ref()
        .and_then(|v| serde_json::from_value::<RegionGeometry>(v.clone()).ok())
}

pub(super) fn draft_from_create(
    input: &GraphQLCreateInteractiveInput,
) -> GraphQLResult<InteractiveDraft> {
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

pub(super) fn draft_from_row(
    row: &crate::models::Interactive,
) -> Result<InteractiveDraft, DieselError> {
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
pub(super) fn merge(
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

/// Approve or refuse one pending request.
///
/// # Why permission is re-checked here rather than trusted from the request
///
/// A request records that somebody *asked*. It does not record that they were
/// allowed, and it must not: minutes may pass between the asking and the
/// deciding, and a Game Master who locks a door and then approves a queued
/// request to open it has contradicted themselves. The lock wins, because it
/// is the more recent statement of what they want.
///
/// So approval re-runs the same resolution an activation runs, against the
/// world as it is now. The requester is the actor, not the Game Master —
/// otherwise approving would silently grant the requester the GM's
/// permissions, which is the one thing an approval flow must never do.
pub(crate) async fn decide_request_impl(
    state: &crate::state::AppState,
    user_id: Uuid,
    is_admin: bool,
    request_id: Uuid,
    approve: bool,
) -> GraphQLResult<GraphQLActivationResult> {
    let mut conn = state
        .db_pool
        .get()
        .map_err(|_| Error::new("Failed to get DB connection"))?;

    tokio::task::spawn_blocking(move || {
        use crate::schema::interaction_requests as r;

        let (interactive_id, scene_id, requested_by, current_state): (Uuid, Uuid, Uuid, String) =
            r::table
                .filter(r::request_id.eq(request_id))
                .select((r::interactive_id, r::scene_id, r::requested_by, r::state))
                .first(&mut conn)
                .map_err(|_| Error::new("Request not found"))?;

        if !crate::auth::world_membership::is_dm_of_scene(&mut conn, user_id, is_admin, scene_id)
            .map_err(|_| Error::new("Failed to check permission"))?
        {
            return Err(Error::new("Only the Game Master decides a request"));
        }

        // The requester does not decide their own request, even when the
        // requester runs the world — a Game Master's own activation never
        // queues in the first place, so a GM deciding one of their own means
        // something has gone wrong upstream.
        if requested_by == user_id {
            return Err(Error::new("A request is not decided by whoever raised it"));
        }

        if current_state != crate::interaction::REQUEST_PENDING {
            return Err(Error::new("That request has already been decided"));
        }

        let world_id = world_id_for_scene(&mut conn, scene_id)?;

        if !approve {
            crate::interaction::decide(
                &mut conn,
                request_id,
                crate::interaction::REQUEST_REFUSED,
                user_id,
            )
            .map_err(|e| Error::new(format!("Failed to refuse: {e}")))?;
            announce_request(
                &mut conn, world_id, scene_id, request_id, "refused", user_id,
            );
            return Ok(GraphQLActivationResult::from_outcome(
                thunderforge_canvas_core::interaction::ActivationOutcome::Refused {
                    reason: thunderforge_canvas_core::interaction::RefusalReason::GmOnly,
                },
            ));
        }

        // Re-resolve against the world as it is now, as the *requester*.
        let loaded = crate::interaction::load(&mut conn, interactive_id)
            .map_err(|_| Error::new("The interactive is gone"))?;
        let requester_is_gm =
            crate::auth::world_membership::actor_in_world(&mut conn, requested_by, false, world_id)
                .runs_the_world();

        // Approval is the permission being granted, so the approval mode
        // itself no longer applies — anything *else* that would refuse it
        // still does.
        let mut context = loaded.context(requester_is_gm);
        if context.activation == Activation::RequiresApproval {
            context.activation = Activation::Anyone;
        }
        let outcome = thunderforge_canvas_core::interaction::resolve_activation(context);
        let mut result = GraphQLActivationResult::from_outcome(outcome);

        // Decided either way: a request the GM acted on does not stay pending
        // because the world moved underneath it. They answered.
        crate::interaction::decide(
            &mut conn,
            request_id,
            crate::interaction::REQUEST_APPROVED,
            user_id,
        )
        .map_err(|e| Error::new(format!("Failed to approve: {e}")))?;

        if outcome == ActivationOutcome::Performed {
            if loaded.fire_mode() == FireMode::Once
                && !claim_firing(&mut conn, interactive_id, requested_by)?
            {
                result = GraphQLActivationResult::from_outcome(ActivationOutcome::Refused {
                    reason: thunderforge_canvas_core::interaction::RefusalReason::AlreadyFired,
                });
            } else {
                result.effect_id = loaded.row.effect_id.clone();
                result.effect_config = loaded.row.effect_config.clone().map(Json);
                if let Some(effect_id) = &loaded.row.effect_id {
                    let performed = crate::interaction::perform(
                        &mut conn,
                        effect_id,
                        loaded
                            .row
                            .effect_config
                            .as_ref()
                            .unwrap_or(&serde_json::Value::Null),
                        loaded.row.scene_id,
                    )
                    .map_err(|e| Error::new(format!("Failed to perform effect: {e}")))?;
                    result.notices = performed.notices.clone();
                    if let Some(subject) = performed.door {
                        let _ = record_world_event(
                            &mut conn,
                            world_id,
                            crate::world_events::EVENT_CODE_DOOR_CHANGED,
                            Some(serde_json::json!({
                                "action": "changed",
                                "wall_id": subject,
                                "scene_id": scene_id,
                            })),
                            user_id,
                        );
                    }
                    if performed.lights_changed {
                        let _ = record_world_event(
                            &mut conn,
                            world_id,
                            crate::world_events::EVENT_CODE_LIGHT_SOURCE_CHANGED,
                            Some(serde_json::json!({
                                "action": "updated",
                                "scene_id": scene_id,
                            })),
                            user_id,
                        );
                    }
                }
            }
        }

        announce_request(
            &mut conn, world_id, scene_id, request_id, "approved", user_id,
        );
        Ok(result)
    })
    .await
    .map_err(|_| Error::new("Failed to spawn blocking task"))?
}

pub(super) fn announce_request(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Uuid,
    request_id: Uuid,
    action: &str,
    user_id: Uuid,
) {
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_INTERACTION_REQUEST,
        Some(serde_json::json!({
            "action": action,
            "request_id": request_id,
            "scene_id": scene_id,
        })),
        user_id,
    );
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

pub(super) fn announce_door(conn: &mut PgConnection, scene_id: Uuid, wall_id: Uuid, user_id: Uuid) {
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
