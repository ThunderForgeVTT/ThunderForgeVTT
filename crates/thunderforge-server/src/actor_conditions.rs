//! The conditions a character is under (spec 067 Story 4).
//!
//! # The shape
//!
//! The *system* says which conditions exist and what the board draws for
//! each: its manifest's `conditions` block, read by
//! `pack_system_spec::conditions`. The *actor* holds the ones it is under:
//! rows in `world_actor_conditions`. This module joins the two, and it is the
//! only code that touches the table.
//!
//! A condition lives on the actor, not the token. It follows the character
//! from scene to scene and is drawn on every token of theirs; a token with no
//! actor carries none.
//!
//! # The invariant everything here rests on
//!
//! **A condition that is not declared does not exist.** A stored id the
//! manifest of the world's system does not declare is never handed to a
//! client, and it is not deleted either: a world that changes system and
//! changes back finds its characters as they were left. This is
//! `world_system_settings`' rule, for the same reason.
//!
//! # Who is told
//!
//! Nothing here decides that. Conditions ride on the token a viewer is sent
//! (`graphql::token_art`), so they reach exactly the clients that token
//! reaches. A change is announced with ids only, and each client reads again.

use std::collections::HashMap;

use diesel::prelude::*;
use uuid::Uuid;

use pack_system_spec::conditions::{SystemCondition, conditions_from_manifest};

use crate::schema::{tokens, world_actor_conditions, world_actors, worlds};
use crate::world_events::{
    EVENT_CODE_ACTOR_SHEET_CHANGED, EVENT_CODE_TOKEN_CHANGED, record_world_event,
};
use crate::world_system_settings::manifest_of_system;

/// The conditions a system declares, in the order it declares them. A system
/// with no manifest, or no `conditions` block, declares none.
pub fn declarations_for_system(systems_dir: &str, system_id: &str) -> Vec<SystemCondition> {
    manifest_of_system(systems_dir, system_id)
        .map(|manifest| conditions_from_manifest(&manifest))
        .unwrap_or_default()
}

/// The declared conditions each of these actors is under, in manifest order.
/// An actor under none is absent. One query for any number of actors, and no
/// manifest is read unless one of them holds something.
pub fn held_by_actors(
    conn: &mut PgConnection,
    systems_dir: &str,
    actor_ids: &[Uuid],
) -> QueryResult<HashMap<Uuid, Vec<SystemCondition>>> {
    if actor_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows: Vec<(Uuid, String, Option<String>)> = world_actor_conditions::table
        .inner_join(world_actors::table.inner_join(worlds::table))
        .filter(world_actor_conditions::actor_id.eq_any(actor_ids))
        .select((
            world_actor_conditions::actor_id,
            world_actor_conditions::condition_id,
            worlds::game_system_id,
        ))
        .load(conn)?;

    let mut stored: HashMap<Uuid, (String, Vec<String>)> = HashMap::new();
    for (actor_id, condition_id, system_id) in rows {
        let Some(system_id) = system_id else {
            continue;
        };
        stored
            .entry(actor_id)
            .or_insert_with(|| (system_id, Vec::new()))
            .1
            .push(condition_id);
    }

    let mut declared: HashMap<String, Vec<SystemCondition>> = HashMap::new();
    let mut held = HashMap::new();
    for (actor_id, (system_id, ids)) in stored {
        let declarations = declared
            .entry(system_id)
            .or_insert_with_key(|system_id| declarations_for_system(systems_dir, system_id));
        let conditions: Vec<SystemCondition> = declarations
            .iter()
            .filter(|declaration| ids.contains(&declaration.id))
            .cloned()
            .collect();
        if !conditions.is_empty() {
            held.insert(actor_id, conditions);
        }
    }
    Ok(held)
}

/// The declared conditions one actor is under, in manifest order.
pub fn held_by_actor(
    conn: &mut PgConnection,
    systems_dir: &str,
    actor_id: Uuid,
) -> QueryResult<Vec<SystemCondition>> {
    Ok(held_by_actors(conn, systems_dir, &[actor_id])?
        .remove(&actor_id)
        .unwrap_or_default())
}

/// Put an actor under a condition. Applying one it is already under changes
/// nothing. The caller has checked that the condition is declared.
pub fn apply(
    conn: &mut PgConnection,
    actor_id: Uuid,
    condition_id: &str,
    applied_by: Uuid,
) -> QueryResult<()> {
    diesel::insert_into(world_actor_conditions::table)
        .values((
            world_actor_conditions::actor_id.eq(actor_id),
            world_actor_conditions::condition_id.eq(condition_id),
            world_actor_conditions::applied_by.eq(Some(applied_by)),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}

/// Lift a condition from an actor. Clearing one it is not under changes
/// nothing — and an id nothing declares may be cleared, which is how a row
/// left behind by an older manifest is removed.
pub fn clear(conn: &mut PgConnection, actor_id: Uuid, condition_id: &str) -> QueryResult<()> {
    diesel::delete(
        world_actor_conditions::table
            .filter(world_actor_conditions::actor_id.eq(actor_id))
            .filter(world_actor_conditions::condition_id.eq(condition_id)),
    )
    .execute(conn)?;
    Ok(())
}

/// Tell every seat to read this actor's conditions again: the sheet
/// (actor-sheet-changed) and the board (token-changed, once per scene one of
/// its tokens stands on). Ids only, never a condition — what a client may
/// know is decided by the read it makes next. Best-effort: a lost
/// announcement costs a client one read.
pub fn announce_changed(conn: &mut PgConnection, world_id: Uuid, actor_id: Uuid, user_id: Uuid) {
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_ACTOR_SHEET_CHANGED,
        Some(serde_json::json!({
            "action": "changed",
            "actorId": actor_id,
            "dataType": "conditions",
        })),
        user_id,
    );

    let placed = tokens::table
        .filter(tokens::actor_id.eq(actor_id))
        .select((tokens::token_id, tokens::scene_id))
        .load::<(Uuid, Uuid)>(conn)
        .unwrap_or_default();
    let mut announced = std::collections::HashSet::new();
    for (token_id, scene_id) in placed {
        if !announced.insert(scene_id) {
            continue;
        }
        let _ = record_world_event(
            conn,
            world_id,
            EVENT_CODE_TOKEN_CHANGED,
            Some(serde_json::json!({
                "action": "updated",
                "token_id": token_id,
                "scene_id": scene_id,
            })),
            user_id,
        );
    }
}
