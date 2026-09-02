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
mod tests {
    use super::*;

    fn wall(id: &str, x1: f32, y1: f32, x2: f32, y2: f32) -> Wall {
        Wall {
            id: id.to_string(),
            x1,
            y1,
            x2,
            y2,
            blocks_vision: true,
            blocks_movement: false,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        }
    }

    #[test]
    fn door_state_round_trips() {
        assert_eq!(DoorState::from_str_loose("open"), DoorState::Open);
        assert_eq!(DoorState::from_str_loose("closed"), DoorState::Closed);
        assert_eq!(DoorState::from_str_loose("none"), DoorState::None);
        assert_eq!(DoorState::from_str_loose("garbage"), DoorState::None);
        assert_eq!(DoorState::Open.as_str(), "open");
        assert_eq!(DoorState::Closed.as_str(), "closed");
        assert_eq!(DoorState::None.as_str(), "none");
    }

    #[test]
    fn open_door_never_blocks_regardless_of_stored_flags() {
        let mut w = wall("door-1", 0.0, 0.0, 10.0, 0.0);
        w.door_state = DoorState::Open;
        w.blocks_vision = true;
        w.blocks_movement = true;

        assert!(!w.currently_blocks_vision());
        assert!(!w.currently_blocks_movement());
    }

    #[test]
    fn closed_door_applies_stored_flags() {
        let mut w = wall("door-1", 0.0, 0.0, 10.0, 0.0);
        w.door_state = DoorState::Closed;
        w.blocks_vision = true;
        w.blocks_movement = false;

        assert!(w.currently_blocks_vision());
        assert!(!w.currently_blocks_movement());
    }

    #[test]
    fn non_door_wall_always_applies_stored_flags() {
        let mut w = wall("wall-1", 0.0, 0.0, 10.0, 0.0);
        w.door_state = DoorState::None;
        w.blocks_vision = false;
        w.blocks_movement = true;

        assert!(!w.currently_blocks_vision());
        assert!(w.currently_blocks_movement());
    }

    #[test]
    fn segments_intersect_crossing() {
        assert!(segments_intersect(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
            Vec2::new(10.0, 0.0),
        ));
    }

    #[test]
    fn segments_intersect_parallel_non_touching() {
        assert!(!segments_intersect(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(0.0, 5.0),
            Vec2::new(10.0, 5.0),
        ));
    }

    #[test]
    fn is_visible_true_with_no_walls() {
        let walls = WallSet::default();
        assert!(is_visible(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            &walls
        ));
    }

    #[test]
    fn is_visible_false_when_blocking_wall_crosses_line_of_sight() {
        let mut walls = WallSet::default();
        walls.upsert(wall("w1", 50.0, -10.0, 50.0, 10.0));

        assert!(!is_visible(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            &walls
        ));
    }

    #[test]
    fn is_visible_true_when_wall_does_not_cross_line_of_sight() {
        let mut walls = WallSet::default();
        walls.upsert(wall("w1", 50.0, 20.0, 50.0, 40.0));

        assert!(is_visible(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            &walls
        ));
    }

    #[test]
    fn is_visible_true_when_blocking_wall_is_open_door() {
        let mut walls = WallSet::default();
        let mut door = wall("door-1", 50.0, -10.0, 50.0, 10.0);
        door.door_state = DoorState::Open;
        walls.upsert(door);

        assert!(is_visible(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            &walls
        ));
    }

    #[test]
    fn is_visible_false_when_blocking_wall_is_closed_door() {
        let mut walls = WallSet::default();
        let mut door = wall("door-1", 50.0, -10.0, 50.0, 10.0);
        door.door_state = DoorState::Closed;
        walls.upsert(door);

        assert!(!is_visible(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            &walls
        ));
    }

    #[test]
    fn is_visible_true_when_wall_does_not_block_vision() {
        let mut walls = WallSet::default();
        let mut w = wall("w1", 50.0, -10.0, 50.0, 10.0);
        w.blocks_vision = false;
        walls.upsert(w);

        assert!(is_visible(
            Vec2::new(0.0, 0.0),
            Vec2::new(100.0, 0.0),
            &walls
        ));
    }

    #[test]
    fn is_visible_light_occlusion_combined_wall_and_door_scenario() {
        // T065: a combined light+wall+door geometry scenario. `is_visible`
        // is shared verbatim between vision occlusion (systems/wall.rs) and
        // light occlusion (systems/lighting.rs's `apply_light_illumination`
        // calls it as `is_visible(light_position, target, &wall_set)`), so
        // this framing uses `observer` as a light source's position and
        // `target` as an illuminated point, alongside an irrelevant
        // non-blocking wall and an irrelevant open door elsewhere in the
        // scene, then flips the blocking wall's door state on the *same*
        // `WallSet` (rather than two separate fixtures, unlike the existing
        // `is_visible_true/false_when_blocking_wall_is_open/closed_door`
        // tests above) to prove a door opening dynamically restores light
        // through a previously-blocked path.
        let light_position = Vec2::new(0.0, 0.0);
        let illuminated_point = Vec2::new(100.0, 0.0);

        let mut walls = WallSet::default();
        // Directly between the light and the point, closed: should block.
        let mut blocking_door = wall("door-1", 50.0, -10.0, 50.0, 10.0);
        blocking_door.door_state = DoorState::Closed;
        walls.upsert(blocking_door);
        // Off to the side: irrelevant to this light/target pair, blocks
        // vision in general but doesn't cross this particular segment.
        walls.upsert(wall("w-decoy", 50.0, 20.0, 50.0, 40.0));
        // Another open door elsewhere: also irrelevant, present to prove
        // the check isn't accidentally short-circuiting on "any door".
        let mut decoy_open_door = wall("door-2", 20.0, 20.0, 20.0, 40.0);
        decoy_open_door.door_state = DoorState::Open;
        walls.upsert(decoy_open_door);

        assert!(
            !is_visible(light_position, illuminated_point, &walls),
            "closed door directly between light and target should block illumination"
        );

        // Same WallSet, same wall id: open the door and re-check. This is
        // the "door opens" transition a live session would produce via
        // `handle_wall_keyboard_toggles`'s `O` keybind + `WallSet::upsert`.
        let mut reopened = walls.get("door-1").cloned().unwrap();
        reopened.door_state = DoorState::Open;
        walls.upsert(reopened);

        assert!(
            is_visible(light_position, illuminated_point, &walls),
            "opening the same door should restore illumination through it"
        );
    }

    #[test]
    fn upsert_inserts_then_updates_by_id() {
        let mut walls = WallSet::default();
        walls.upsert(wall("w1", 0.0, 0.0, 1.0, 1.0));
        assert_eq!(walls.walls().len(), 1);

        walls.upsert(wall("w1", 5.0, 5.0, 6.0, 6.0));
        assert_eq!(walls.walls().len(), 1);
        assert_eq!(walls.get("w1").unwrap().x1, 5.0);
    }

    #[test]
    fn remove_returns_removed_wall() {
        let mut walls = WallSet::default();
        walls.upsert(wall("w1", 0.0, 0.0, 1.0, 1.0));

        let removed = walls.remove("w1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "w1");
        assert!(walls.get("w1").is_none());
    }

    #[test]
    fn undo_stack_is_bounded() {
        let mut walls = WallSet::default();
        for i in 0..(MAX_UNDO_STACK + 10) {
            walls.push_undo(WallEdit::DoorToggle {
                wall_id: format!("w{i}"),
                prior_door_state: DoorState::None,
            });
        }
        assert_eq!(walls.undo_stack_len(), MAX_UNDO_STACK);
    }

    #[test]
    fn undo_stack_pops_most_recent_first() {
        let mut walls = WallSet::default();
        walls.push_undo(WallEdit::DoorToggle {
            wall_id: "first".to_string(),
            prior_door_state: DoorState::None,
        });
        walls.push_undo(WallEdit::DoorToggle {
            wall_id: "second".to_string(),
            prior_door_state: DoorState::None,
        });

        match walls.pop_undo() {
            Some(WallEdit::DoorToggle { wall_id, .. }) => assert_eq!(wall_id, "second"),
            _ => panic!("expected DoorToggle edit"),
        }
    }

    #[test]
    fn wall_geometry_helpers() {
        let w = wall("w1", 0.0, 0.0, 10.0, 0.0);
        assert_eq!(w.length(), 10.0);
        assert_eq!(w.midpoint(), Vec2::new(5.0, 0.0));
        assert_eq!(w.angle(), 0.0);
    }

    #[test]
    fn zero_length_wall_has_zero_length() {
        let w = wall("w1", 5.0, 5.0, 5.0, 5.0);
        assert_eq!(w.length(), 0.0);
    }

    // --- doors: what open, closed and locked actually mean ----------------

    #[test]
    fn a_closed_window_stays_see_through_and_a_closed_stone_door_does_not() {
        // The whole reason closed blocking is *derived* from the wall's own
        // profile rather than stored a second time. Two doors, same state,
        // different materials, and nothing had to be kept consistent.
        let mut window = wall("window", 0.0, 0.0, 10.0, 0.0);
        window.blocks_vision = false;
        window.blocks_movement = true;
        window.door_state = DoorState::Closed;

        let mut stone = wall("stone", 0.0, 0.0, 10.0, 0.0);
        stone.blocks_vision = true;
        stone.blocks_movement = true;
        stone.door_state = DoorState::Closed;

        assert_eq!(
            window.blocking(),
            Blocking {
                vision: false,
                movement: true
            }
        );
        assert_eq!(
            stone.blocking(),
            Blocking {
                vision: true,
                movement: true
            }
        );
    }

    #[test]
    fn an_open_door_blocks_neither_whatever_it_is_made_of() {
        let mut door = wall("door", 0.0, 0.0, 10.0, 0.0);
        door.blocks_vision = true;
        door.blocks_movement = true;
        door.door_state = DoorState::Open;

        assert_eq!(
            door.blocking(),
            Blocking {
                vision: false,
                movement: false
            }
        );
    }

    #[test]
    fn a_closed_door_is_indistinguishable_from_a_plain_wall_in_what_it_blocks() {
        // Correct, and worth pinning: the difference is that it can be opened,
        // not that it stops anything differently.
        let mut plain = wall("plain", 0.0, 0.0, 10.0, 0.0);
        plain.blocks_movement = true;
        let mut door = plain.clone();
        door.id = String::from("door");
        door.door_state = DoorState::Closed;

        assert_eq!(plain.blocking(), door.blocking());
        assert!(!plain.is_door());
        assert!(door.is_door());
    }

    #[test]
    fn lock_is_independent_of_state_so_a_spiked_open_door_is_expressible() {
        // The case a three-state Open/Closed/Locked enum cannot represent, and
        // the reason `locked` is a separate flag (FR-010).
        let mut portcullis = wall("portcullis", 0.0, 0.0, 10.0, 0.0);
        portcullis.blocks_vision = true;
        portcullis.blocks_movement = true;
        portcullis.door_state = DoorState::Open;
        portcullis.locked = true;

        // Open, so it blocks nothing...
        assert_eq!(
            portcullis.blocking(),
            Blocking {
                vision: false,
                movement: false
            }
        );
        // ...and no player can shut it.
        assert!(!portcullis.may_change_state(false));
        assert!(portcullis.may_change_state(true));
    }

    #[test]
    fn locking_changes_who_may_act_and_never_what_the_door_blocks() {
        let mut door = wall("door", 0.0, 0.0, 10.0, 0.0);
        door.blocks_vision = true;
        door.blocks_movement = true;
        door.door_state = DoorState::Closed;

        let unlocked = door.blocking();
        door.locked = true;
        assert_eq!(door.blocking(), unlocked);
    }

    #[test]
    fn a_plain_wall_has_no_state_for_anyone_to_change() {
        let plain = wall("plain", 0.0, 0.0, 10.0, 0.0);
        assert!(!plain.may_change_state(false));
        // Not even the Game Master — there is no door here to open.
        assert!(!plain.may_change_state(true));
    }

    #[test]
    fn secret_is_presentation_and_touches_neither_blocking_nor_permission() {
        let mut door = wall("door", 0.0, 0.0, 10.0, 0.0);
        door.blocks_movement = true;
        door.door_state = DoorState::Closed;

        let before = door.blocking();
        door.secret = true;
        assert_eq!(door.blocking(), before);
        assert!(door.may_change_state(false));
    }

    // --- what doors contribute --------------------------------------------

    #[test]
    fn the_declarations_are_namespaced_and_assemble() {
        use crate::interaction::EffectRegistry;

        let registry = EffectRegistry::assemble([interaction_effects()]).expect("one contributor");
        assert_eq!(registry.len(), 3);
        for declaration in registry.all() {
            assert_eq!(declaration.namespace(), "door");
        }
    }

    #[test]
    fn only_a_lock_effect_may_be_put_on_something_that_cannot_be_crossed() {
        // `door.set_lock` is deliberately not offered on a region. Locking a
        // door by walking past it is a thing no Game Master asked for, and
        // offering it would put a footgun in the authoring form.
        let effects = interaction_effects();
        let lock = effects.iter().find(|e| e.id == SET_LOCK).expect("declared");
        assert!(
            !lock
                .subject_kinds
                .contains(&crate::interaction::SubjectKind::Region)
        );
    }

    #[test]
    fn toggle_means_the_other_way_from_wherever_it_is() {
        let toggle = serde_json::json!({ "state": "toggle" });
        assert_eq!(
            requested_state(&toggle, DoorState::Open),
            Some(DoorState::Closed)
        );
        assert_eq!(
            requested_state(&toggle, DoorState::Closed),
            Some(DoorState::Open)
        );
    }

    #[test]
    fn an_explicit_state_ignores_where_the_door_is_now() {
        // A lever that says "open" opens it, and pulling it twice does not
        // close it. That is the difference between "open" and "toggle", and a
        // Game Master chose which one they wanted.
        let open = serde_json::json!({ "state": "open" });
        assert_eq!(
            requested_state(&open, DoorState::Open),
            Some(DoorState::Open)
        );
        assert_eq!(
            requested_state(&open, DoorState::Closed),
            Some(DoorState::Open)
        );
    }

    #[test]
    fn an_unrecognised_state_asks_for_nothing_rather_than_guessing() {
        assert_eq!(
            requested_state(&serde_json::json!({ "state": "ajar" }), DoorState::Closed),
            None
        );
        assert_eq!(
            requested_state(&serde_json::json!({}), DoorState::Closed),
            None
        );
    }

    #[test]
    fn a_target_and_a_lock_read_back() {
        let config = serde_json::json!({ "target": "w-1", "locked": true });
        assert_eq!(target_of(&config), Some("w-1"));
        assert_eq!(requested_lock(&config), Some(true));
        assert_eq!(requested_lock(&serde_json::json!({})), None);
    }
}

#[cfg(test)]
mod room_and_clear_tests {
    use super::*;

    fn wall(id: &str) -> Wall {
        Wall {
            id: id.to_string(),
            x1: 0.0,
            y1: 0.0,
            x2: 10.0,
            y2: 0.0,
            blocks_vision: true,
            blocks_movement: false,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        }
    }

    #[test]
    fn clearing_removes_every_wall() {
        let mut set = WallSet::default();
        set.upsert(wall("a"));
        set.upsert(wall("b"));
        set.clear();
        assert!(set.walls().is_empty());
        assert!(set.get("a").is_none());
    }

    #[test]
    fn clearing_discards_the_undo_history_too() {
        // An undo entry names a wall id from a scene nobody is looking at any
        // more. Replaying one would emit a mutation against the wrong scene,
        // which is worse than having no undo across a scene change.
        let mut set = WallSet::default();
        set.upsert(wall("a"));
        set.push_undo(WallEdit::Delete { deleted: wall("a") });
        set.clear();
        assert_eq!(set.undo_stack_len(), 0);
        assert!(set.pop_undo().is_none());
    }

    #[test]
    fn clearing_marks_the_set_dirty() {
        // Occlusion and illumination recompute off `dirty`. A cleared scene
        // that did not set it would keep casting the previous scene's shadows.
        let mut set = WallSet::default();
        set.upsert(wall("a"));
        set.dirty = false;
        set.clear();
        assert!(set.dirty);
    }

    #[test]
    fn a_room_is_four_segments_that_close() {
        let room = room_segments(Vec2::ZERO, Vec2::new(100.0, 50.0)).expect("non-degenerate");
        assert_eq!(room.len(), 4);
        // Each segment's end is the next one's start, and the last closes back
        // onto the first. This is the property the vision pass depends on: a
        // corner that misses by a fraction of a unit is a corner light escapes
        // through.
        for pair in 0..4 {
            assert_eq!(room[pair].1, room[(pair + 1) % 4].0, "segment {pair}");
        }
    }

    #[test]
    fn a_room_covers_exactly_the_dragged_rectangle() {
        let room = room_segments(Vec2::new(-20.0, 5.0), Vec2::new(30.0, 45.0)).expect("room");
        let xs: Vec<f32> = room.iter().flat_map(|(a, b)| [a.x, b.x]).collect();
        let ys: Vec<f32> = room.iter().flat_map(|(a, b)| [a.y, b.y]).collect();
        assert_eq!(xs.iter().cloned().fold(f32::MAX, f32::min), -20.0);
        assert_eq!(xs.iter().cloned().fold(f32::MIN, f32::max), 30.0);
        assert_eq!(ys.iter().cloned().fold(f32::MAX, f32::min), 5.0);
        assert_eq!(ys.iter().cloned().fold(f32::MIN, f32::max), 45.0);
    }

    #[test]
    fn the_drag_direction_does_not_change_the_room() {
        // Four ways to drag out the same rectangle. A Game Master should not
        // have to start from a particular corner.
        let reference = room_segments(Vec2::new(0.0, 0.0), Vec2::new(60.0, 40.0)).expect("room");
        for (a, b) in [
            (Vec2::new(60.0, 40.0), Vec2::new(0.0, 0.0)),
            (Vec2::new(0.0, 40.0), Vec2::new(60.0, 0.0)),
            (Vec2::new(60.0, 0.0), Vec2::new(0.0, 40.0)),
        ] {
            assert_eq!(room_segments(a, b).expect("room"), reference, "{a:?}->{b:?}");
        }
    }

    #[test]
    fn a_flat_drag_is_not_a_room() {
        // No interior means no room. Emitting two pairs of coincident walls
        // would leave four walls stacked on a line with nothing to show for it.
        assert!(room_segments(Vec2::ZERO, Vec2::new(100.0, 0.0)).is_none());
        assert!(room_segments(Vec2::ZERO, Vec2::new(0.0, 100.0)).is_none());
        assert!(room_segments(Vec2::ZERO, Vec2::ZERO).is_none());
    }

    #[test]
    fn a_non_finite_corner_is_not_a_room() {
        // A cursor position can arrive as NaN when the camera projection is
        // degenerate. Four walls at NaN would be invisible and unselectable.
        assert!(room_segments(Vec2::ZERO, Vec2::new(f32::NAN, 10.0)).is_none());
        assert!(room_segments(Vec2::new(f32::INFINITY, 0.0), Vec2::new(10.0, 10.0)).is_none());
    }

    #[test]
    fn room_walls_built_from_the_segments_enclose_their_interior() {
        // The point of drawing a room rather than four walls: a sightline from
        // inside to outside is blocked on every side.
        let mut set = WallSet::default();
        for (index, (a, b)) in room_segments(Vec2::new(-50.0, -50.0), Vec2::new(50.0, 50.0))
            .expect("room")
            .into_iter()
            .enumerate()
        {
            let mut segment = wall(&format!("r{index}"));
            segment.x1 = a.x;
            segment.y1 = a.y;
            segment.x2 = b.x;
            segment.y2 = b.y;
            set.upsert(segment);
        }

        let inside = Vec2::ZERO;
        for outside in [
            Vec2::new(0.0, 200.0),
            Vec2::new(0.0, -200.0),
            Vec2::new(200.0, 0.0),
            Vec2::new(-200.0, 0.0),
        ] {
            assert!(
                !is_visible(inside, outside, &set),
                "the room leaked toward {outside:?}",
            );
        }
    }
}
