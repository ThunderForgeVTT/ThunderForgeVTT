//! Spec 088 FR-092: a level's new background brings its edge walls, and
//! takes the old background's with it. Run inside the level's transaction.

use diesel::prelude::*;
use uuid::Uuid;

use crate::map_import::perimeter::{perimeter_metadata, perimeter_walls};
use crate::map_import::{ScenePlacement, WallInsert, stale_perimeter};
use crate::world_events::{EVENT_CODE_WALL_CHANGED, record_world_event};

/// An uploaded image's size in pixels.
fn image_size(conn: &mut PgConnection, asset_id: Uuid) -> QueryResult<(f64, f64)> {
    use crate::schema::canvas_image_assets;
    let (w, h) = canvas_image_assets::table
        .filter(canvas_image_assets::asset_id.eq(asset_id))
        .select((
            canvas_image_assets::width_px,
            canvas_image_assets::height_px,
        ))
        .first::<(i32, i32)>(conn)?;
    Ok((f64::from(w), f64::from(h)))
}

/// The level's board was `old_size` (and `old_asset`'s image, if it had
/// one); it is now `new_asset`'s image. Delete the edge walls still on the
/// old bounds, wall the new image's edges where the level's other walls
/// leave them open, and tell every client about each wall.
pub(super) fn replace_level_perimeter(
    conn: &mut PgConnection,
    user_id: Uuid,
    scene_id: Uuid,
    level_id: Uuid,
    old_size: (i32, i32),
    old_asset: Option<Uuid>,
    new_asset: Uuid,
) -> QueryResult<()> {
    use crate::schema::{scenes, walls};

    let mut old_bounds = vec![(f64::from(old_size.0), f64::from(old_size.1))];
    if let Some(asset) = old_asset {
        old_bounds.push(image_size(conn, asset)?);
    }
    let (width, height) = image_size(conn, new_asset)?;
    let world_id: Uuid = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first(conn)?;
    let announce = |conn: &mut PgConnection, action: &str, wall_id: Uuid| {
        let _ = record_world_event(
            conn,
            world_id,
            EVENT_CODE_WALL_CHANGED,
            Some(serde_json::json!({
                "action": action,
                "wall_id": wall_id,
                "scene_id": scene_id,
            })),
            user_id,
        );
    };

    let stale = stale_perimeter(conn, scene_id, level_id, &old_bounds)?;
    if !stale.is_empty() {
        diesel::delete(walls::table.filter(walls::wall_id.eq_any(&stale))).execute(conn)?;
        for wall_id in stale {
            announce(conn, "deleted", wall_id);
        }
    }

    let others: Vec<WallInsert> = walls::table
        .filter(walls::level_id.eq(level_id))
        .select((walls::x1, walls::y1, walls::x2, walls::y2))
        .load::<(f64, f64, f64, f64)>(conn)?
        .into_iter()
        .map(|(x1, y1, x2, y2)| WallInsert {
            x1,
            y1,
            x2,
            y2,
            blocks_vision: true,
            blocks_movement: true,
            door_state: "none",
            perimeter: false,
        })
        .collect();
    let placement = ScenePlacement {
        // Only the rectangle matters to the edges.
        grid_size: 0.0,
        width,
        height,
    };
    let now = chrono::Utc::now().naive_utc();
    for wall in perimeter_walls(&placement, &others) {
        let wall_id = Uuid::now_v7();
        diesel::insert_into(walls::table)
            .values((
                walls::wall_id.eq(wall_id),
                walls::scene_id.eq(scene_id),
                walls::level_id.eq(level_id),
                walls::x1.eq(wall.x1),
                walls::y1.eq(wall.y1),
                walls::x2.eq(wall.x2),
                walls::y2.eq(wall.y2),
                walls::blocks_vision.eq(wall.blocks_vision),
                walls::blocks_movement.eq(wall.blocks_movement),
                walls::door_state.eq(wall.door_state),
                walls::metadata.eq(Some(perimeter_metadata())),
                walls::created_by.eq(user_id),
                walls::updated_by.eq(user_id),
                walls::created_at.eq(now),
                walls::updated_at.eq(now),
            ))
            .execute(conn)?;
        announce(conn, "created", wall_id);
    }
    Ok(())
}
