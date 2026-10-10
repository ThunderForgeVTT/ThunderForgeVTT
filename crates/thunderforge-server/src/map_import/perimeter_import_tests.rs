//! Spec 088 FR-091: an import walls the map's edges, and a re-import
//! replaces those walls rather than adding a second set.

use super::perimeter::{is_marked, lies_on_bounds};
use super::tests::read_fixture;
use super::*;

const AMBUSH: &str = "grassy-path-ambush.dd2vtt";

struct Imported {
    state: AppState,
    owner_id: Uuid,
    scene_id: Uuid,
}

fn scene() -> Imported {
    use crate::test_support::*;
    crate::test_support::load_dotenv();
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    drop(conn);
    Imported {
        state,
        owner_id,
        scene_id,
    }
}

async fn import(scene: &Imported, fixture: &str, wall_edges: bool) -> ImportResult {
    import_uvtt_impl(
        &scene.state,
        scene.owner_id,
        false,
        scene.scene_id,
        read_fixture(fixture),
        wall_edges,
    )
    .await
    .unwrap_or_else(|e| panic!("{fixture} should import: {e}"))
}

/// Every wall on the scene, as an insert row, its mark read back from
/// `metadata`.
fn walls(scene: &Imported) -> Vec<(Uuid, WallInsert)> {
    use crate::schema::walls;
    let mut conn = scene.state.db_pool.get().unwrap();
    walls::table
        .filter(walls::scene_id.eq(scene.scene_id))
        .select((
            walls::wall_id,
            walls::x1,
            walls::y1,
            walls::x2,
            walls::y2,
            walls::door_state,
            walls::metadata,
        ))
        .load::<(Uuid, f64, f64, f64, f64, String, Option<serde_json::Value>)>(&mut conn)
        .expect("walls reload")
        .into_iter()
        .map(|(id, x1, y1, x2, y2, door, metadata)| {
            (
                id,
                WallInsert {
                    x1,
                    y1,
                    x2,
                    y2,
                    blocks_vision: true,
                    blocks_movement: true,
                    door_state: if door == "none" { "none" } else { "door" },
                    perimeter: is_marked(metadata.as_ref()),
                },
            )
        })
        .collect()
}

fn size(scene: &Imported) -> (f64, f64) {
    use crate::schema::scenes;
    let mut conn = scene.state.db_pool.get().unwrap();
    let (w, h) = scenes::table
        .filter(scenes::scene_id.eq(scene.scene_id))
        .select((scenes::width, scenes::height))
        .first::<(i32, i32)>(&mut conn)
        .expect("scene reloads");
    (f64::from(w), f64::from(h))
}

fn marked(scene: &Imported) -> Vec<WallInsert> {
    walls(scene)
        .into_iter()
        .map(|(_, wall)| wall)
        .filter(|wall| wall.perimeter)
        .collect()
}

#[tokio::test]
async fn the_ambush_map_arrives_walled_at_its_four_edges() {
    let scene = scene();
    let result = import(&scene, AMBUSH, true).await;
    assert_eq!(result.walls_created, 4, "the map has no walls of its own");
    assert_eq!(result.perimeter_walls_created, 4);

    let (w, h) = size(&scene);
    let edges = marked(&scene);
    assert_eq!(edges.len(), 4);
    assert!(edges.iter().all(|wall| lies_on_bounds(wall, w, h)));
}

#[tokio::test]
async fn a_re_import_leaves_four_edge_walls_not_eight() {
    let scene = scene();
    import(&scene, AMBUSH, true).await;
    let again = import(&scene, AMBUSH, true).await;
    assert_eq!(again.perimeter_walls_created, 4);
    assert_eq!(marked(&scene).len(), 4);
    assert_eq!(walls(&scene).len(), 4);
}

#[tokio::test]
async fn unticked_the_import_adds_no_edge_walls() {
    let scene = scene();
    let result = import(&scene, AMBUSH, false).await;
    assert_eq!(
        (result.walls_created, result.perimeter_walls_created),
        (0, 0)
    );
    assert!(walls(&scene).is_empty());
}

#[tokio::test]
async fn an_edge_wall_the_gm_moved_survives_a_re_import() {
    use crate::schema::walls;
    let scene = scene();
    import(&scene, AMBUSH, true).await;
    let (id, wall) = walls(&scene).into_iter().next().expect("an edge wall");
    let mut conn = scene.state.db_pool.get().unwrap();
    // Pulled 40 px in from wherever it was.
    diesel::update(walls::table.filter(walls::wall_id.eq(id)))
        .set((
            walls::x1.eq(wall.x1 * 0.9),
            walls::y1.eq(wall.y1 * 0.9),
            walls::x2.eq(wall.x2 * 0.9),
            walls::y2.eq(wall.y2 * 0.9),
        ))
        .execute(&mut conn)
        .expect("move the wall");
    drop(conn);

    import(&scene, AMBUSH, true).await;
    let ids: Vec<Uuid> = walls(&scene).into_iter().map(|(id, _)| id).collect();
    assert!(ids.contains(&id), "the moved wall is still there");
    assert_eq!(ids.len(), 5, "the moved one, and four on the edges again");
}

#[tokio::test]
async fn edge_walls_move_none_of_the_files_own() {
    // A walled room whose four walls are its edges: the perimeter adds
    // nothing, and the file's walls are what they were without it.
    let with = scene();
    let without = scene();
    let a = import(&with, "chamber-of-echoing-grief.dd2vtt", true).await;
    let b = import(&without, "chamber-of-echoing-grief.dd2vtt", false).await;
    assert_eq!(a.perimeter_walls_created, 0);
    assert_eq!(a.walls_created, b.walls_created);

    // A map with doors and inner walls: every wall it had is still there,
    // unmoved, beside the new edge walls.
    let with = scene();
    let without = scene();
    import(&with, "dwarven-forge.dd2vtt", true).await;
    import(&without, "dwarven-forge.dd2vtt", false).await;
    let ends = |walls: Vec<(Uuid, WallInsert)>| {
        let mut all: Vec<[i64; 4]> = walls
            .into_iter()
            .filter(|(_, wall)| !wall.perimeter)
            .map(|(_, w)| [w.x1, w.y1, w.x2, w.y2].map(|v| (v * 100.0).round() as i64))
            .collect();
        all.sort();
        all
    };
    assert_eq!(ends(walls(&with)), ends(walls(&without)));
}

#[tokio::test]
async fn the_import_event_counts_its_edge_walls() {
    use crate::schema::world_events;
    let scene = scene();
    import(&scene, AMBUSH, true).await;
    let mut conn = scene.state.db_pool.get().unwrap();
    let payload: Option<serde_json::Value> = world_events::table
        .filter(world_events::event_code.eq(crate::world_events::EVENT_CODE_MAP_IMPORTED))
        .filter(world_events::token_event.is_not_null())
        .order(world_events::id.desc())
        .select(world_events::token_event)
        .load::<Option<serde_json::Value>>(&mut conn)
        .expect("events load")
        .into_iter()
        .flatten()
        .find(|payload| payload["scene_id"] == serde_json::json!(scene.scene_id));
    let payload = payload.expect("the import's event");
    assert_eq!(payload["perimeter_walls_created"], 4);
    assert_eq!(payload["walls_created"], 4);
}

#[test]
fn the_form_field_turns_the_edges_off_only_when_it_says_so() {
    for off in ["false", "0", "off", " FALSE "] {
        assert!(!wall_edges_from(off), "{off:?} turns it off");
    }
    for on in ["true", "1", "on", ""] {
        assert!(wall_edges_from(on), "{on:?} leaves it on");
    }
}
