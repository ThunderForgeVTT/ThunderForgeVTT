//! Spec 088 (US2, FR-023 to FR-026): putting a base map on a new world's
//! Starting Scene.
//!
//! `createWorld` decides the map before the world exists, so an unknown id
//! is refused with nothing created, and applies it after the world's own
//! transaction has committed. The apply is the map import's own write
//! (`map_import::write_map`), fed from the prepared files instead of a
//! `.dd2vtt`: the background goes in through the storage adapter, then the
//! size, grid, walls, doors, lights and `MAP_IMPORTED` in one transaction.
//! A failure leaves the world as it was, blank, and the caller says so.

use async_graphql::{ID, MaybeUndefined};
use diesel::prelude::*;
use uuid::Uuid;

use super::{BaseMaps, Picture};
use crate::map_import::{
    MapImportError, MapWrite, PreparedBackground, StoredPlace, store_background_webp,
    store_scene_preview_webp, write_map,
};
use crate::state::AppState;

/// What `createWorld` reads when the chosen map is not on this instance.
pub const UNKNOWN_MAP: &str = "That map is not available on this instance.";

/// The non-fatal error `createWorld` adds when the world was made but its
/// map was not.
pub const STARTING_MAP_FAILED: &str = "STARTING_MAP_FAILED";
pub const STARTING_MAP_FAILED_MESSAGE: &str =
    "Your world is ready, but its map could not be added. You can import it from the scene.";

/// The map a world is to open on, from `createWorld`'s `baseMapId`:
/// - left out: the default, when this instance has it;
/// - `null`: none;
/// - an id: that map, or a refusal when this instance does not have it.
pub fn choose(maps: &BaseMaps, requested: &MaybeUndefined<ID>) -> Result<Option<String>, String> {
    match requested {
        MaybeUndefined::Undefined => Ok(maps.default_id().map(str::to_string)),
        MaybeUndefined::Null => Ok(None),
        MaybeUndefined::Value(id) => {
            let id = id.as_str().trim();
            maps.get(id)
                .map(|map| Some(map.id.clone()))
                .ok_or_else(|| UNKNOWN_MAP.to_string())
        }
    }
}

/// Puts the map on the scene. The world and scene already exist.
pub async fn apply(
    state: &AppState,
    user_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    base_map_id: &str,
) -> Result<(), MapImportError> {
    let maps = state.base_maps.clone();
    let map = maps
        .get(base_map_id)
        .ok_or_else(|| MapImportError::Io(format!("no base map {base_map_id}")))?
        .clone();
    let full = maps
        .picture_path(&map.id, Picture::Full)
        .ok_or_else(|| MapImportError::Io(format!("no image for {}", map.id)))?;
    let webp_bytes = tokio::fs::read(&full)
        .await
        .map_err(|e| MapImportError::Io(format!("{}: {e}", full.display())))?;

    let storage_cfg = crate::storage::rustfs::RustFsConfig::resolve(state).await;
    let background = store_background_webp(
        &storage_cfg,
        StoredPlace {
            owner_user_id: user_id,
            world_id,
            scene_id,
        },
        PreparedBackground {
            webp_bytes,
            original_format: "webp".to_string(),
            width: map.width,
            height: map.height,
            grid_size: i32::try_from(map.grid_size).unwrap_or(i32::MAX),
        },
        Some(state.db_pool.clone()),
    )
    .await?;

    // The thumbnail doubles as the scene's preview. Best-effort, as it is
    // for an import.
    let preview = match maps.picture_path(&map.id, Picture::Thumbnail) {
        Some(path) => match tokio::fs::read(&path).await {
            Ok(bytes) => store_scene_preview_webp(&storage_cfg, bytes).await.ok(),
            Err(_) => None,
        },
        None => None,
    };

    let (doors, walls) = map
        .walls
        .iter()
        .cloned()
        .partition(|wall| wall.door_state != "none");
    let write = MapWrite {
        world_id,
        scene_id,
        user_id,
        background,
        preview,
        walls,
        doors,
        lights: map.lights.clone(),
        ambient_light: map.ambient_light.clone(),
        source_map: None,
        base_map_id: Some(map.id.clone()),
        // A new world's scene has no edge walls to replace; the map's own
        // carry their mark from `maps.json`.
        replace_perimeter: false,
    };
    let pool = state.db_pool.clone();
    tokio::task::spawn_blocking(move || -> Result<(), MapImportError> {
        let mut conn = pool
            .get()
            .map_err(|e| MapImportError::Io(format!("Failed to get DB connection: {e}")))?;
        conn.transaction(|conn| write_map(conn, &write))
            .map_err(MapImportError::Database)
    })
    .await
    .map_err(|_| MapImportError::Io("Failed to spawn blocking task".to_string()))?
}
