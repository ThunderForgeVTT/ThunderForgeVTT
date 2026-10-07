//! Spec 082: who may write a shape.
//!
//! One answer for `createShape`, `updateShape`, `deleteShape` and
//! `clearShapes` (FR-006). A Game Master writes any shape in their world; a
//! player writes only the shapes they created, and only while they hold the
//! Shapes tool. Anyone else writes nothing.
//!
//! Synchronous, like `is_dm_of_scene` beside it: every shape mutation asks
//! from inside `spawn_blocking`, holding a connection.

use diesel::prelude::*;
use uuid::Uuid;

use crate::auth::authoring_tools::effective_tools_on;
use crate::auth::world_membership::is_dm_of_scene;
use crate::schema::scenes;

/// What the caller may do to a shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeAuthority {
    /// Runs the world: any shape, and its visibility.
    Dm,
    /// The shape is (or will be) theirs. It is always visible to players.
    Creator,
    /// Nothing. The caller is told the shape was not found.
    None,
}

/// What `user_id` may do to a shape on `scene_id` whose creator is
/// `created_by`, or to a new one when `created_by` is `None`.
pub fn shape_authority(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    created_by: Option<Uuid>,
) -> QueryResult<ShapeAuthority> {
    if is_dm_of_scene(conn, user_id, is_admin, scene_id)? {
        return Ok(ShapeAuthority::Dm);
    }
    let Some(world_id) = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first::<Uuid>(conn)
        .optional()?
    else {
        return Ok(ShapeAuthority::None);
    };
    let holds_shapes = effective_tools_on(conn, user_id, is_admin, world_id)?
        .iter()
        .any(|tool| tool == "shapes");
    let theirs = created_by.is_none_or(|creator| creator == user_id);
    Ok(if holds_shapes && theirs {
        ShapeAuthority::Creator
    } else {
        ShapeAuthority::None
    })
}

#[cfg(test)]
#[path = "shape_authority_tests.rs"]
mod tests;
