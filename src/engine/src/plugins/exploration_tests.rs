use super::*;
use crate::TokenIdentity;
use thunderforge_canvas_core::wall::{DoorState, Wall};

const CELL: f32 = 32.0;

fn app_with(enabled: bool, walls: Vec<Wall>, viewer_at: Vec2) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(SceneGrid::from_server("square", CELL, Vec2::ZERO));
    app.insert_resource(ExplorationEnabled(enabled));
    app.init_resource::<ExploredCells>();

    let mut wall_set = WallSet::default();
    for wall in walls {
        wall_set.upsert(wall);
    }
    app.insert_resource(wall_set);
    app.insert_resource(ViewerToken(Some("aria".to_string())));

    app.world_mut().spawn((
        Transform::from_translation(viewer_at.extend(0.0)),
        TokenIdentity("aria".to_string()),
    ));
    app.add_systems(Update, accumulate_explored);
    app
}

/// A wall across the board at x = 200, which blocks sight.
fn blocking_wall() -> Wall {
    Wall {
        id: "w-1".to_string(),
        x1: 200.0,
        y1: -2000.0,
        x2: 200.0,
        y2: 2000.0,
        blocks_vision: true,
        blocks_movement: true,
        door_state: DoorState::None,
        locked: false,
        secret: false,
    }
}

fn remembered(app: &App) -> usize {
    app.world().resource::<ExploredCells>().0.len()
}

fn remembers(app: &App, q: i32, r: i32) -> bool {
    app.world().resource::<ExploredCells>().0.contains(&(q, r))
}

#[test]
fn a_scene_with_exploration_off_remembers_nothing() {
    // FR-070. Off for every scene until a Game Master turns it on, and a
    // scene that was never asked to remember must not quietly accumulate.
    let mut app = app_with(false, vec![], Vec2::ZERO);
    app.update();
    assert_eq!(remembered(&app), 0);
}

#[test]
fn a_token_remembers_where_it_stands() {
    let mut app = app_with(true, vec![], Vec2::ZERO);
    app.update();
    assert!(remembers(&app, 0, 0), "the cell under the token");
    assert!(remembered(&app) > 1, "and the ones around it");
}

#[test]
fn a_wall_stops_the_memory_as_it_stops_the_sight() {
    // The whole point: fog must match what the player can actually see. A
    // second notion of visibility would drift from the lighting pass, and a
    // player would see remembered ground through a wall.
    let mut app = app_with(true, vec![blocking_wall()], Vec2::ZERO);
    app.update();

    assert!(remembers(&app, 0, 0), "here");
    assert!(remembers(&app, 3, 0), "this side of the wall");
    assert!(
        !remembers(&app, 12, 0),
        "and nothing beyond it — x = 384 is past the wall at 200"
    );
}

#[test]
fn what_was_seen_stays_seen_after_the_token_moves_away() {
    // Memory, not sight. Without this the fog would simply be a second
    // drawing of the lighting pass and remember nothing at all.
    let mut app = app_with(true, vec![], Vec2::new(-400.0, 0.0));
    app.update();
    let far_away = (-12, 0);
    assert!(
        app.world()
            .resource::<ExploredCells>()
            .0
            .contains(&far_away),
        "seen from where the token started"
    );

    // Walk east, well out of sight of where it began.
    let mut query = app.world_mut().query::<&mut Transform>();
    for mut transform in query.iter_mut(app.world_mut()) {
        transform.translation.x = 1600.0;
    }
    app.update();

    assert!(
        app.world()
            .resource::<ExploredCells>()
            .0
            .contains(&far_away),
        "and still remembered from a thousand units away"
    );
}

#[test]
fn a_game_master_explores_nothing() {
    // The fog is a player's own memory. A Game Master sees through no token,
    // and accumulating for them would mean the table shared one map.
    let mut app = app_with(true, vec![], Vec2::ZERO);
    app.insert_resource(ViewerToken(None));
    app.update();
    assert_eq!(remembered(&app), 0);
}

#[test]
fn a_memory_that_reaches_its_bound_stops_growing_rather_than_filling_a_browser() {
    // Data-model.md: a record past a size bound is dropped rather than grown.
    let mut app = app_with(true, vec![], Vec2::ZERO);
    {
        let mut explored = app.world_mut().resource_mut::<ExploredCells>();
        for i in 0..MAX_CELLS as i32 {
            explored.0.insert((i, 10_000));
        }
        assert!(explored.is_full());
    }
    app.update();
    assert_eq!(
        remembered(&app),
        MAX_CELLS,
        "a full memory accumulates nothing further"
    );
    assert!(!remembers(&app, 0, 0), "not even the cell underfoot");
}

#[test]
fn clearing_forgets_everything() {
    let mut app = app_with(true, vec![], Vec2::ZERO);
    app.update();
    assert!(remembered(&app) > 0);

    clear_explored(&mut app.world_mut().resource_mut::<ExploredCells>());
    assert_eq!(remembered(&app), 0);
}

// ---------------------------------------------------------------------------
// What the browser hands back, and what it reads
// ---------------------------------------------------------------------------

/// An app that only reconciles — it does not accumulate.
///
/// Needed because accumulation and reconciliation both act on the same
/// resource in the same frame, and a token standing anywhere re-fills its
/// surroundings the instant a reset clears them. That is correct on a board
/// and makes the reset impossible to observe in a test that also accumulates.
fn app_for_reconcile() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.init_resource::<ExploredCells>();
    app.insert_resource(ExplorationEnabled(true));
    app.add_systems(Update, reconcile_exploration);
    app
}

#[test]
fn a_memory_handed_back_from_the_browser_is_taken_up() {
    // The whole point of persisting: a player returning to a scene sees what
    // they saw last session, without walking it again.
    let mut app = app_for_reconcile();
    assert!(set_explored_cells("[[7,7],[8,8]]"));
    app.update();

    assert!(remembers(&app, 7, 7));
    assert!(remembers(&app, 8, 8));
}

#[test]
fn an_empty_string_is_a_reset_and_not_a_silence() {
    // "The Game Master cleared your map" and "the application has not told me
    // anything yet" must not look the same, or a reset would be lost in any
    // frame where nothing else happened.
    let mut app = app_for_reconcile();
    assert!(set_explored_cells("[[1,1],[2,2]]"));
    app.update();
    assert!(remembered(&app) >= 2);

    assert!(set_explored_cells(""));
    app.update();
    assert_eq!(remembered(&app), 0, "the reset reached the board");
}

#[test]
fn a_malformed_memory_is_refused_rather_than_forgetting_what_is_there() {
    // Broken storage must not read as "you have explored nothing": a player
    // whose browser returned nonsense should keep their map, not lose it.
    let mut app = app_for_reconcile();
    assert!(set_explored_cells("[[3,3]]"));
    app.update();

    assert!(!set_explored_cells("{not cells}"), "refused");
    app.update();
    assert!(remembers(&app, 3, 3), "and nothing was forgotten");
}

#[test]
fn the_mirror_reports_what_is_remembered_in_a_stable_order() {
    // The web compares this string to decide whether to write to storage. An
    // unstable order would make every frame look like a change and write
    // constantly.
    let mut app = app_for_reconcile();
    assert!(set_explored_cells("[[9,9],[1,1],[5,5]]"));
    app.update();

    let first = explored_cells();
    app.update();
    assert_eq!(first, explored_cells(), "two reads agree");
    assert!(first.starts_with("[[1,1]"), "sorted: {first}");
}

#[test]
fn after_a_reset_a_player_still_sees_where_they_are_standing() {
    // What a reset actually looks like at the table, and the reason the tests
    // above reconcile without accumulating. The distant map is gone; the room
    // the player is in comes straight back, because they are looking at it.
    let mut app = app_with(true, vec![], Vec2::ZERO);
    app.add_systems(Update, reconcile_exploration);
    assert!(set_explored_cells("[[500,500]]"));
    app.update();
    assert!(
        remembers(&app, 500, 500),
        "somewhere they went last session"
    );

    assert!(set_explored_cells(""));
    app.update();

    assert!(!remembers(&app, 500, 500), "the far room is forgotten");
    assert!(
        remembers(&app, 0, 0),
        "and the one they are standing in is remembered again at once"
    );
}
