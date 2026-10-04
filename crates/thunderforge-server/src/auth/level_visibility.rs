//! Which of a scene's levels a viewer may read.
//!
//! A scene is one place with several floors, and a client holds exactly one
//! of them at a time. That is only a privacy boundary if the server decides
//! which one — a client that could name any level and be answered would turn
//! "the party split up" into "everyone can watch the cellar from the roof".
//!
//! # The rule, stated once
//!
//! - A **Game Master** (world Owner or GM, or a site admin) may read every
//!   level of the scene.
//! - **Anyone else** may read only a level on which they control a token —
//!   by the same definition of control that decides whose token may be moved
//!   (`combat::controllers`). When they control no token in the scene at all,
//!   they may read the scene's **entry level** and nothing else.
//! - A level the viewer may not read is answered exactly as a level that does
//!   not exist. Nothing distinguishes "no such floor" from "not your floor".
//!
//! When a read names no level, it is answered for the viewer's *default*
//! level: the entry level if they may read it, otherwise the lowest-ordered
//! level they may. For a Game Master that is always the entry level.
//!
//! # What `hidden` is not
//!
//! `scene_levels.hidden` is a Game Master's note that a floor is not ready.
//! It does not appear in this rule, because the rule is already narrower:
//! a player reaches a level only by having a token put there, and the Game
//! Master putting a token on a hidden floor is the same deliberate act as
//! launching a hidden scene (`auth::scene_visibility`).
//!
//! # Why this is one module
//!
//! Every level-scoped read — tokens, walls, lights, shapes, interactives,
//! fog, the level list, and a floor's background image — asks this question,
//! and the scene-visibility rule before it is the cautionary tale: two call
//! sites, two answers, and the bytes were readable where the list was not.
//! The decision below is a pure function over three small inputs so that it
//! can be tested without a database, and the database half only gathers them.

use diesel::prelude::*;
use uuid::Uuid;

/// The little of a level this rule needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelBrief {
    pub level_id: Uuid,
    pub sort_order: i32,
    pub is_entry: bool,
}

/// The levels of a scene this viewer may read, in the scene's own order.
///
/// `levels` is every level of the scene; `standing_on` is the level of each
/// token the viewer controls there (repeats are harmless).
pub fn readable(levels: &[LevelBrief], is_gm: bool, standing_on: &[Uuid]) -> Vec<Uuid> {
    let mut ordered: Vec<&LevelBrief> = levels.iter().collect();
    ordered.sort_by_key(|level| level.sort_order);

    if is_gm {
        return ordered.iter().map(|level| level.level_id).collect();
    }

    let occupied: Vec<Uuid> = ordered
        .iter()
        .filter(|level| standing_on.contains(&level.level_id))
        .map(|level| level.level_id)
        .collect();
    if !occupied.is_empty() {
        return occupied;
    }

    // No token of theirs anywhere in the scene: the way in, and only that.
    ordered
        .iter()
        .filter(|level| level.is_entry)
        .map(|level| level.level_id)
        .collect()
}

/// The one level a read is answered for, or `None` when it is refused.
///
/// `requested` is the level the caller named, if any. A named level outside
/// the readable set — another scene's, a nonexistent one, or a floor the
/// viewer has no token on — is `None`, with nothing to tell those apart.
pub fn resolve(
    levels: &[LevelBrief],
    is_gm: bool,
    standing_on: &[Uuid],
    requested: Option<Uuid>,
) -> Option<Uuid> {
    let may_read = readable(levels, is_gm, standing_on);
    match requested {
        Some(level_id) => may_read.contains(&level_id).then_some(level_id),
        None => {
            let entry = levels
                .iter()
                .find(|level| level.is_entry)
                .map(|level| level.level_id);
            entry
                .filter(|entry| may_read.contains(entry))
                .or_else(|| may_read.first().copied())
        }
    }
}

/// Every level of a scene, reduced to what the rule reads.
pub fn levels_of_scene(conn: &mut PgConnection, scene_id: Uuid) -> QueryResult<Vec<LevelBrief>> {
    use crate::schema::scene_levels;

    Ok(scene_levels::table
        .filter(scene_levels::scene_id.eq(scene_id))
        .order(scene_levels::sort_order.asc())
        .select((
            scene_levels::level_id,
            scene_levels::sort_order,
            scene_levels::is_entry,
        ))
        .load::<(Uuid, i32, bool)>(conn)?
        .into_iter()
        .map(|(level_id, sort_order, is_entry)| LevelBrief {
            level_id,
            sort_order,
            is_entry,
        })
        .collect())
}

/// The level of each token this user controls in the scene.
pub fn levels_stood_on(
    conn: &mut PgConnection,
    user_id: Uuid,
    scene_id: Uuid,
) -> QueryResult<Vec<Uuid>> {
    use crate::schema::tokens;

    let controlled =
        crate::combat::controllers::controlled_tokens_in_scene(conn, user_id, scene_id)?;
    if controlled.is_empty() {
        return Ok(Vec::new());
    }
    tokens::table
        .filter(tokens::token_id.eq_any(controlled))
        .select(tokens::level_id)
        .distinct()
        .load::<Uuid>(conn)
}

/// The levels of `scene_id` this viewer may read, in order.
///
/// `is_gm` is the caller's already-resolved standing in the scene's world
/// (`world_membership::is_dm_of_scene`), passed in because every caller has
/// just established it.
pub fn readable_levels(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_gm: bool,
    scene_id: Uuid,
) -> QueryResult<Vec<Uuid>> {
    let levels = levels_of_scene(conn, scene_id)?;
    let standing_on = if is_gm {
        Vec::new()
    } else {
        levels_stood_on(conn, user_id, scene_id)?
    };
    Ok(readable(&levels, is_gm, &standing_on))
}

/// The level a read of `scene_id` is answered for, or `None` when refused.
///
/// The database half of [`resolve`]. A caller that gets `None` answers with
/// nothing — an empty list, a missing row — never with an error that would
/// say the level exists.
pub fn level_for_read(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_gm: bool,
    scene_id: Uuid,
    requested: Option<Uuid>,
) -> QueryResult<Option<Uuid>> {
    let levels = levels_of_scene(conn, scene_id)?;
    let standing_on = if is_gm {
        Vec::new()
    } else {
        levels_stood_on(conn, user_id, scene_id)?
    };
    Ok(resolve(&levels, is_gm, &standing_on, requested))
}

/// [`level_for_read`] for a caller that has not yet worked out whether the
/// viewer runs the world.
pub fn level_for_viewer(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    requested: Option<Uuid>,
) -> QueryResult<Option<Uuid>> {
    let is_gm = crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)?;
    level_for_read(conn, user_id, is_gm, scene_id, requested)
}

/// The scene's entry level — where a thing lands when nobody said otherwise.
pub fn entry_level(conn: &mut PgConnection, scene_id: Uuid) -> QueryResult<Uuid> {
    use crate::schema::scene_levels;

    scene_levels::table
        .filter(scene_levels::scene_id.eq(scene_id))
        .filter(scene_levels::is_entry.eq(true))
        .select(scene_levels::level_id)
        .first(conn)
}

/// Whether this viewer may fetch a canvas asset, as far as levels go.
///
/// A level's background is the floor plan of that level, so it is readable
/// exactly when the level is. An asset that is no level's background is not
/// this rule's business and passes. One that is the background of several
/// levels passes if any of them is readable.
///
/// Asked after `scene_visibility::asset_scene_visible`, never instead of it.
pub fn asset_level_readable(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_gm: bool,
    asset_id: Uuid,
) -> QueryResult<bool> {
    use crate::schema::scene_levels;

    if is_gm {
        return Ok(true);
    }
    let floors: Vec<(Uuid, Uuid)> = scene_levels::table
        .filter(scene_levels::background_asset_id.eq(asset_id))
        .select((scene_levels::level_id, scene_levels::scene_id))
        .load(conn)?;
    if floors.is_empty() {
        return Ok(true);
    }
    for (level_id, scene_id) in floors {
        if readable_levels(conn, user_id, false, scene_id)?.contains(&level_id) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The floor backgrounds, across `scene_ids`, that this viewer may not
/// fetch — [`asset_level_readable`] answered for a whole world at once, for
/// the sync plan, so the plan and the asset route cannot disagree.
pub fn withheld_backgrounds(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_gm: bool,
    scene_ids: &[Uuid],
) -> QueryResult<std::collections::HashSet<Uuid>> {
    use crate::schema::scene_levels;
    use std::collections::hash_map::Entry;
    use std::collections::{HashMap, HashSet};

    if is_gm {
        return Ok(HashSet::new());
    }
    let floors: Vec<(Uuid, Uuid, Option<Uuid>)> = scene_levels::table
        .filter(scene_levels::scene_id.eq_any(scene_ids))
        .filter(scene_levels::background_asset_id.is_not_null())
        .select((
            scene_levels::level_id,
            scene_levels::scene_id,
            scene_levels::background_asset_id,
        ))
        .load(conn)?;

    let mut may_read: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    let mut allowed = HashSet::new();
    let mut withheld = HashSet::new();
    for (level_id, scene_id, asset_id) in floors {
        let Some(asset_id) = asset_id else { continue };
        if let Entry::Vacant(unread) = may_read.entry(scene_id) {
            unread.insert(readable_levels(conn, user_id, false, scene_id)?);
        }
        if may_read[&scene_id].contains(&level_id) {
            allowed.insert(asset_id);
        } else {
            withheld.insert(asset_id);
        }
    }
    // One readable floor using the image is enough to have it.
    Ok(withheld.difference(&allowed).copied().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> (Vec<LevelBrief>, Uuid, Uuid, Uuid) {
        let cellar = Uuid::now_v7();
        let ground = Uuid::now_v7();
        let loft = Uuid::now_v7();
        let levels = vec![
            LevelBrief {
                level_id: loft,
                sort_order: 2,
                is_entry: false,
            },
            LevelBrief {
                level_id: cellar,
                sort_order: 0,
                is_entry: false,
            },
            LevelBrief {
                level_id: ground,
                sort_order: 1,
                is_entry: true,
            },
        ];
        (levels, cellar, ground, loft)
    }

    #[test]
    fn a_game_master_reads_every_level_in_order() {
        let (levels, cellar, ground, loft) = scene();
        assert_eq!(readable(&levels, true, &[]), vec![cellar, ground, loft]);
    }

    #[test]
    fn a_player_with_no_token_reads_the_entry_level_and_nothing_else() {
        let (levels, cellar, ground, loft) = scene();
        assert_eq!(readable(&levels, false, &[]), vec![ground]);
        assert_eq!(resolve(&levels, false, &[], None), Some(ground));
        assert_eq!(resolve(&levels, false, &[], Some(ground)), Some(ground));
        assert_eq!(resolve(&levels, false, &[], Some(cellar)), None);
        assert_eq!(resolve(&levels, false, &[], Some(loft)), None);
    }

    #[test]
    fn a_player_upstairs_reads_upstairs_and_loses_the_entry_level() {
        // The whole point: standing in the loft is not a view of the ground
        // floor, entry level or not.
        let (levels, cellar, ground, loft) = scene();
        assert_eq!(readable(&levels, false, &[loft]), vec![loft]);
        assert_eq!(resolve(&levels, false, &[loft], None), Some(loft));
        assert_eq!(resolve(&levels, false, &[loft], Some(ground)), None);
        assert_eq!(resolve(&levels, false, &[loft], Some(cellar)), None);
    }

    #[test]
    fn a_player_with_tokens_on_two_levels_reads_both_and_defaults_to_the_entry() {
        let (levels, cellar, ground, loft) = scene();
        assert_eq!(
            readable(&levels, false, &[loft, ground, loft]),
            vec![ground, loft]
        );
        assert_eq!(resolve(&levels, false, &[loft, ground], None), Some(ground));
        assert_eq!(
            resolve(&levels, false, &[loft, cellar], None),
            Some(cellar),
            "with no token on the entry level, the lowest-ordered occupied one"
        );
    }

    #[test]
    fn a_level_from_somewhere_else_is_refused_for_everyone() {
        let (levels, _, _, _) = scene();
        let stranger = Uuid::now_v7();
        assert_eq!(resolve(&levels, true, &[], Some(stranger)), None);
        assert_eq!(resolve(&levels, false, &[stranger], Some(stranger)), None);
    }

    #[test]
    fn a_game_master_defaults_to_the_entry_level() {
        let (levels, _, ground, loft) = scene();
        assert_eq!(resolve(&levels, true, &[], None), Some(ground));
        assert_eq!(resolve(&levels, true, &[], Some(loft)), Some(loft));
    }

    #[test]
    fn a_scene_with_no_levels_answers_nothing() {
        assert_eq!(resolve(&[], true, &[], None), None);
        assert_eq!(resolve(&[], false, &[], None), None);
    }
}
