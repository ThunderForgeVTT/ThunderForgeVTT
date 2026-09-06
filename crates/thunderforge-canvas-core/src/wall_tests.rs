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
