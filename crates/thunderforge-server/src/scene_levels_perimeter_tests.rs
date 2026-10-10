//! Spec 088 FR-092: a level given a new background is walled at the image's
//! edges, and given another, the walls move with it.

use super::tests::{Table, seat_a_table};
use super::*;
use crate::map_import::WallInsert;
use crate::map_import::perimeter::{is_marked, lies_on_bounds};

/// An image uploaded to the table's world, `w` x `h` pixels.
fn image(t: &Table, w: i32, h: i32) -> Uuid {
    use crate::schema::canvas_image_assets;
    let asset_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(canvas_image_assets::table)
        .values((
            canvas_image_assets::asset_id.eq(asset_id),
            canvas_image_assets::world_id.eq(t.world_id),
            canvas_image_assets::scene_id.eq(Some(t.scene_id)),
            canvas_image_assets::owner_user_id.eq(t.gm),
            canvas_image_assets::storage_path.eq(format!("test/{asset_id}.webp")),
            canvas_image_assets::original_format.eq("webp"),
            canvas_image_assets::width_px.eq(w),
            canvas_image_assets::height_px.eq(h),
            canvas_image_assets::byte_size.eq(1_i64),
            canvas_image_assets::kind.eq(crate::db_types::CanvasImageAssetKindEnum::Background),
            canvas_image_assets::created_by.eq(t.gm),
            canvas_image_assets::updated_by.eq(t.gm),
            canvas_image_assets::created_at.eq(now),
            canvas_image_assets::updated_at.eq(now),
        ))
        .execute(&mut t.conn())
        .expect("an image is uploaded");
    asset_id
}

fn change(t: &Table, level_id: Uuid, changes: LevelChanges) {
    update_level(&mut t.conn(), t.gm, false, level_id, changes).expect("the GM changes the level");
}

fn background(asset: Uuid) -> LevelChanges {
    LevelChanges {
        background_asset_id: Some(asset),
        wall_edges: true,
        ..LevelChanges::default()
    }
}

/// The level's walls, with their marks.
fn walls_on(t: &Table, level_id: Uuid) -> Vec<WallInsert> {
    use crate::schema::walls;
    walls::table
        .filter(walls::level_id.eq(level_id))
        .select((walls::x1, walls::y1, walls::x2, walls::y2, walls::metadata))
        .load::<(f64, f64, f64, f64, Option<serde_json::Value>)>(&mut t.conn())
        .unwrap()
        .into_iter()
        .map(|(x1, y1, x2, y2, metadata)| WallInsert {
            x1,
            y1,
            x2,
            y2,
            blocks_vision: true,
            blocks_movement: true,
            door_state: "none",
            perimeter: is_marked(metadata.as_ref()),
        })
        .collect()
}

fn all_on(walls: &[WallInsert], w: f64, h: f64) -> bool {
    walls
        .iter()
        .all(|wall| wall.perimeter && lies_on_bounds(wall, w, h))
}

#[test]
fn a_new_background_is_walled_at_the_images_edges() {
    let t = seat_a_table();
    change(&t, t.ground, background(image(&t, 800, 600)));
    let walls = walls_on(&t, t.ground);
    assert_eq!(walls.len(), 4);
    assert!(all_on(&walls, 800.0, 600.0));
}

#[test]
fn a_second_background_moves_the_edge_walls() {
    let t = seat_a_table();
    change(&t, t.ground, background(image(&t, 800, 600)));
    change(&t, t.ground, background(image(&t, 1000, 400)));
    let walls = walls_on(&t, t.ground);
    assert_eq!(walls.len(), 4, "the old set is replaced, not kept");
    assert!(all_on(&walls, 1000.0, 400.0));
}

#[test]
fn unticked_a_new_background_adds_no_walls() {
    let t = seat_a_table();
    change(
        &t,
        t.ground,
        LevelChanges {
            wall_edges: false,
            ..background(image(&t, 800, 600))
        },
    );
    assert!(walls_on(&t, t.ground).is_empty());
}

#[test]
fn clearing_the_background_or_renaming_leaves_the_walls() {
    let t = seat_a_table();
    change(&t, t.ground, background(image(&t, 800, 600)));
    change(
        &t,
        t.ground,
        LevelChanges {
            name: Some("Cellar".to_string()),
            wall_edges: true,
            ..LevelChanges::default()
        },
    );
    change(
        &t,
        t.ground,
        LevelChanges {
            clear_background: true,
            wall_edges: true,
            ..LevelChanges::default()
        },
    );
    let walls = walls_on(&t, t.ground);
    assert_eq!(walls.len(), 4);
    assert!(all_on(&walls, 800.0, 600.0));
}

#[test]
fn an_upper_floors_edges_are_its_own_and_are_announced() {
    use crate::schema::world_events;
    let t = seat_a_table();
    let loft = t.level("Loft");
    change(&t, loft, background(image(&t, 640, 640)));
    assert_eq!(walls_on(&t, loft).len(), 4);
    assert!(
        walls_on(&t, t.ground).is_empty(),
        "the ground floor is untouched"
    );

    let created = world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .filter(world_events::event_code.eq(crate::world_events::EVENT_CODE_WALL_CHANGED))
        .select(world_events::token_event)
        .load::<Option<serde_json::Value>>(&mut t.conn())
        .unwrap()
        .into_iter()
        .flatten()
        .filter(|payload| payload["action"] == "created")
        .count();
    assert_eq!(created, 4, "every client hears of each wall");
}
