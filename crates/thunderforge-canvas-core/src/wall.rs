use glam::Vec2;

/// Door state for a wall, mirroring `door_state` in the server schema
/// (data-model.md's Wall section). `None` = ordinary wall (not a door);
/// `Open`/`Closed` = a door in that state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DoorState {
    #[default]
    None,
    Open,
    Closed,
}

impl DoorState {
    pub fn from_str_loose(value: &str) -> Self {
        match value {
            "open" => DoorState::Open,
            "closed" => DoorState::Closed,
            _ => DoorState::None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DoorState::None => "none",
            DoorState::Open => "open",
            DoorState::Closed => "closed",
        }
    }
}

/// A single wall segment (data-model.md's Wall section). `id` is `None`
/// for a locally-drawn-but-not-yet-server-confirmed wall (the server
/// assigns the real UUID on create) — this is deliberately simple rather
/// than a full temp-ID reconciliation system.
#[derive(Debug, Clone, PartialEq)]
pub struct Wall {
    pub id: String,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub blocks_vision: bool,
    pub blocks_movement: bool,
    pub door_state: DoorState,
    /// Who may change the door's state — not the state itself.
    ///
    /// Deliberately a separate flag rather than a third `DoorState`. As one
    /// enum, "open, and players cannot close it" — a spiked-open portcullis —
    /// becomes inexpressible, and opening a locked door forces a decision
    /// about what happens to the lock that a separate flag never raises.
    ///
    /// A locked door refuses a player's state change and accepts the Game
    /// Master's (FR-013).
    pub locked: bool,
    /// A door the players are not shown until it is revealed.
    ///
    /// Presentation only. Per the spec's decision the geometry still reaches
    /// every client; it is the drawing that differs, because a player who
    /// inspects their own client and announces a secret door has created a
    /// table problem rather than found a security hole.
    pub secret: bool,
}

/// What a segment blocks *right now*.
///
/// Returned as a pair rather than two calls because vision and movement are
/// decided by the same rule and answering them separately invites the two
/// from drifting apart — a closed window that stops arrows and light would be
/// two lines of code away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocking {
    pub vision: bool,
    pub movement: bool,
}

impl Wall {
    pub fn start(&self) -> Vec2 {
        Vec2::new(self.x1, self.y1)
    }

    pub fn end(&self) -> Vec2 {
        Vec2::new(self.x2, self.y2)
    }

    pub fn midpoint(&self) -> Vec2 {
        (self.start() + self.end()) / 2.0
    }

    pub fn length(&self) -> f32 {
        self.start().distance(self.end())
    }

    /// The angle (radians) of the segment from start to end, for rotating
    /// the rendered sprite to match.
    pub fn angle(&self) -> f32 {
        let delta = self.end() - self.start();
        delta.y.atan2(delta.x)
    }

    /// Whether this wall *currently* blocks vision, accounting for door
    /// semantics (data-model.md: an open door never blocks regardless of
    /// its stored `blocks_vision` flag; a closed door or non-door wall
    /// applies the stored flag as-is).
    pub fn currently_blocks_vision(&self) -> bool {
        if self.door_state == DoorState::Open {
            return false;
        }
        self.blocks_vision
    }

    /// Whether this wall *currently* blocks movement, same door semantics
    /// as `currently_blocks_vision`.
    pub fn currently_blocks_movement(&self) -> bool {
        if self.door_state == DoorState::Open {
            return false;
        }
        self.blocks_movement
    }

    /// The definition FR-008 and FR-009 asked for.
    ///
    /// Open blocks neither. Closed — and a plain wall, which is the same
    /// thing for this purpose — blocks exactly what the wall's own profile
    /// says. Deriving the closed state from the profile rather than storing a
    /// second set of flags is what keeps a closed window see-through and a
    /// closed stone door not, with nothing to keep consistent.
    ///
    /// A closed door is therefore indistinguishable from a plain wall in what
    /// it blocks. That is correct: the difference is that it can be opened.
    pub fn blocking(&self) -> Blocking {
        Blocking {
            vision: self.currently_blocks_vision(),
            movement: self.currently_blocks_movement(),
        }
    }

    /// Whether this segment has been designated a door at all.
    pub fn is_door(&self) -> bool {
        self.door_state != DoorState::None
    }

    /// Whether `actor_is_gm` may change this door's state.
    ///
    /// The lock is the only thing that separates them; a Game Master is never
    /// refused their own door.
    pub fn may_change_state(&self, actor_is_gm: bool) -> bool {
        self.is_door() && (actor_is_gm || !self.locked)
    }
}

// ---------------------------------------------------------------------------
// What doors contribute to the interaction seam (spec 030)
// ---------------------------------------------------------------------------

/// Set a door open or closed.
pub const SET_STATE: &str = "door.set_state";
/// Lock or unlock a door.
pub const SET_LOCK: &str = "door.set_lock";
/// Reveal a secret door.
pub const REVEAL: &str = "door.reveal";

/// What a door reference points at.
pub const WALL: &str = "wall";
/// The configuration key naming the door.
pub const TARGET_KEY: &str = "target";
/// The configuration key carrying the desired state.
pub const STATE_KEY: &str = "state";
/// The configuration key carrying the desired lock.
pub const LOCKED_KEY: &str = "locked";

/// What doors contribute to the interaction registry.
///
/// Declared *here*, beside doors, rather than in `crate::interaction`. That is
/// the whole point of the seam: doors are the effect most tempting to build
/// into the interaction core, because they are the most obviously spatial
/// thing on a map. `scripts/verify.mjs` greps that core for the word so the
/// temptation is visible rather than a matter of judgement (FR-039).
pub fn interaction_effects() -> Vec<crate::interaction::EffectDeclaration> {
    use crate::interaction::{
        ChoiceOption, ConfigField, ConfigFieldKind, EffectDeclaration, SubjectKind,
    };

    let target = |required: bool| ConfigField {
        key: TARGET_KEY.to_string(),
        label: String::from("Which door"),
        kind: ConfigFieldKind::Reference {
            of: WALL.to_string(),
        },
        required,
    };

    vec![
        EffectDeclaration {
            id: SET_STATE.to_string(),
            label: String::from("Open or close a door"),
            description: String::from("Swings a door, or toggles whichever way it is now."),
            // A lever on the wall, the door itself, or a threshold the party
            // crosses — all three are things a Game Master reaches for.
            subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region],
            config: vec![
                target(true),
                ConfigField {
                    key: STATE_KEY.to_string(),
                    label: String::from("Set it to"),
                    kind: ConfigFieldKind::Choice {
                        options: vec![
                            ChoiceOption {
                                value: String::from("open"),
                                label: String::from("Open"),
                            },
                            ChoiceOption {
                                value: String::from("closed"),
                                label: String::from("Closed"),
                            },
                            // The commonest case, and the one a single lever
                            // wants: whatever it is now, make it the other.
                            ChoiceOption {
                                value: String::from("toggle"),
                                label: String::from("The other way"),
                            },
                        ],
                    },
                    required: true,
                },
            ],
        },
        EffectDeclaration {
            id: SET_LOCK.to_string(),
            label: String::from("Lock or unlock a door"),
            description: String::from("Changes who may open it, not whether it is open."),
            subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door],
            config: vec![
                target(true),
                ConfigField {
                    key: LOCKED_KEY.to_string(),
                    label: String::from("Locked"),
                    kind: ConfigFieldKind::Boolean,
                    required: true,
                },
            ],
        },
        EffectDeclaration {
            id: REVEAL.to_string(),
            label: String::from("Reveal a secret door"),
            description: String::from("Shows the table a door that was not drawn for them."),
            subject_kinds: vec![SubjectKind::Prop, SubjectKind::Door, SubjectKind::Region],
            config: vec![target(true)],
        },
    ]
}

/// What a configured door effect points at.
pub fn target_of(config: &serde_json::Value) -> Option<&str> {
    config.get(TARGET_KEY)?.as_str()
}

/// What state a `door.set_state` asks for, resolved against where it is now.
///
/// `toggle` is resolved here rather than by the caller so that "the other way"
/// means the same thing everywhere — and so that a lever wired to a door that
/// is already open does the obvious thing rather than nothing.
pub fn requested_state(config: &serde_json::Value, current: DoorState) -> Option<DoorState> {
    match config.get(STATE_KEY)?.as_str()? {
        "open" => Some(DoorState::Open),
        "closed" => Some(DoorState::Closed),
        "toggle" => Some(match current {
            DoorState::Open => DoorState::Closed,
            // A wall that is not a door has nothing to toggle, but a toggle
            // aimed at one is a misconfiguration rather than a reason to
            // invent a state — closed is what "not open" means.
            DoorState::Closed | DoorState::None => DoorState::Open,
        }),
        _ => None,
    }
}

/// Whether a `door.set_lock` asks for locked or unlocked.
pub fn requested_lock(config: &serde_json::Value) -> Option<bool> {
    config.get(LOCKED_KEY)?.as_bool()
}

/// One reversible wall edit, pushed onto `WallSet`'s undo stack whenever a
/// confirmed edit is applied locally. Undo re-issues the inverse as a
/// normal outbound mutation (research.md §4) rather than a special
/// client-only rollback.
#[derive(Debug, Clone)]
pub enum WallEdit {
    /// A wall was moved (endpoint drag). Undo re-issues `update_wall` with
    /// the prior endpoints.
    Move {
        wall_id: String,
        prior_x1: f32,
        prior_y1: f32,
        prior_x2: f32,
        prior_y2: f32,
    },
    /// A wall's door state was toggled. Undo re-issues `update_wall` with
    /// the prior door state.
    DoorToggle {
        wall_id: String,
        prior_door_state: DoorState,
    },
    /// A wall's blocks_vision/blocks_movement flags were toggled. Undo
    /// re-issues `update_wall` with the prior flags.
    FlagsToggle {
        wall_id: String,
        prior_blocks_vision: bool,
        prior_blocks_movement: bool,
    },
    /// A wall was deleted. Undo re-issues `create_wall` with the wall's
    /// full prior state (note: the re-created wall gets a *new* server-
    /// assigned id, it cannot resurrect the original id).
    Delete { deleted: Wall },
}

const MAX_UNDO_STACK: usize = 50;

/// Segment list + door state for the scene's walls, plus a bounded
/// per-session undo stack (research.md §4). A `Vec` is an intentionally
/// simple "spatial index" here — the scale involved (tens to low-hundreds
/// of walls per scene) doesn't justify a BVH/grid.
///
/// Plain data, no Bevy `Resource` derive — `thunderforge_engine` wraps
/// this in a `Resource` newtype (`src/engine/src/resources/wall.rs`) so
/// Bevy's change-detection still works transparently on `ResMut` access.
#[derive(Debug, Clone, Default)]
pub struct WallSet {
    walls: Vec<Wall>,
    undo_stack: Vec<WallEdit>,
    /// Set whenever the wall list changes, so occlusion-recompute systems
    /// can react to `WallSet` changes — cleared by the system that
    /// consumes it.
    pub dirty: bool,
}

impl WallSet {
    pub fn walls(&self) -> &[Wall] {
        &self.walls
    }

    pub fn get(&self, id: &str) -> Option<&Wall> {
        self.walls.iter().find(|w| w.id == id)
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.walls.iter().position(|w| w.id == id)
    }

    /// Insert-or-update by id, mirroring the `TokenEntities` upsert
    /// pattern used in lib.rs for tokens.
    pub fn upsert(&mut self, wall: Wall) {
        if let Some(index) = self.index_of(&wall.id) {
            self.walls[index] = wall;
        } else {
            self.walls.push(wall);
        }
        self.dirty = true;
    }

    /// Removes and returns the wall with the given id, if present.
    pub fn remove(&mut self, id: &str) -> Option<Wall> {
        let index = self.index_of(id)?;
        self.dirty = true;
        Some(self.walls.remove(index))
    }

    /// Drop every wall, and the undo history that referred to them.
    ///
    /// For a scene change (spec 031 FR-018), where the previous scene's
    /// geometry has to stop existing rather than be edited away one wall at a
    /// time. The undo stack goes with it deliberately: an undo entry names a
    /// wall id from a scene nobody is looking at any more, and replaying one
    /// would emit a mutation against the wrong scene.
    pub fn clear(&mut self) {
        self.walls.clear();
        self.undo_stack.clear();
        self.dirty = true;
    }

    pub fn push_undo(&mut self, edit: WallEdit) {
        self.undo_stack.push(edit);
        if self.undo_stack.len() > MAX_UNDO_STACK {
            self.undo_stack.remove(0);
        }
    }

    pub fn pop_undo(&mut self) -> Option<WallEdit> {
        self.undo_stack.pop()
    }

    pub fn undo_stack_len(&self) -> usize {
        self.undo_stack.len()
    }

    /// Only vision-blocking walls (door-state-aware) are relevant to
    /// occlusion checks.
    pub fn vision_blocking_walls(&self) -> impl Iterator<Item = &Wall> {
        self.walls.iter().filter(|w| w.currently_blocks_vision())
    }
}

/// The four segments of an axis-aligned room drawn between two opposite
/// corners.
///
/// Spec 031 FR-026. A room is the commonest thing a Game Master draws and the
/// slowest to draw by hand: four segments whose ends have to meet exactly, or
/// the vision pass leaks light through the seam. Building it from the four
/// corner points guarantees they meet, because adjacent segments are handed
/// the *same* [`Vec2`], not two values that round to the same place.
///
/// Returned in a closed loop starting at the lower-left corner and running
/// anticlockwise, so the caller emits them in a predictable order.
///
/// `None` for a degenerate rectangle — a drag with no width or no height. That
/// is a room with no interior, and the honest answer is that the gesture did
/// not describe one; drawing two pairs of coincident walls instead would leave
/// the Game Master with four walls stacked on a line and no way to see it.
///
/// Corner order is normalised, so dragging up-left produces the same room as
/// dragging down-right. Nothing downstream should have to care which way the
/// hand moved.
pub fn room_segments(a: Vec2, b: Vec2) -> Option<[(Vec2, Vec2); 4]> {
    let min = Vec2::new(a.x.min(b.x), a.y.min(b.y));
    let max = Vec2::new(a.x.max(b.x), a.y.max(b.y));

    if !(max.x - min.x).is_finite() || !(max.y - min.y).is_finite() {
        return None;
    }
    if max.x - min.x <= f32::EPSILON || max.y - min.y <= f32::EPSILON {
        return None;
    }

    let bottom_left = min;
    let bottom_right = Vec2::new(max.x, min.y);
    let top_right = max;
    let top_left = Vec2::new(min.x, max.y);

    Some([
        (bottom_left, bottom_right),
        (bottom_right, top_right),
        (top_right, top_left),
        (top_left, bottom_left),
    ])
}

/// Returns true if segment `p1`-`p2` intersects segment `p3`-`p4`.
/// Standard orientation-based segment intersection test (does not treat
/// shared endpoints/collinear overlap as a special case beyond the
/// orientation math naturally handling it).
fn segments_intersect(p1: Vec2, p2: Vec2, p3: Vec2, p4: Vec2) -> bool {
    fn cross(o: Vec2, a: Vec2, b: Vec2) -> f32 {
        (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
    }

    fn on_segment(p: Vec2, q: Vec2, r: Vec2) -> bool {
        q.x <= p.x.max(r.x) && q.x >= p.x.min(r.x) && q.y <= p.y.max(r.y) && q.y >= p.y.min(r.y)
    }

    let d1 = cross(p3, p4, p1);
    let d2 = cross(p3, p4, p2);
    let d3 = cross(p1, p2, p3);
    let d4 = cross(p1, p2, p4);

    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
        && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
    {
        return true;
    }

    if d1 == 0.0 && on_segment(p3, p1, p4) {
        return true;
    }
    if d2 == 0.0 && on_segment(p3, p2, p4) {
        return true;
    }
    if d3 == 0.0 && on_segment(p1, p3, p2) {
        return true;
    }
    if d4 == 0.0 && on_segment(p1, p4, p2) {
        return true;
    }

    false
}

/// 2D shadow-casting occlusion check (T013, research.md §3): is `target`
/// visible from `observer`, given the current set of vision-blocking
/// walls? Returns false if the observer-target segment crosses any wall
/// with `currently_blocks_vision() == true` (door-state-aware — an open
/// door never blocks, per data-model.md).
///
/// This is the pure, unit-testable geometry core of vision occlusion.
/// Applying it to actually render fog-of-war (hiding/dimming tokens,
/// eventually a full shadow mesh) is done by a separate system in
/// `thunderforge_engine` that calls this function.
pub fn is_visible(observer: Vec2, target: Vec2, walls: &WallSet) -> bool {
    for wall in walls.vision_blocking_walls() {
        if segments_intersect(observer, target, wall.start(), wall.end()) {
            return false;
        }
    }
    true
}

#[cfg(test)]
#[path = "wall_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wall_room_and_clear_tests.rs"]
mod room_and_clear_tests;
