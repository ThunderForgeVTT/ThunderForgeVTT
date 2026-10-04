//! Authoring a scene's levels: the floors of one place.
//!
//! A scene owns an ordered set of levels and every placed thing stands on
//! exactly one. This module is the Game Master's half — making, renaming,
//! ordering and removing floors, and carrying tokens between them by hand.
//! Who may *read* a level is `auth::level_visibility`; how a token walks
//! between two is `level_travel`.
//!
//! # The entry level and the scene are one board
//!
//! Every scene has exactly one entry level, and its board — background, size,
//! ambient light — is the scene's own, kept equal by a database trigger with
//! the scene leading. So an edit to the entry level's board is written to the
//! scene row and arrives on the level by that trigger, and naming a different
//! level the entry copies *its* board onto the scene. Everything that predates
//! levels (map import, the scene list, the engine) keeps reading the scene and
//! keeps being right.
//!
//! # Refusals are sentences
//!
//! Each function here answers [`LevelError::Refused`] with words a Game
//! Master can act on. They are all Game-Master-only, so there is nothing to
//! hide from the caller about what exists.

use diesel::prelude::*;
use diesel::result::Error as DieselError;
use uuid::Uuid;

use crate::models::{SceneLevel, Token};
use crate::play_pause::gate::refuse_scene_if_paused;
use crate::world_events::{EVENT_CODE_SCENE_LEVEL_CHANGED, record_world_event, world_id_for_scene};

/// How many levels one scene may have.
///
/// A scene is one place. A dozen floors is a tall tower; past that the thing
/// being built is several places, and several scenes say so better.
pub const MAX_LEVELS_PER_SCENE: i64 = 12;

/// The values a level's ambient light may take — the column's own check.
const AMBIENT_LEVELS: [&str; 3] = ["bright", "dim", "dark"];

#[derive(Debug)]
pub enum LevelError {
    /// The request was understood and is not allowed; the text says why.
    Refused(String),
    Database(DieselError),
}

impl From<DieselError> for LevelError {
    fn from(e: DieselError) -> Self {
        LevelError::Database(e)
    }
}

fn refused<T>(why: &str) -> Result<T, LevelError> {
    Err(LevelError::Refused(why.to_string()))
}

/// What a new level starts with. Anything left out is taken from the scene,
/// so a new floor is the same size and as well lit as the one below it.
#[derive(Debug, Default, Clone)]
pub struct NewLevel {
    pub name: String,
    pub hidden: Option<bool>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub ambient_light: Option<String>,
    pub background_asset_id: Option<Uuid>,
}

/// What to change about a level. `None` leaves a thing as it is.
#[derive(Debug, Default, Clone)]
pub struct LevelChanges {
    pub name: Option<String>,
    pub hidden: Option<bool>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub ambient_light: Option<String>,
    pub background_asset_id: Option<Uuid>,
    /// Remove the background. Wins over `background_asset_id`.
    pub clear_background: bool,
    /// Make this the scene's entry level.
    pub make_entry: bool,
}

fn checked_name(name: &str) -> Result<String, LevelError> {
    let name = name.trim();
    if name.is_empty() {
        return refused("A level needs a name");
    }
    if name.chars().count() > 80 {
        return refused("A level's name is at most 80 characters");
    }
    Ok(name.to_string())
}

fn checked_size(width: Option<i32>, height: Option<i32>) -> Result<(), LevelError> {
    if width.is_some_and(|w| w <= 0) || height.is_some_and(|h| h <= 0) {
        return refused("A level's width and height must be more than zero");
    }
    Ok(())
}

fn checked_light(light: Option<String>) -> Result<Option<String>, LevelError> {
    let Some(light) = light else { return Ok(None) };
    let light = light.trim().to_ascii_lowercase();
    if !AMBIENT_LEVELS.contains(&light.as_str()) {
        return refused("A level's light is bright, dim or dark");
    }
    Ok(Some(light))
}

/// A background has to be an image of the same world: an asset id is a bare
/// reference, and another world's art must not be reachable by naming it.
fn checked_background(
    conn: &mut PgConnection,
    scene_id: Uuid,
    asset_id: Option<Uuid>,
) -> Result<(), LevelError> {
    use crate::schema::{canvas_image_assets, scenes};

    let Some(asset_id) = asset_id else {
        return Ok(());
    };
    let world_id: Uuid = scenes::table
        .filter(scenes::scene_id.eq(scene_id))
        .select(scenes::world_id)
        .first(conn)?;
    let found: i64 = canvas_image_assets::table
        .filter(canvas_image_assets::asset_id.eq(asset_id))
        .filter(canvas_image_assets::world_id.eq(world_id))
        .count()
        .get_result(conn)?;
    if found == 0 {
        return refused("That image is not in this world");
    }
    Ok(())
}

/// The gate every function here passes first: the caller runs the scene's
/// world, and the world is not paused.
fn authorize(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
) -> Result<(), LevelError> {
    if !crate::auth::world_membership::is_dm_of_scene(conn, user_id, is_admin, scene_id)? {
        return refused("Only the Game Master may change a scene's levels");
    }
    refuse_scene_if_paused(conn, scene_id).map_err(DieselError::from)?;
    Ok(())
}

fn load_level(conn: &mut PgConnection, level_id: Uuid) -> Result<SceneLevel, LevelError> {
    use crate::schema::scene_levels;

    match scene_levels::table
        .filter(scene_levels::level_id.eq(level_id))
        .select(SceneLevel::as_select())
        .first::<SceneLevel>(conn)
        .optional()?
    {
        Some(level) => Ok(level),
        None => refused("Level not found"),
    }
}

/// Tell the table a scene's levels changed. Names the level and nothing about
/// it: each client re-reads `sceneLevels`, and is answered for who it is.
fn announce(
    conn: &mut PgConnection,
    scene_id: Uuid,
    action: &str,
    level_id: Option<Uuid>,
    user_id: Uuid,
) {
    crate::scene_fingerprint::refresh_scene_fingerprint(conn, scene_id, user_id);
    if let Ok(world_id) = world_id_for_scene(conn, scene_id) {
        let _ = record_world_event(
            conn,
            world_id,
            EVENT_CODE_SCENE_LEVEL_CHANGED,
            Some(serde_json::json!({
                "action": action,
                "scene_id": scene_id,
                "level_id": level_id,
            })),
            user_id,
        );
    }
}

/// Add a level on top of the scene's others.
pub fn create_level(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    new: NewLevel,
) -> Result<SceneLevel, LevelError> {
    use crate::schema::{scene_levels, scenes};

    authorize(conn, user_id, is_admin, scene_id)?;
    let name = checked_name(&new.name)?;
    checked_size(new.width, new.height)?;
    let light = checked_light(new.ambient_light)?;
    checked_background(conn, scene_id, new.background_asset_id)?;

    let level = conn.transaction::<_, LevelError, _>(|conn| {
        // The scene row is locked so two creations cannot both count eleven.
        let (width, height, ambient): (i32, i32, String) = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select((scenes::width, scenes::height, scenes::ambient_light))
            .for_update()
            .first(conn)?;
        let existing: Vec<i32> = scene_levels::table
            .filter(scene_levels::scene_id.eq(scene_id))
            .select(scene_levels::sort_order)
            .load(conn)?;
        if existing.len() as i64 >= MAX_LEVELS_PER_SCENE {
            return refused("A scene has at most 12 levels");
        }
        let top = existing.iter().max().map_or(0, |highest| highest + 1);

        Ok(diesel::insert_into(scene_levels::table)
            .values((
                scene_levels::scene_id.eq(scene_id),
                scene_levels::name.eq(&name),
                scene_levels::sort_order.eq(top),
                scene_levels::is_entry.eq(false),
                scene_levels::hidden.eq(new.hidden.unwrap_or(false)),
                scene_levels::background_asset_id.eq(new.background_asset_id),
                scene_levels::width.eq(new.width.unwrap_or(width)),
                scene_levels::height.eq(new.height.unwrap_or(height)),
                scene_levels::ambient_light.eq(light.unwrap_or(ambient)),
                scene_levels::created_by.eq(user_id),
                scene_levels::updated_by.eq(user_id),
            ))
            .returning(SceneLevel::as_returning())
            .get_result::<SceneLevel>(conn)?)
    })?;

    announce(conn, scene_id, "created", Some(level.level_id), user_id);
    Ok(level)
}

/// Change a level, and optionally make it the scene's entry.
pub fn update_level(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    level_id: Uuid,
    changes: LevelChanges,
) -> Result<SceneLevel, LevelError> {
    use crate::schema::{scene_levels, scenes};

    let level = load_level(conn, level_id)?;
    let scene_id = level.scene_id;
    authorize(conn, user_id, is_admin, scene_id)?;

    let name = changes.name.as_deref().map(checked_name).transpose()?;
    checked_size(changes.width, changes.height)?;
    let light = checked_light(changes.ambient_light.clone())?;
    if !changes.clear_background {
        checked_background(conn, scene_id, changes.background_asset_id)?;
    }

    let updated = conn.transaction::<_, LevelError, _>(|conn| {
        let now = chrono::Utc::now().naive_utc();

        // What is the level's alone, whichever level it is.
        diesel::update(scene_levels::table.filter(scene_levels::level_id.eq(level_id)))
            .set((
                name.as_ref().map(|name| scene_levels::name.eq(name)),
                changes.hidden.map(|hidden| scene_levels::hidden.eq(hidden)),
                scene_levels::updated_by.eq(user_id),
                scene_levels::updated_at.eq(now),
            ))
            .execute(conn)?;

        // The board. Written to the level here; an entry level's is then
        // carried to the scene below, which is where it is really kept.
        let (path, asset): (Option<String>, Option<Uuid>) = if changes.clear_background {
            (None, None)
        } else if let Some(asset) = changes.background_asset_id {
            // A stored asset replaces a legacy path rather than sitting
            // beside it, so there is one answer to "what is drawn here".
            (None, Some(asset))
        } else {
            (
                level.background_image_path.clone(),
                level.background_asset_id,
            )
        };
        let width = changes.width.unwrap_or(level.width);
        let height = changes.height.unwrap_or(level.height);
        let ambient = light.clone().unwrap_or_else(|| level.ambient_light.clone());
        diesel::update(scene_levels::table.filter(scene_levels::level_id.eq(level_id)))
            .set((
                scene_levels::background_image_path.eq(&path),
                scene_levels::background_asset_id.eq(asset),
                scene_levels::width.eq(width),
                scene_levels::height.eq(height),
                scene_levels::ambient_light.eq(&ambient),
            ))
            .execute(conn)?;

        if changes.make_entry && !level.is_entry {
            // One entry per scene is a unique index, so the old one steps
            // down before the new one steps up.
            diesel::update(
                scene_levels::table
                    .filter(scene_levels::scene_id.eq(scene_id))
                    .filter(scene_levels::is_entry.eq(true)),
            )
            .set(scene_levels::is_entry.eq(false))
            .execute(conn)?;
            diesel::update(scene_levels::table.filter(scene_levels::level_id.eq(level_id)))
                .set(scene_levels::is_entry.eq(true))
                .execute(conn)?;
        }

        if level.is_entry || changes.make_entry {
            // The scene leads; its trigger writes the same values back onto
            // whichever level is now the entry.
            diesel::update(scenes::table.filter(scenes::scene_id.eq(scene_id)))
                .set((
                    scenes::background_image_path.eq(&path),
                    scenes::background_asset_id.eq(asset),
                    scenes::width.eq(width),
                    scenes::height.eq(height),
                    scenes::ambient_light.eq(&ambient),
                ))
                .execute(conn)?;
        }

        Ok(scene_levels::table
            .filter(scene_levels::level_id.eq(level_id))
            .select(SceneLevel::as_select())
            .first::<SceneLevel>(conn)?)
    })?;

    announce(conn, scene_id, "updated", Some(level_id), user_id);
    Ok(updated)
}

/// Put a scene's levels in the order given, lowest first.
///
/// `ordered` must name every level of the scene exactly once: an order with a
/// floor missing is not an order, and guessing where the missing one goes
/// would be deciding it for the Game Master.
pub fn reorder_levels(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    scene_id: Uuid,
    ordered: &[Uuid],
) -> Result<Vec<SceneLevel>, LevelError> {
    use crate::schema::scene_levels;

    authorize(conn, user_id, is_admin, scene_id)?;

    let levels = conn.transaction::<_, LevelError, _>(|conn| {
        let mut current: Vec<Uuid> = scene_levels::table
            .filter(scene_levels::scene_id.eq(scene_id))
            .select(scene_levels::level_id)
            .for_update()
            .load(conn)?;
        let mut named = ordered.to_vec();
        current.sort();
        named.sort();
        if current != named {
            return refused("Name every level of the scene once, in the order you want");
        }

        // The order is unique per scene but checked at commit, so the rows
        // may pass through each other's places on the way.
        for (position, level_id) in ordered.iter().enumerate() {
            diesel::update(scene_levels::table.filter(scene_levels::level_id.eq(level_id)))
                .set((
                    scene_levels::sort_order.eq(position as i32),
                    scene_levels::updated_by.eq(user_id),
                ))
                .execute(conn)?;
        }

        Ok(scene_levels::table
            .filter(scene_levels::scene_id.eq(scene_id))
            .order(scene_levels::sort_order.asc())
            .select(SceneLevel::as_select())
            .load::<SceneLevel>(conn)?)
    })?;

    announce(conn, scene_id, "reordered", None, user_id);
    Ok(levels)
}

/// Remove a level and everything built on it.
///
/// Refused for the scene's last level and for its entry level — a scene with
/// no way in is not a scene — and refused while any token stands there:
/// walls and lights are scenery and go with the floor, but a token is
/// somebody's character, and deleting a floor must never be how one is lost.
pub fn delete_level(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    level_id: Uuid,
) -> Result<Uuid, LevelError> {
    use crate::schema::{scene_levels, tokens};

    let level = load_level(conn, level_id)?;
    let scene_id = level.scene_id;
    authorize(conn, user_id, is_admin, scene_id)?;

    conn.transaction::<_, LevelError, _>(|conn| {
        let count: i64 = scene_levels::table
            .filter(scene_levels::scene_id.eq(scene_id))
            .count()
            .get_result(conn)?;
        if count <= 1 {
            return refused("A scene keeps at least one level");
        }
        let is_entry: bool = scene_levels::table
            .filter(scene_levels::level_id.eq(level_id))
            .select(scene_levels::is_entry)
            .for_update()
            .first(conn)?;
        if is_entry {
            return refused(
                "This is the scene's entry level. Make another level the entry before deleting it",
            );
        }
        let standing: i64 = tokens::table
            .filter(tokens::level_id.eq(level_id))
            .count()
            .get_result(conn)?;
        if standing > 0 {
            return refused("Move the tokens off this level before deleting it");
        }

        // Walls, lights, shapes, interactives and fog go by foreign key.
        diesel::delete(scene_levels::table.filter(scene_levels::level_id.eq(level_id)))
            .execute(conn)?;
        Ok(())
    })?;

    announce(conn, scene_id, "deleted", Some(level_id), user_id);
    Ok(level_id)
}

/// Carry tokens to a level by hand — the Game Master's lift.
///
/// Every token must already be in the level's scene; this moves between
/// floors, not between places. With a point given the tokens are set down
/// around it, fanned out so they do not stack; without one each keeps the
/// position it had, which is right for floors drawn one above the other.
/// The lights a token carries go with it.
pub fn move_tokens_to_level(
    conn: &mut PgConnection,
    user_id: Uuid,
    is_admin: bool,
    token_ids: &[Uuid],
    level_id: Uuid,
    at: Option<(f64, f64)>,
) -> Result<Vec<Token>, LevelError> {
    use crate::schema::{light_sources, scenes, tokens};

    let level = load_level(conn, level_id)?;
    let scene_id = level.scene_id;
    authorize(conn, user_id, is_admin, scene_id)?;
    if token_ids.is_empty() {
        return Ok(Vec::new());
    }
    if at.is_some_and(|(x, y)| !x.is_finite() || !y.is_finite()) {
        return refused("That is not a place on the level");
    }

    let moved = conn.transaction::<_, LevelError, _>(|conn| {
        let here: Vec<Uuid> = tokens::table
            .filter(tokens::token_id.eq_any(token_ids))
            .filter(tokens::scene_id.eq(scene_id))
            .select(tokens::token_id)
            .for_update()
            .load(conn)?;
        if token_ids.iter().any(|token_id| !here.contains(token_id)) {
            return refused("Every token must be in the same scene as the level");
        }

        let step: i32 = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::grid_size)
            .first(conn)?;
        let mut taken: Vec<(f64, f64)> = tokens::table
            .filter(tokens::level_id.eq(level_id))
            .filter(tokens::token_id.ne_all(token_ids))
            .select((tokens::x, tokens::y))
            .load(conn)?;

        let mut moved = Vec::with_capacity(token_ids.len());
        for token_id in token_ids {
            let token = match at {
                Some(centre) => {
                    let (x, y) =
                        crate::level_travel::free_spot(centre, f64::from(step.max(1)), &taken);
                    taken.push((x, y));
                    diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                        .set((
                            tokens::level_id.eq(level_id),
                            tokens::x.eq(x),
                            tokens::y.eq(y),
                        ))
                        .returning(Token::as_returning())
                        .get_result::<Token>(conn)?
                }
                None => diesel::update(tokens::table.filter(tokens::token_id.eq(token_id)))
                    .set(tokens::level_id.eq(level_id))
                    .returning(Token::as_returning())
                    .get_result::<Token>(conn)?,
            };
            let lights = diesel::update(
                light_sources::table.filter(light_sources::attached_token_id.eq(token_id)),
            )
            .set((
                light_sources::level_id.eq(level_id),
                light_sources::x.eq(token.x),
                light_sources::y.eq(token.y),
            ))
            .execute(conn)?;
            moved.push((token, lights > 0));
        }
        Ok(moved)
    })?;

    Ok(moved
        .into_iter()
        .map(|(token, lights_moved)| {
            crate::level_travel::announce(conn, &token, lights_moved, user_id);
            token
        })
        .collect())
}

/// How many tokens stand on each level of a scene. A Game Master's view.
pub fn token_counts(
    conn: &mut PgConnection,
    scene_id: Uuid,
) -> QueryResult<std::collections::HashMap<Uuid, i64>> {
    use crate::schema::tokens;
    use diesel::dsl::count_star;

    Ok(tokens::table
        .filter(tokens::scene_id.eq(scene_id))
        .group_by(tokens::level_id)
        .select((tokens::level_id, count_star()))
        .load::<(Uuid, i64)>(conn)?
        .into_iter()
        .collect())
}

#[cfg(test)]
#[path = "scene_levels_tests.rs"]
pub(crate) mod tests;
