//! Who may see which scenes, and the canvas assets attached to them.
//!
//! Spec `028-client-world-cache` T045c. Scenes and canvas image assets carry
//! no permission-grant table of their own — the ADR-050 ladder covers actors,
//! items, abilities and lore — so the per-object visibility axis that applies
//! to them is `scenes.hidden`, the GM-only flag `scenes_impl` enforces.
//!
//! # The rule, stated once
//!
//! - A **DM** (world Owner or GM, or a site admin) sees every scene in the
//!   world, hidden or not.
//! - Anyone else sees only scenes with `hidden = false`. The column defaults
//!   to **true**: a freshly created scene is invisible to players until the
//!   GM reveals it.
//! - A **canvas asset** inherits the visibility of the scene it is attached
//!   to. `scene_id IS NULL` means it belongs to the world rather than to any
//!   scene, and is visible to every member.
//!
//! # Why this module exists rather than the rule living at each call site
//!
//! It had two call sites answering differently. `world_sync_plan` narrowed a
//! world's assets to visible scenes, so a hidden scene's art never appeared
//! in a player's plan — while `GET /canvas-assets/{id}` authorized on world
//! membership alone, so the same player could fetch those exact bytes by
//! asking for the id directly. The plan was the only thing enforcing a rule
//! the bytes did not have.
//!
//! The two shapes below are deliberately different queries, not one function
//! used twice: a plan classifies every asset in a world and wants the visible
//! set in one round trip, while the byte route holds one asset and wants one
//! answer. Sharing a *predicate* across those would have cost the plan an
//! N+1. What they share is this module and the rule written above it.

use diesel::prelude::*;
use uuid::Uuid;

/// Every scene in `world_id` this caller may see.
///
/// `is_dm` is the caller's already-resolved DM-ness — Owner or GM in this
/// world, or a site admin. It is a parameter rather than something derived
/// here because both callers have just established it from
/// `require_world_member` and re-deriving it would take a second query to
/// answer a question already answered.
pub fn visible_scene_ids(
    conn: &mut PgConnection,
    is_dm: bool,
    world_id: Uuid,
) -> Result<Vec<Uuid>, diesel::result::Error> {
    use crate::schema::scenes;

    let mut query = scenes::table
        .filter(scenes::world_id.eq(world_id))
        .select(scenes::scene_id)
        .into_boxed();
    if !is_dm {
        query = query.filter(scenes::hidden.eq(false));
    }
    query.load(conn)
}

/// Whether this caller may see the canvas asset attached to `scene_id`.
///
/// `None` — a world-scoped asset — is visible to any member, which is why
/// this takes the asset's scene rather than looking one up: the caller has
/// already loaded the asset row to find its world.
///
/// A scene id that matches no row answers `false`. That is the safe way to
/// be wrong about a dangling reference, and it costs nothing real: an asset
/// pointing at a deleted scene has no scene to be viewed from.
pub fn asset_scene_visible(
    conn: &mut PgConnection,
    is_dm: bool,
    scene_id: Option<Uuid>,
) -> Result<bool, diesel::result::Error> {
    use crate::schema::scenes;

    let Some(scene_id) = scene_id else {
        return Ok(true);
    };
    if is_dm {
        return Ok(true);
    }
    let hidden: Option<bool> = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::hidden)
        .first::<bool>(conn)
        .optional()?;
    Ok(hidden == Some(false))
}
