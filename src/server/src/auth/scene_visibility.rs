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
//! - Anyone else sees scenes with `hidden = false`, **and the one scene the
//!   world is currently playing**, hidden or not.
//! - The `hidden` column defaults to **true**: a freshly created scene is
//!   invisible to players until the GM reveals it, or launches it.
//! - A **canvas asset** inherits the visibility of the scene it is attached
//!   to. `scene_id IS NULL` means it belongs to the world rather than to any
//!   scene, and is visible to every member.
//!
//! # Why the scene being played is not hidden from the people playing it
//!
//! `hidden` keeps a GM's unfinished prep out of the players' Scenes table.
//! Launching a scene is the opposite act — the GM deliberately putting it in
//! front of everyone — and a world's auto-created scene is hidden by default,
//! so without this carve-out the ordinary case was a player sitting at a
//! table whose map they were not allowed to know anything about. Concretely
//! it made `world_sync_plan` return an **empty plan** to every player in
//! every world whose scene had never been un-hidden: no scene state, no
//! assets, nothing cached, in a feature whose whole promise is that the
//! world's content is already on the device.
//!
//! It is one scene per world, chosen by the GM. Guessing another hidden
//! scene's id still answers `false`.
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

    if is_dm {
        return scenes::table
            .filter(scenes::world_id.eq(world_id))
            .select(scenes::scene_id)
            .load(conn);
    }

    // One extra read rather than a correlated subquery: this runs once per
    // plan, and the plan's cost is the asset classification below it, not
    // this.
    let active: Option<Uuid> = {
        use crate::schema::worlds;
        worlds::table
            .filter(worlds::id.eq(world_id))
            .select(worlds::active_scene_id)
            .first::<Option<Uuid>>(conn)
            .optional()?
            .flatten()
    };

    scenes::table
        .filter(scenes::world_id.eq(world_id))
        .filter(
            scenes::hidden
                .eq(false)
                .or(scenes::scene_id.nullable().eq(active)),
        )
        .select(scenes::scene_id)
        .load(conn)
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
    // The scene's own flag and its world's current scene, in one read: an
    // asset on the scene being played is reachable by the people playing it,
    // exactly as the scene itself is.
    use crate::schema::worlds;
    let row: Option<(bool, Option<Uuid>)> = scenes::table
        .inner_join(worlds::table.on(worlds::id.eq(scenes::world_id)))
        .filter(scenes::scene_id.eq(scene_id))
        .select((scenes::hidden, worlds::active_scene_id))
        .first::<(bool, Option<Uuid>)>(conn)
        .optional()?;

    // A scene id matching no row answers `false` — the safe way to be wrong
    // about a dangling reference.
    Ok(match row {
        Some((hidden, active)) => !hidden || active == Some(scene_id),
        None => false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{scenes, worlds};
    use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

    /// A hidden scene, optionally the world's active one.
    fn insert_hidden_scene(
        conn: &mut PgConnection,
        world_id: Uuid,
        owner_id: Uuid,
        name: &str,
    ) -> Uuid {
        let scene_id = Uuid::now_v7();
        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq(name.to_string()),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(5),
                scenes::grid_type.eq("square"),
                scenes::width.eq(100),
                scenes::height.eq(100),
                scenes::owner_id.eq(owner_id),
                scenes::hidden.eq(true),
            ))
            .execute(conn)
            .expect("failed to insert scene");
        scene_id
    }

    fn launch(conn: &mut PgConnection, world_id: Uuid, scene_id: Uuid) {
        diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
            .set(worlds::active_scene_id.eq(Some(scene_id)))
            .execute(conn)
            .expect("failed to set the active scene");
    }

    /// The carve-out and its edge, in one test.
    ///
    /// Two hidden scenes, one of them launched. A player must see the one
    /// they are playing — without it `world_sync_plan` hands them an empty
    /// plan and they cache nothing at all — and must still not see the
    /// other, or "the scene being played is visible" has quietly become
    /// "hidden means nothing". Asserting only the first half would pass on a
    /// change that dropped the filter entirely.
    #[test]
    fn a_player_sees_the_scene_being_played_and_no_other_hidden_one() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let played = insert_hidden_scene(&mut conn, world_id, owner_id, "Played");
        let prep = insert_hidden_scene(&mut conn, world_id, owner_id, "Unfinished Prep");
        launch(&mut conn, world_id, played);

        let visible = visible_scene_ids(&mut conn, false, world_id).expect("query");
        assert!(
            visible.contains(&played),
            "a player must see the scene their world is playing",
        );
        assert!(
            !visible.contains(&prep),
            "every other hidden scene must stay hidden from a player",
        );

        // And the DM still sees both, which is the rule the carve-out must
        // not have disturbed.
        let dm_visible = visible_scene_ids(&mut conn, true, world_id).expect("query");
        assert!(dm_visible.contains(&played) && dm_visible.contains(&prep));
    }

    /// Nothing is launched, so nothing hidden is visible. Guards against a
    /// `NULL` active scene matching a `NULL`-ish comparison and revealing
    /// every hidden scene at once.
    #[test]
    fn a_world_playing_nothing_reveals_no_hidden_scene() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);
        let prep = insert_hidden_scene(&mut conn, world_id, owner_id, "Prep");

        let visible = visible_scene_ids(&mut conn, false, world_id).expect("query");
        assert!(
            !visible.contains(&prep),
            "with no active scene, a hidden scene stays hidden",
        );
    }

    /// The asset side of the same rule. The byte route asks this, so a
    /// player whose plan now lists the played scene's art must also be
    /// allowed to fetch the bytes — the two answering differently is the
    /// exact split this module was written to end.
    #[test]
    fn art_on_the_scene_being_played_is_fetchable_and_other_hidden_art_is_not() {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner_id = insert_test_user(&mut conn);
        let world_id = insert_test_world(&mut conn, owner_id);

        let played = insert_hidden_scene(&mut conn, world_id, owner_id, "Played");
        let prep = insert_hidden_scene(&mut conn, world_id, owner_id, "Prep");
        launch(&mut conn, world_id, played);

        assert!(asset_scene_visible(&mut conn, false, Some(played)).expect("query"));
        assert!(!asset_scene_visible(&mut conn, false, Some(prep)).expect("query"));
        // A world-scoped asset belongs to no scene and is every member's.
        assert!(asset_scene_visible(&mut conn, false, None).expect("query"));
    }
}
