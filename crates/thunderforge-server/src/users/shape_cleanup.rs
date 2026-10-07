//! A deleted account's drawings in worlds it did not create (spec 082 R9).
//!
//! `shapes.created_by` and `shapes.updated_by` reference the user with no
//! `ON DELETE` action, so a player who ever drew would otherwise be unable to
//! delete their account. A player's drawing is a scribble on someone else's
//! map and does not outlive them: it is deleted, and each live board is told
//! by the world's owner, since the account's own events go with it.

use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::schema::{scenes, shapes, worlds};
use crate::world_events::{EVENT_CODE_SHAPE_CHANGED, record_world_event};

/// Deletes `user_id`'s shapes, announcing each, and hands any shape they
/// only edited back to its creator. Answers how many were deleted. Run
/// inside the deletion's transaction, after the account's own worlds go.
pub(crate) fn forget_shapes_of(conn: &mut PgConnection, user_id: Uuid) -> QueryResult<i64> {
    let drawn: Vec<(Uuid, Uuid)> =
        diesel::delete(shapes::table.filter(shapes::created_by.eq(user_id)))
            .returning((shapes::shape_id, shapes::scene_id))
            .get_results(conn)?;
    let scene_ids: Vec<Uuid> = drawn.iter().map(|(_, scene)| *scene).collect();
    let scene_worlds: Vec<(Uuid, Uuid)> = scenes::table
        .filter(scenes::scene_id.eq_any(&scene_ids))
        .select((scenes::scene_id, scenes::world_id))
        .load(conn)?;
    let owners: Vec<(Uuid, Uuid)> = worlds::table
        .filter(worlds::id.eq_any(scene_worlds.iter().map(|(_, world)| *world)))
        .select((worlds::id, worlds::created_by))
        .load(conn)?;

    for (shape_id, scene_id) in &drawn {
        let Some((_, world_id)) = scene_worlds.iter().find(|s| s.0 == *scene_id) else {
            continue;
        };
        let Some((_, owner)) = owners.iter().find(|w| w.0 == *world_id) else {
            continue;
        };
        record_world_event(
            conn,
            *world_id,
            EVENT_CODE_SHAPE_CHANGED,
            Some(serde_json::json!({
                "action": "deleted",
                "shape_id": shape_id,
                "scene_id": scene_id,
            })),
            *owner,
        )
        .map_err(|_| DieselError::RollbackTransaction)?;
    }

    diesel::update(shapes::table.filter(shapes::updated_by.eq(user_id)))
        .set(shapes::updated_by.eq(shapes::created_by))
        .execute(conn)?;

    Ok(drawn.len() as i64)
}
