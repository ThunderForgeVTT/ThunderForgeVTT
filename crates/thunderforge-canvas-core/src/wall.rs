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
}
