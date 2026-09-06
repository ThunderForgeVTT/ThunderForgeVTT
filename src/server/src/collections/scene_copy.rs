//! Spec 026 T025: copying a scene, and deciding what a scene *is*.
//!
//! Nothing in this product had ever duplicated a scene before this — not
//! within a world, not across them. So the question "what comes with a scene"
//! had no precedent to follow and is answered here.
//!
//! # What is copied, and what is not
//!
//! | Table | Copied | Why |
//! |---|---|---|
//! | `scenes` | yes | the place itself |
//! | `walls` | yes | the shape of the place — SC-008a names it |
//! | `light_sources` | yes | how the place is lit — SC-008a names it |
//! | `shapes` | yes | drawn scenery, by the same argument |
//! | background asset | yes, **as a new row on the same `storage_path`** | see below |
//! | `tokens` | **no** | placed actors mid-play, not scenery |
//! | `fog_masks` | **no** | per-session play state: what one table has explored |
//! | `interactives` | **no** | wired to the source world's content |
//!
//! The line is *a place versus a session*. Copying tokens would drag actor
//! rows in as a side effect the collection's owner never chose, which
//! FR-014/FR-015 reject in favour of a declared loss. Copying fog would hand a
//! recipient someone else's exploration. Each omission becomes a fidelity
//! note (FR-015) rather than a silence.
//!
//! # The background image
//!
//! A **new `canvas_image_assets` row in the destination world, pointing at the
//! same `storage_path`**. This is the designed use of `storage/dedupe.rs`,
//! which states that each asset keeps its own row with its own `asset_id`,
//! `world_id`, `scene_id` and owner, that only `storage_path` is shared, and
//! that `assets_serve::canvas` authorises against the row it looked up. Two
//! worlds pointing at one object are still two independent permission checks.
//!
//! That satisfies three requirements at once: FR-019 and SC-008 (no additional
//! stored bytes), and FR-018 (the copy depends on the *object*, which is
//! instance-wide, not on the source world continuing to exist).
//!
//! **Nothing here deletes a stored object**, and nothing in this feature does.
//! `dedupe.rs` is explicit that a shared path is safe only while nothing
//! deletes, and that adding deletion means adding reference counting first.
//! Collections must not be the feature that quietly breaks that.

use diesel::prelude::*;
use uuid::Uuid;

use super::copy::{CopyContext, CopyError};

pub fn copy_scene(
    conn: &mut PgConnection,
    ctx: &mut CopyContext,
    source_id: Uuid,
) -> Result<(), CopyError> {
    use crate::schema::{fog_masks, interactives, light_sources, scenes, shapes, tokens, walls};

    let (
        name,
        description,
        type_,
        grid_size,
        grid_type,
        width,
        height,
        metadata,
        background_image_path,
        background_asset_id,
        summary_markdown,
        summary_rendered_html,
    ) = scenes::table
        .filter(scenes::scene_id.eq(source_id))
        .select((
            scenes::name,
            scenes::description,
            scenes::type_,
            scenes::grid_size,
            scenes::grid_type,
            scenes::width,
            scenes::height,
            scenes::metadata,
            scenes::background_image_path,
            scenes::background_asset_id,
            scenes::summary_markdown,
            scenes::summary_rendered_html,
        ))
        .first::<(
            String,
            Option<String>,
            String,
            i32,
            String,
            i32,
            i32,
            Option<serde_json::Value>,
            Option<String>,
            Option<Uuid>,
            Option<String>,
            Option<String>,
        )>(conn)?;

    // `UNIQUE (world_id, name)`: copying twice into one world, or into the
    // world it came from, must produce two scenes rather than a conflict.
    let name = unique_scene_name(conn, ctx.destination_world_id, &name)?;

    let new_scene_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();

    diesel::insert_into(scenes::table)
        .values((
            scenes::scene_id.eq(new_scene_id),
            scenes::world_id.eq(ctx.destination_world_id),
            scenes::name.eq(&name),
            scenes::description.eq(&description),
            scenes::type_.eq(type_),
            scenes::grid_size.eq(grid_size),
            scenes::grid_type.eq(grid_type),
            scenes::width.eq(width),
            scenes::height.eq(height),
            scenes::metadata.eq(metadata),
            scenes::owner_id.eq(ctx.user_id),
            scenes::created_at.eq(now),
            scenes::updated_at.eq(now),
            scenes::background_image_path.eq(&background_image_path),
            // Named in a second pass below: the asset row's `scene_id` points
            // back here, so the scene has to exist before the asset can.
            scenes::background_asset_id.eq(None::<Uuid>),
            scenes::summary_markdown.eq(&summary_markdown),
            scenes::summary_rendered_html.eq(&summary_rendered_html),
            // Arrives hidden, which is the default for a new scene and the
            // right one here: a recipient decides when their players see it.
            scenes::hidden.eq(true),
            scenes::preview_asset_id.eq(None::<Uuid>),
        ))
        .execute(conn)?;

    // The background asset, now that there is a scene for it to belong to.
    if let Some(asset_id) = background_asset_id {
        let new_asset_id = copy_background_asset(conn, ctx, asset_id, new_scene_id)?;
        if new_asset_id.is_some() {
            diesel::update(scenes::table.filter(scenes::scene_id.eq(new_scene_id)))
                .set(scenes::background_asset_id.eq(new_asset_id))
                .execute(conn)?;
        }
    }

    // --- walls (SC-008a) ---
    let wall_rows = walls::table
        .filter(walls::scene_id.eq(source_id))
        .select((
            walls::x1,
            walls::y1,
            walls::x2,
            walls::y2,
            walls::blocks_vision,
            walls::blocks_movement,
            walls::metadata,
            walls::door_state,
            walls::locked,
            walls::secret,
        ))
        .load::<(
            f64,
            f64,
            f64,
            f64,
            bool,
            bool,
            Option<serde_json::Value>,
            String,
            bool,
            bool,
        )>(conn)?;

    for (x1, y1, x2, y2, blocks_vision, blocks_movement, metadata, door_state, locked, secret) in
        wall_rows
    {
        diesel::insert_into(walls::table)
            .values((
                walls::wall_id.eq(Uuid::now_v7()),
                walls::scene_id.eq(new_scene_id),
                walls::x1.eq(x1),
                walls::y1.eq(y1),
                walls::x2.eq(x2),
                walls::y2.eq(y2),
                walls::blocks_vision.eq(blocks_vision),
                walls::blocks_movement.eq(blocks_movement),
                walls::metadata.eq(metadata),
                walls::created_by.eq(ctx.user_id),
                walls::updated_by.eq(ctx.user_id),
                walls::created_at.eq(now),
                walls::updated_at.eq(now),
                walls::door_state.eq(door_state),
                walls::locked.eq(locked),
                walls::secret.eq(secret),
            ))
            .execute(conn)?;
    }

    // --- lighting (SC-008a) ---
    let light_rows = light_sources::table
        .filter(light_sources::scene_id.eq(source_id))
        .select((
            light_sources::x,
            light_sources::y,
            light_sources::radius,
            light_sources::intensity,
            light_sources::color,
            light_sources::casts_shadows,
            light_sources::metadata,
        ))
        .load::<(
            f64,
            f64,
            f64,
            f64,
            Option<String>,
            bool,
            Option<serde_json::Value>,
        )>(conn)?;

    for (x, y, radius, intensity, color, casts_shadows, metadata) in light_rows {
        diesel::insert_into(light_sources::table)
            .values((
                light_sources::light_id.eq(Uuid::now_v7()),
                light_sources::scene_id.eq(new_scene_id),
                light_sources::x.eq(x),
                light_sources::y.eq(y),
                light_sources::radius.eq(radius),
                light_sources::intensity.eq(intensity),
                light_sources::color.eq(color),
                // A light attached to a token loses that attachment, because
                // tokens are not copied. It stays where it was standing.
                light_sources::attached_token_id.eq(None::<Uuid>),
                light_sources::casts_shadows.eq(casts_shadows),
                light_sources::metadata.eq(metadata),
                light_sources::created_by.eq(ctx.user_id),
                light_sources::updated_by.eq(ctx.user_id),
                light_sources::created_at.eq(now),
                light_sources::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    // --- drawn scenery ---
    let shape_rows = shapes::table
        .filter(shapes::scene_id.eq(source_id))
        .select((
            shapes::kind,
            shapes::geometry,
            shapes::text,
            shapes::style,
            shapes::visible_to_players,
            shapes::metadata,
        ))
        .load::<(
            String,
            serde_json::Value,
            Option<String>,
            Option<serde_json::Value>,
            bool,
            Option<serde_json::Value>,
        )>(conn)?;

    for (kind, geometry, text, style, visible_to_players, metadata) in shape_rows {
        diesel::insert_into(shapes::table)
            .values((
                shapes::shape_id.eq(Uuid::now_v7()),
                shapes::scene_id.eq(new_scene_id),
                shapes::kind.eq(kind),
                shapes::geometry.eq(geometry),
                shapes::text.eq(text),
                shapes::style.eq(style),
                shapes::visible_to_players.eq(visible_to_players),
                shapes::metadata.eq(metadata),
                shapes::created_by.eq(ctx.user_id),
                shapes::updated_by.eq(ctx.user_id),
                shapes::created_at.eq(now),
                shapes::updated_at.eq(now),
            ))
            .execute(conn)?;
    }

    // --- what stayed behind, declared (FR-015) ---
    let token_count: i64 = tokens::table
        .filter(tokens::scene_id.eq(source_id))
        .count()
        .get_result(conn)?;
    if token_count > 0 {
        ctx.notes.push(format!(
            "\"{name}\" had {token_count} token{} placed on it. Tokens are part of a game in progress rather than the place itself, so they were not copied.",
            if token_count == 1 { "" } else { "s" }
        ));
    }

    let fog_count: i64 = fog_masks::table
        .filter(fog_masks::scene_id.eq(source_id))
        .count()
        .get_result(conn)?;
    if fog_count > 0 {
        ctx.notes.push(format!(
            "\"{name}\" carried explored-area fog from its own game, which was not copied."
        ));
    }

    let interactive_count: i64 = interactives::table
        .filter(interactives::scene_id.eq(source_id))
        .count()
        .get_result(conn)?;
    if interactive_count > 0 {
        ctx.notes.push(format!(
            "\"{name}\" had {interactive_count} interactive element{} wired to its own world's content, which {} not copied.",
            if interactive_count == 1 { "" } else { "s" },
            if interactive_count == 1 { "was" } else { "were" }
        ));
    }

    ctx.scene_map.insert(source_id, new_scene_id);
    ctx.record("scene", new_scene_id, &name);
    Ok(())
}

/// A new asset row in the destination world, on the **same** `storage_path`.
///
/// Returns `None` when the source asset row has gone — a scene whose
/// background row was deleted keeps its `background_image_path` and loses its
/// asset, which is the state it was already in.
fn copy_background_asset(
    conn: &mut PgConnection,
    ctx: &mut CopyContext,
    source_asset_id: Uuid,
    new_scene_id: Uuid,
) -> Result<Option<Uuid>, CopyError> {
    use crate::schema::canvas_image_assets;

    let source = canvas_image_assets::table
        .filter(canvas_image_assets::asset_id.eq(source_asset_id))
        .select((
            canvas_image_assets::storage_path,
            canvas_image_assets::original_format,
            canvas_image_assets::width_px,
            canvas_image_assets::height_px,
            canvas_image_assets::byte_size,
            canvas_image_assets::kind,
            canvas_image_assets::content_hash,
        ))
        .first::<(
            String,
            String,
            i32,
            i32,
            i64,
            crate::db_types::CanvasImageAssetKindEnum,
            Option<String>,
        )>(conn)
        .optional()?;

    let Some((storage_path, original_format, width_px, height_px, byte_size, kind, content_hash)) =
        source
    else {
        return Ok(None);
    };

    let new_asset_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(canvas_image_assets::table)
        .values((
            canvas_image_assets::asset_id.eq(new_asset_id),
            canvas_image_assets::world_id.eq(ctx.destination_world_id),
            canvas_image_assets::scene_id.eq(Some(new_scene_id)),
            canvas_image_assets::owner_user_id.eq(ctx.user_id),
            // The same object. Not a copy of the bytes — SC-008 asserts that
            // this adds no new `storage_path`.
            canvas_image_assets::storage_path.eq(storage_path),
            canvas_image_assets::original_format.eq(original_format),
            canvas_image_assets::width_px.eq(width_px),
            canvas_image_assets::height_px.eq(height_px),
            canvas_image_assets::byte_size.eq(byte_size),
            canvas_image_assets::kind.eq(kind),
            canvas_image_assets::created_by.eq(ctx.user_id),
            canvas_image_assets::updated_by.eq(ctx.user_id),
            canvas_image_assets::created_at.eq(now),
            canvas_image_assets::updated_at.eq(now),
            canvas_image_assets::content_hash.eq(content_hash),
        ))
        .execute(conn)?;

    Ok(Some(new_asset_id))
}

/// A scene name free in this world, derived from the source's.
fn unique_scene_name(
    conn: &mut PgConnection,
    world_id: Uuid,
    desired: &str,
) -> Result<String, CopyError> {
    use crate::schema::scenes;

    let mut candidate = desired.to_string();
    let mut suffix = 1;
    loop {
        let taken: i64 = scenes::table
            .filter(scenes::world_id.eq(world_id))
            .filter(scenes::name.eq(&candidate))
            .count()
            .get_result(conn)?;
        if taken == 0 {
            return Ok(candidate);
        }
        suffix += 1;
        candidate = if suffix == 2 {
            format!("{desired} (copy)")
        } else {
            format!("{desired} (copy {})", suffix - 1)
        };
        if suffix > 1000 {
            return Err(CopyError(
                "Could not find a free name for a copied scene".to_string(),
            ));
        }
    }
}
