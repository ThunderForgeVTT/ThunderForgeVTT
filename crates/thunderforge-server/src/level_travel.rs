//! Moving a token from one level of a scene to another.
//!
//! A transition is an interactive carrying `nav.travel` and naming a partner
//! interactive, usually on another level. Travelling is being picked up at
//! the one and put down at the other.
//!
//! # Why the server does this and no client does
//!
//! A level is a privacy boundary (`auth::level_visibility`): a player is told
//! about a floor only while their own token stands on it. So which floor a
//! token is on cannot be something a client reports. A player's client is not
//! even sent a region's outline — it could not detect the stairs if it wanted
//! to. The server sees the move land inside the region and does the rest.
//!
//! # Two ways in
//!
//! - **Walking in.** A region with the `enter` trigger. Judged here, after
//!   `moveOwnToken` has already let the move stand — pause, control, turn and
//!   walls have all had their say by then. Only a move that *crosses into*
//!   the region counts (`interaction::entered`), and only its end point is
//!   looked at: a route that merely passes over the stairs does not climb
//!   them.
//! - **Clicking.** A door or a prop — a trapdoor, a ladder — through
//!   `activateInteractive`, which has to be told which token is going.
//!
//! A Game Master dragging a token (`updateToken`) never travels by accident.
//! They are arranging the board, and they have `moveTokensToLevel` for
//! putting a token on another floor on purpose.
//!
//! # Arriving is a placement
//!
//! The traveller is set down at the partner and nothing is activated on
//! arrival. This is what stops two stairwells pointing at each other from
//! bouncing a token between floors for ever: entry is a transition from
//! outside to inside, and a token that was *placed* inside has not made one.
//! It has to walk out and back in to go down again.
//!
//! # One transaction
//!
//! The token's level and position, and every light it carries, change
//! together or not at all. A torch left burning on the floor its bearer has
//! just left would light a room for people who can no longer see who holds
//! it.

use diesel::prelude::*;
use uuid::Uuid;

use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::interaction::{ActivationOutcome, FireMode, RegionGeometry, entered};
use thunderforge_canvas_core::navigation::{TRAVEL, partner_of};

use crate::models::{Interactive, Token};
use crate::world_events::{
    EVENT_CODE_INTERACTION_REQUEST, EVENT_CODE_LIGHT_SOURCE_CHANGED, EVENT_CODE_TOKEN_CHANGED,
    EVENT_CODE_TOKEN_TRAVELLED, record_world_event, world_id_for_scene,
};

/// Why a traveller did not arrive.
#[derive(Debug, PartialEq, Eq)]
pub enum TravelError {
    /// The partner is missing, malformed, or belongs to another scene.
    NoDestination,
    /// The token is not standing where this transition is.
    NotHere,
    Database(String),
}

impl From<diesel::result::Error> for TravelError {
    fn from(error: diesel::result::Error) -> Self {
        TravelError::Database(error.to_string())
    }
}

impl std::fmt::Display for TravelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TravelError::NoDestination => f.write_str("This way does not lead anywhere"),
            TravelError::NotHere => f.write_str("That token is not on this level"),
            TravelError::Database(_) => f.write_str("Failed to travel"),
        }
    }
}

/// What walking into a region came to.
#[derive(Debug)]
pub enum Arrival {
    /// The token is now somewhere else; this is the row as it stands.
    Travelled(Box<Token>),
    /// A Game Master has been asked. The token stays where it walked to.
    Requested(Uuid),
}

/// Whether an interactive is a way between levels.
pub fn is_travel(row: &Interactive) -> bool {
    row.effect_id.as_deref() == Some(TRAVEL)
}

/// The middle of a region's area.
fn centre_of(geometry: &RegionGeometry) -> Option<(f64, f64)> {
    match geometry {
        RegionGeometry::Rect {
            x,
            y,
            width,
            height,
        } => Some((
            f64::from(*x) + f64::from(*width) / 2.0,
            f64::from(*y) + f64::from(*height) / 2.0,
        )),
        RegionGeometry::Polygon { points } => {
            if points.is_empty() {
                return None;
            }
            let count = points.len() as f64;
            let (sum_x, sum_y) = points.iter().fold((0.0, 0.0), |(sx, sy), p| {
                (sx + f64::from(p[0]), sy + f64::from(p[1]))
            });
            Some((sum_x / count, sum_y / count))
        }
    }
}

/// Where a traveller arriving at `partner` is set down: the middle of a
/// region, the token a prop is, the midpoint of a door.
fn arrival_point(
    conn: &mut PgConnection,
    partner: &Interactive,
) -> QueryResult<Option<(f64, f64)>> {
    use crate::schema::{tokens, walls};

    match partner.subject_kind.as_str() {
        "region" => Ok(partner
            .geometry
            .clone()
            .and_then(|value| serde_json::from_value::<RegionGeometry>(value).ok())
            .and_then(|geometry| centre_of(&geometry))),
        "prop" => {
            let Some(token_id) = partner.subject_ref else {
                return Ok(None);
            };
            tokens::table
                .filter(tokens::token_id.eq(token_id))
                .select((tokens::x, tokens::y))
                .first::<(f64, f64)>(conn)
                .optional()
        }
        "door" => {
            let Some(wall_id) = partner.subject_ref else {
                return Ok(None);
            };
            Ok(walls::table
                .filter(walls::wall_id.eq(wall_id))
                .select((walls::x1, walls::y1, walls::x2, walls::y2))
                .first::<(f64, f64, f64, f64)>(conn)
                .optional()?
                .map(|(x1, y1, x2, y2)| ((x1 + x2) / 2.0, (y1 + y2) / 2.0)))
        }
        _ => Ok(None),
    }
}

/// The first free spot at or around `centre`, a grid step at a time.
///
/// A party taking the stairs one after another would otherwise arrive as a
/// single stack of tokens on one square. The spots are tried in a fixed
/// order — the centre, then the eight around it, then the next ring — so the
/// same arrivals always fan out the same way. If two rings are full the
/// traveller is put on the centre anyway: arriving stacked is better than
/// not arriving.
pub fn free_spot(centre: (f64, f64), step: f64, taken: &[(f64, f64)]) -> (f64, f64) {
    let near = step / 2.0;
    let is_free = |spot: (f64, f64)| {
        !taken
            .iter()
            .any(|other| (other.0 - spot.0).abs() < near && (other.1 - spot.1).abs() < near)
    };
    if is_free(centre) {
        return centre;
    }
    for ring in 1..=2_i32 {
        for dy in -ring..=ring {
            for dx in -ring..=ring {
                // Only the ring's edge: its inside was the previous ring.
                if dx.abs() != ring && dy.abs() != ring {
                    continue;
                }
                let spot = (
                    centre.0 + f64::from(dx) * step,
                    centre.1 + f64::from(dy) * step,
                );
                if is_free(spot) {
                    return spot;
                }
            }
        }
    }
    centre
}

/// Put `token_id` down at the partner of `via`.
///
/// The whole of travelling: the caller has already decided that this token
/// may go. Refuses when the partner is gone or is in another scene — a
/// transition never leaves its scene — and allows a partner on the same
/// level, which is a teleporter.
pub fn travel(
    conn: &mut PgConnection,
    token_id: Uuid,
    via: &Interactive,
    user_id: Uuid,
) -> Result<Token, TravelError> {
    use crate::schema::{interactives, light_sources, scenes, tokens};

    let partner_id = via
        .effect_config
        .as_ref()
        .and_then(partner_of)
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .ok_or(TravelError::NoDestination)?;

    let partner = interactives::table
        .filter(interactives::interactive_id.eq(partner_id))
        .select(Interactive::as_select())
        .first::<Interactive>(conn)
        .optional()?
        .filter(|partner| partner.scene_id == via.scene_id)
        .ok_or(TravelError::NoDestination)?;

    let centre = arrival_point(conn, &partner)?.ok_or(TravelError::NoDestination)?;
    let scene_id = via.scene_id;
    let level_id = partner.level_id;

    let (token, lights_moved) = conn.transaction::<_, TravelError, _>(|conn| {
        let step: i32 = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .select(scenes::grid_size)
            .first(conn)?;
        let taken = tokens::table
            .filter(tokens::scene_id.eq(scene_id))
            .filter(tokens::level_id.eq(level_id))
            .filter(tokens::token_id.ne(token_id))
            .select((tokens::x, tokens::y))
            .load::<(f64, f64)>(conn)?;
        let (x, y) = free_spot(centre, f64::from(step.max(1)), &taken);

        let token = diesel::update(
            tokens::table
                .filter(tokens::token_id.eq(token_id))
                .filter(tokens::scene_id.eq(scene_id)),
        )
        .set((
            tokens::level_id.eq(level_id),
            tokens::x.eq(x),
            tokens::y.eq(y),
        ))
        .returning(Token::as_returning())
        .get_result::<Token>(conn)
        .optional()?
        .ok_or(TravelError::NotHere)?;

        // The lights it carries go with it.
        let lights_moved = diesel::update(
            light_sources::table.filter(light_sources::attached_token_id.eq(token_id)),
        )
        .set((
            light_sources::level_id.eq(level_id),
            light_sources::x.eq(x),
            light_sources::y.eq(y),
        ))
        .execute(conn)?;

        Ok((token, lights_moved))
    })?;

    announce(conn, &token, lights_moved > 0, user_id);
    Ok(token)
}

/// Tell the table a token changed level.
///
/// The travel event names the token and the scene and **nothing else** — not
/// the level it left, not the one it reached. Every member of the world
/// receives every event, and where a token went is exactly what a player on
/// another floor is not told. Each client re-reads its own level, and the
/// server answers per viewer.
///
/// Shared with `moveTokensToLevel`, which is the same fact from a Game
/// Master's hand.
pub fn announce(conn: &mut PgConnection, token: &Token, lights_moved: bool, user_id: Uuid) {
    crate::scene_fingerprint::refresh_scene_fingerprint(conn, token.scene_id, user_id);
    let Ok(world_id) = world_id_for_scene(conn, token.scene_id) else {
        return;
    };
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_TOKEN_TRAVELLED,
        Some(serde_json::json!({
            "token_id": token.token_id,
            "scene_id": token.scene_id,
        })),
        user_id,
    );
    let _ = record_world_event(
        conn,
        world_id,
        EVENT_CODE_TOKEN_CHANGED,
        Some(serde_json::json!({
            "action": "updated",
            "token_id": token.token_id,
            "scene_id": token.scene_id,
        })),
        user_id,
    );
    if lights_moved {
        let _ = record_world_event(
            conn,
            world_id,
            EVENT_CODE_LIGHT_SOURCE_CHANGED,
            Some(serde_json::json!({
                "action": "updated",
                "scene_id": token.scene_id,
            })),
            user_id,
        );
    }
}

/// Take the one firing a `once` transition has, if it is still there.
///
/// The same conditional update `activateInteractive` makes, for the same
/// reason: two tokens stepping onto a one-way trapdoor in the same instant
/// must resolve to one of them falling.
fn claim_firing(conn: &mut PgConnection, interactive_id: Uuid, user_id: Uuid) -> QueryResult<bool> {
    use crate::schema::interactives;

    let now = chrono::Utc::now().naive_utc();
    let claimed = diesel::update(
        interactives::table
            .filter(interactives::interactive_id.eq(interactive_id))
            .filter(interactives::fired_at.is_null()),
    )
    .set((
        interactives::fired_at.eq(now),
        interactives::updated_by.eq(user_id),
        interactives::updated_at.eq(now),
    ))
    .execute(conn)?;
    Ok(claimed > 0)
}

/// Ask a Game Master, remembering which token asked.
///
/// The token is stored on the request because an approval arrives later and
/// from someone else: by then "the token that walked in" is not something
/// anyone can reconstruct.
pub fn raise_request(
    conn: &mut PgConnection,
    via: &Interactive,
    token_id: Uuid,
    user_id: Uuid,
) -> QueryResult<Uuid> {
    let request_id =
        crate::interaction::raise_request(conn, via.interactive_id, via.scene_id, user_id)?;
    remember_traveller(conn, request_id, token_id)?;
    Ok(request_id)
}

/// Record which token a request is for.
pub fn remember_traveller(
    conn: &mut PgConnection,
    request_id: Uuid,
    token_id: Uuid,
) -> QueryResult<()> {
    use crate::schema::interaction_requests as r;

    diesel::update(r::table.filter(r::request_id.eq(request_id)))
        .set(r::token_id.eq(Some(token_id)))
        .execute(conn)?;
    Ok(())
}

/// The token an approved travel request was raised for, if it still exists.
pub fn traveller_of_request(
    conn: &mut PgConnection,
    request_id: Uuid,
) -> QueryResult<Option<Uuid>> {
    use crate::schema::interaction_requests as r;

    Ok(r::table
        .filter(r::request_id.eq(request_id))
        .select(r::token_id)
        .first::<Option<Uuid>>(conn)
        .optional()?
        .flatten())
}

/// What a player's own move walked into, if anything.
///
/// Called by `moveOwnToken` once the move has been stored. `from` is where
/// the token stood before it; `token` is the row after it.
///
/// A transition that is Game-Master-only, already spent, or pointing nowhere
/// leaves the move standing and does nothing else — the player walked onto
/// the stairs and the stairs did not take them, which is not an error in
/// their move. Returns `None` for all of those.
pub fn on_entering(
    conn: &mut PgConnection,
    token: &Token,
    from: (f64, f64),
    user_id: Uuid,
    is_gm: bool,
) -> QueryResult<Option<Arrival>> {
    use crate::schema::interactives;

    let regions = interactives::table
        .filter(interactives::scene_id.eq(token.scene_id))
        .filter(interactives::level_id.eq(token.level_id))
        .filter(interactives::subject_kind.eq("region"))
        .filter(interactives::trigger.eq("enter"))
        .filter(interactives::effect_id.eq(TRAVEL))
        // By id, so a token crossing into two overlapping stairwells takes
        // the same one every time.
        .order(interactives::interactive_id.asc())
        .select(Interactive::as_select())
        .load::<Interactive>(conn)?;

    let previous = Vec2::new(from.0 as f32, from.1 as f32);
    let current = Vec2::new(token.x as f32, token.y as f32);
    let Some(via) = regions.into_iter().find(|row| {
        row.geometry
            .clone()
            .and_then(|value| serde_json::from_value::<RegionGeometry>(value).ok())
            .is_some_and(|geometry| entered(previous, current, &geometry))
    }) else {
        return Ok(None);
    };

    let loaded = crate::interaction::LoadedInteractive {
        row: via,
        // A region has no lock. A locked way between floors is a door.
        subject_locked: false,
    };
    match loaded.outcome(is_gm) {
        ActivationOutcome::Performed => {
            if loaded.fire_mode() == FireMode::Once
                && !claim_firing(conn, loaded.row.interactive_id, user_id)?
            {
                return Ok(None);
            }
            match travel(conn, token.token_id, &loaded.row, user_id) {
                Ok(arrived) => Ok(Some(Arrival::Travelled(Box::new(arrived)))),
                Err(TravelError::Database(reason)) => {
                    tracing::warn!(
                        token_id = %token.token_id,
                        reason,
                        "a token walked into a transition and could not be moved"
                    );
                    Ok(None)
                }
                Err(_) => Ok(None),
            }
        }
        ActivationOutcome::Requested => {
            let request_id = raise_request(conn, &loaded.row, token.token_id, user_id)?;
            if let Ok(world_id) = world_id_for_scene(conn, token.scene_id) {
                let _ = record_world_event(
                    conn,
                    world_id,
                    EVENT_CODE_INTERACTION_REQUEST,
                    Some(serde_json::json!({
                        "action": "raised",
                        "request_id": request_id,
                        "interactive_id": loaded.row.interactive_id,
                        "scene_id": token.scene_id,
                    })),
                    user_id,
                );
            }
            Ok(Some(Arrival::Requested(request_id)))
        }
        _ => Ok(None),
    }
}

/// The token a click sends, checked.
///
/// A click has to say who is going: a player may control more than one token,
/// and the server does not guess. The token must be the caller's to move, and
/// must be standing on the level the transition is on — a ladder is not
/// climbed from another floor.
pub fn traveller_for_click(
    conn: &mut PgConnection,
    via: &Interactive,
    token_id: Option<Uuid>,
    user_id: Uuid,
    is_admin: bool,
) -> Result<Uuid, String> {
    use crate::schema::tokens;

    let token_id = token_id.ok_or_else(|| String::from("Choose which token travels"))?;
    let refused = || String::from("That token cannot travel this way");

    let control = crate::combat::controllers::token_control(conn, token_id)
        .map_err(|_| refused())?
        .ok_or_else(refused)?;
    if !crate::combat::controllers::may_move(conn, user_id, is_admin, &control).unwrap_or(false) {
        return Err(refused());
    }
    let (scene_id, level_id) = tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select((tokens::scene_id, tokens::level_id))
        .first::<(Uuid, Uuid)>(conn)
        .map_err(|_| refused())?;
    if scene_id != via.scene_id || level_id != via.level_id {
        return Err(refused());
    }
    Ok(token_id)
}

#[cfg(test)]
#[path = "level_travel_tests.rs"]
mod tests;
