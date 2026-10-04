//! Judging a player's move against the scene's walls.
//!
//! The rule is ADR-095: the engine stops a move so it *feels* like a wall, and
//! this decides whether it *was* one. The geometry is not here — it is
//! [`thunderforge_canvas_core::wall`], the same functions the engine calls, so
//! the two answers cannot drift.
//!
//! What is here is the part the crate deliberately does not know: which of a
//! request's claims to believe. A path is a client's account of how it got
//! somewhere, and this module is where that account is checked for shape
//! before it is used to decide anything.
//!
//! A Game Master is never judged (spec 045 decision 1, FR-017). That is a rule
//! about who is in charge of a table, not a check to be tightened later, and
//! it is why this is reached from `moveOwnToken` and not from `updateToken`.

use thunderforge_canvas_core::Vec2;
use thunderforge_canvas_core::wall::{
    DoorState, Wall, WallSet, movement_blocked_by, path_blocked_by,
};

/// The longest route a client may claim in one move.
///
/// A route is walked a cell at a time, and nobody walks 64 cells between two
/// position updates — a 5-foot grid makes that 320 feet, far past any
/// creature's turn. The bound exists because the path arrives from a client:
/// without it, a move is an unbounded amount of geometry for the server to
/// intersect, which is a cost a player chooses and the instance pays.
pub const MAX_PATH_POINTS: usize = 64;

/// Why a move was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// A wall — or a closed door, which is a wall that can be opened — lies
    /// across the move. Carries the wall so the caller can say where.
    ///
    /// The wall's id is for the client's own use (drawing the stop); the
    /// message a player is shown must not say more than "a wall is in the
    /// way". A secret door is a wall, and FR-019 forbids the refusal
    /// revealing that it is a door.
    Wall { wall_id: String },
    /// The claimed route is not a route: too many points, or a coordinate
    /// that is not a number. Not the same as being blocked, and worth
    /// separating — this one is a client fault, not a table event.
    MalformedPath,
}

/// The verdict on one move.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Allowed,
    Refused(Refusal),
}

impl Verdict {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Verdict::Allowed)
    }
}

/// Judge a move from `from` to `to`, optionally along a claimed route.
///
/// With no route, the straight line from `from` to `to` is judged. That is the
/// right answer for a drag, which *is* a straight line, and the safe answer for
/// anything else: a client that wants a longer way round accepted has to say
/// what it was.
///
/// With a route, every leg is judged, **and so are the joins to the endpoints**
/// — the route is anchored to where the token actually is and where it claims
/// to end. Without that anchoring a client could send a short, innocent route
/// somewhere else entirely and have it accepted as cover for a move that
/// crossed a wall.
pub fn judge(from: Vec2, to: Vec2, path: Option<&[Vec2]>, walls: &WallSet) -> Verdict {
    let Some(path) = path else {
        return match movement_blocked_by(from, to, walls) {
            Some(wall) => Verdict::Refused(Refusal::Wall {
                wall_id: wall.id.clone(),
            }),
            None => Verdict::Allowed,
        };
    };

    if path.len() > MAX_PATH_POINTS {
        return Verdict::Refused(Refusal::MalformedPath);
    }
    if path.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        // NaN compares false against everything, so an infinite or
        // not-a-number coordinate does not merely give a strange answer — it
        // slips past the intersection test entirely.
        return Verdict::Refused(Refusal::MalformedPath);
    }

    // The token's real position, the claimed route, and the requested
    // destination, as one continuous walk. Duplicated points are harmless:
    // a zero-length leg is never blocked.
    let mut walked = Vec::with_capacity(path.len() + 2);
    walked.push(from);
    walked.extend_from_slice(path);
    walked.push(to);

    match path_blocked_by(&walked, walls) {
        Some(wall) => Verdict::Refused(Refusal::Wall {
            wall_id: wall.id.clone(),
        }),
        None => Verdict::Allowed,
    }
}

/// The scene's walls, as the shared geometry wants them.
///
/// The server stores coordinates as `f64` and door state as a string; the
/// crate works in `f32` and an enum, because that is what the engine renders
/// with. This is the one place the two meet, so a rounding decision is made
/// once rather than at each call site.
pub fn wall_set_from_rows(rows: &[crate::models::Wall]) -> WallSet {
    let mut set = WallSet::default();
    for row in rows {
        set.upsert(Wall {
            id: row.wall_id.to_string(),
            x1: row.x1 as f32,
            y1: row.y1 as f32,
            x2: row.x2 as f32,
            y2: row.y2 as f32,
            blocks_vision: row.blocks_vision,
            blocks_movement: row.blocks_movement,
            door_state: DoorState::from_str_loose(&row.door_state),
            locked: row.locked,
            secret: row.secret,
        });
    }
    set
}

/// What a player is told when a wall refuses their move.
///
/// One sentence, the same for every wall, deliberately saying nothing about
/// doors: a closed secret door stops a player like any wall, and the refusal
/// must not be how they learn there is a door there (FR-019).
pub const BLOCKED_MESSAGE: &str = "A wall is in the way";

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
