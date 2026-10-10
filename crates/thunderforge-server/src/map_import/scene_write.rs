//! The database half of putting a map on a scene: walls, doors, lights, the
//! background's asset row, the scene's size, grid and light, its preview,
//! and the one `MAP_IMPORTED` event that tells every client.
//!
//! Shared by the `.dd2vtt` import and by a new world's base map (spec 088,
//! US2), so a base map arrives exactly as an imported one does. The caller
//! has already stored the pictures and opens the transaction.

use chrono::Utc;
use diesel::prelude::*;
use serde_json::json;
use uuid::Uuid;

use super::geometry::{LightInsert, WallInsert};
use super::image::{SavedBackgroundImage, SavedScenePreview};
use crate::world_events::{EVENT_CODE_MAP_IMPORTED, record_world_event};

/// What the source file said the map is. See `alignment::record_source_map`.
#[derive(Debug, Clone, Copy)]
pub struct SourceMap {
    pub cells_x: f64,
    pub cells_y: f64,
    pub pixels_per_grid: f64,
}

/// Everything one map puts on one scene.
pub struct MapWrite {
    pub world_id: Uuid,
    pub scene_id: Uuid,
    pub user_id: Uuid,
    pub background: SavedBackgroundImage,
    pub preview: Option<SavedScenePreview>,
    pub walls: Vec<WallInsert>,
    pub doors: Vec<WallInsert>,
    pub lights: Vec<LightInsert>,
    pub ambient_light: String,
    /// Recorded on the scene when known; a base map has no file to quote.
    pub source_map: Option<SourceMap>,
    /// Spec 088 (FR-029): the base map the background came from, so the
    /// scene can carry its credit. `None` for a GM's own file.
    pub base_map_id: Option<String>,
}

/// Writes the map. Run it inside a transaction.
pub fn write_map(conn: &mut PgConnection, write: &MapWrite) -> QueryResult<()> {
    use crate::schema::{canvas_image_assets, light_sources, scenes, walls as walls_table};

    let MapWrite {
        world_id,
        scene_id,
        user_id,
        background,
        ..
    } = write;
    let (world_id, scene_id, user_id) = (*world_id, *scene_id, *user_id);
    let now = Utc::now().naive_utc();

    for wall in write.walls.iter().chain(write.doors.iter()) {
        diesel::insert_into(walls_table::table)
            .values((
                walls_table::wall_id.eq(Uuid::now_v7()),
                walls_table::scene_id.eq(scene_id),
                walls_table::x1.eq(wall.x1),
                walls_table::y1.eq(wall.y1),
                walls_table::x2.eq(wall.x2),
                walls_table::y2.eq(wall.y2),
                walls_table::blocks_vision.eq(wall.blocks_vision),
                walls_table::blocks_movement.eq(wall.blocks_movement),
                walls_table::door_state.eq(wall.door_state),
                walls_table::created_by.eq(user_id),
                walls_table::updated_by.eq(user_id),
                walls_table::created_at.eq(now),
                walls_table::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    for light in &write.lights {
        diesel::insert_into(light_sources::table)
            .values((
                light_sources::light_id.eq(Uuid::now_v7()),
                light_sources::scene_id.eq(scene_id),
                light_sources::x.eq(light.x),
                light_sources::y.eq(light.y),
                light_sources::radius.eq(light.radius),
                // A dd2vtt light has one range; it looks as a light with one
                // radius always has (spec 045 FR-062).
                light_sources::bright_radius.eq(light.radius * 0.5),
                light_sources::intensity.eq(light.intensity),
                light_sources::color.eq(&light.color),
                light_sources::attached_token_id.eq(None::<Uuid>),
                light_sources::casts_shadows.eq(light.casts_shadows),
                light_sources::metadata.eq(None::<serde_json::Value>),
                light_sources::created_by.eq(user_id),
                light_sources::updated_by.eq(user_id),
                light_sources::created_at.eq(now),
                light_sources::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    diesel::insert_into(canvas_image_assets::table)
        .values((
            canvas_image_assets::asset_id.eq(background.asset_id),
            canvas_image_assets::world_id.eq(world_id),
            canvas_image_assets::scene_id.eq(Some(scene_id)),
            canvas_image_assets::owner_user_id.eq(user_id),
            canvas_image_assets::storage_path.eq(&background.storage_path),
            canvas_image_assets::original_format.eq(&background.original_format),
            canvas_image_assets::width_px.eq(background.width_px),
            canvas_image_assets::height_px.eq(background.height_px),
            canvas_image_assets::byte_size.eq(background.byte_size),
            // Without this the row is NULL here and the background is
            // permanently uncacheable; see `SavedBackgroundImage`.
            canvas_image_assets::content_hash.eq(Some(background.content_hash.clone())),
            canvas_image_assets::kind.eq(crate::db_types::CanvasImageAssetKindEnum::Background),
            canvas_image_assets::base_map_id.eq(write.base_map_id.as_deref()),
            canvas_image_assets::created_by.eq(user_id),
            canvas_image_assets::updated_by.eq(user_id),
            canvas_image_assets::created_at.eq(now),
            canvas_image_assets::updated_at.eq(now),
        ))
        .execute(conn)?;

    // `width`/`height` are the stored art's real pixel size, not whatever
    // the scene was created with: the engine sizes the background sprite
    // from them (`systems/background.rs`), so a new scene's 100x100 would
    // draw a 6144x3456 map as a 100-unit sliver. `grid_size` is the stored
    // image's own cell size, so the grid lines up 1:1 with the art.
    let existing_metadata: Option<serde_json::Value> = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::metadata)
        .first::<Option<serde_json::Value>>(conn)?;
    // What the file said the map is, so a later disagreement between the
    // grid and the background is answerable at all. Without it the worst
    // case is undetectable: 4096/128 is exactly 32 and 2304/128 exactly 18,
    // so a scene that is uniformly 1.5x wrong looks self-consistent.
    let metadata = match write.source_map {
        Some(source) => Some(super::alignment::record_source_map(
            existing_metadata,
            source.cells_x,
            source.cells_y,
            source.pixels_per_grid,
        )),
        None => existing_metadata,
    };

    diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
        .set((
            scenes::background_asset_id.eq(background.asset_id),
            scenes::grid_size.eq(background.grid_size),
            scenes::grid_type.eq("square"),
            scenes::width.eq(background.width_px),
            scenes::height.eq(background.height_px),
            scenes::ambient_light.eq(&write.ambient_light),
            // The scene's art and size just changed; nothing else bumps this
            // (no trigger on `scenes`), and it is what tells anyone reading
            // the row that it moved.
            scenes::updated_at.eq(now),
            scenes::metadata.eq(metadata),
        ))
        .execute(conn)?;

    if let Some(preview) = &write.preview {
        use crate::schema::scene_preview_images;
        diesel::insert_into(scene_preview_images::table)
            .values((
                scene_preview_images::id.eq(preview.asset_id),
                scene_preview_images::scene_id.eq(scene_id),
                scene_preview_images::byte_size.eq(preview.byte_size),
                scene_preview_images::created_at.eq(now),
            ))
            .execute(conn)?;
        diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
            .set(scenes::preview_asset_id.eq(preview.asset_id))
            .execute(conn)?;
    }

    // T027: best-effort NOTIFY for the whole batch — do not fail the import
    // if this fails.
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_MAP_IMPORTED,
        Some(json!({
            "scene_id": scene_id,
            "walls_created": write.walls.len(),
            "doors_created": write.doors.len(),
            "lights_created": write.lights.len(),
            "background_image_set": true,
        })),
        user_id,
    );

    Ok(())
}
