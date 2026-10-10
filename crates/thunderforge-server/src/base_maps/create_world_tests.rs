//! Spec 088 (T036, T039): `createWorld`'s `baseMapId`, and the credit a
//! scene carries afterwards, through the real schema, a real Postgres and a
//! real RustFS (`docker compose up -d rustfs`).

use async_graphql::{Request, Variables};
use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use super::{BaseMap, BaseMaps, MapCredit};
use crate::auth_middleware::AuthenticatedUser;
use crate::map_import::{LightInsert, WallInsert};
use crate::schema::{canvas_image_assets, light_sources, scenes, walls, world_events, worlds};
use crate::test_support::{insert_test_user, test_app_state};
use crate::world_events::EVENT_CODE_MAP_IMPORTED;

const MAP: &str = "grassy-path-ambush";
const SIZE: u32 = 64;

const CREATE: &str = r#"
    mutation Create($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) { id activeSceneId }
    }
"#;

const SCENES: &str = r#"
    query Scenes($worldId: UUID!) {
        scenes(worldId: $worldId) {
            sceneId
            backgroundCredit { author licence licenceUrl source catalog shareAlike }
        }
    }
"#;

fn credit() -> MapCredit {
    MapCredit {
        author: "MBRound18".to_string(),
        licence: "CC BY-SA 4.0".to_string(),
        licence_url: "https://creativecommons.org/licenses/by-sa/4.0/".to_string(),
        source: "https://github.com/mbround18/vtt-maps".to_string(),
        catalog: "https://vtt-maps.dnd-apps.dev/catalog".to_string(),
    }
}

fn wall(door_state: &'static str) -> WallInsert {
    WallInsert {
        x1: 0.0,
        y1: 0.0,
        x2: f64::from(SIZE),
        y2: 0.0,
        blocks_vision: true,
        blocks_movement: true,
        door_state,
    }
}

/// One map, with a real image on disk, one wall, one door and one light.
fn maps(with_image: bool) -> (tempfile::TempDir, BaseMaps) {
    let dir = tempfile::tempdir().expect("a temp dir");
    if with_image {
        let pixels = vec![90u8; (SIZE * SIZE * 4) as usize];
        let webp = webp::Encoder::from_rgba(&pixels, SIZE, SIZE).encode_lossless();
        std::fs::write(dir.path().join(format!("{MAP}.webp")), &*webp).unwrap();
        std::fs::write(dir.path().join(format!("{MAP}.thumb.webp")), &*webp).unwrap();
    }
    let map = BaseMap {
        id: MAP.to_string(),
        name: "Grassy Path Ambush".to_string(),
        width: SIZE,
        height: SIZE,
        grid_size: 32,
        ambient_light: "dim".to_string(),
        walls: vec![wall("none"), wall("closed")],
        lights: vec![LightInsert {
            x: 1.0,
            y: 2.0,
            radius: 30.0,
            intensity: 1.0,
            color: "#ffaa00".to_string(),
            casts_shadows: true,
        }],
        has_thumbnail: with_image,
    };
    let maps = BaseMaps::for_tests(dir.path().to_path_buf(), vec![map], credit());
    (dir, maps)
}

fn as_user(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

struct Instance {
    schema: crate::graphql::AppSchema,
    state: crate::state::AppState,
    gm: Uuid,
    _dir: tempfile::TempDir,
}

fn instance(with_image: bool) -> Instance {
    let (dir, maps) = maps(with_image);
    let mut state = test_app_state();
    state.base_maps = std::sync::Arc::new(maps);
    let mut conn = state.db_pool.get().expect("conn");
    let gm = insert_test_user(&mut conn);
    drop(conn);
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state.clone())
    .finish();
    Instance {
        schema,
        state,
        gm,
        _dir: dir,
    }
}

async fn create(instance: &Instance, base_map_id: Option<Value>) -> async_graphql::Response {
    // A name of its own each time: one GM cannot own two worlds of one name.
    let mut input = json!({ "name": format!("A World {}", Uuid::now_v7()) });
    if let Some(id) = base_map_id {
        input["baseMapId"] = id;
    }
    instance
        .schema
        .execute(
            Request::new(CREATE)
                .variables(Variables::from_json(json!({ "input": input })))
                .data(as_user(instance.gm)),
        )
        .await
}

fn created(response: &async_graphql::Response) -> (Uuid, Uuid) {
    let data = response.data.clone().into_json().expect("json");
    let world = &data["createWorld"];
    let id = |field: &str| {
        let value = world[field].as_str();
        let value = value.unwrap_or_else(|| panic!("no {field}: {:?}", response.errors));
        Uuid::parse_str(value).expect("uuid")
    };
    (id("id"), id("activeSceneId"))
}

/// What the Starting Scene holds after `createWorld`.
struct Scene {
    base_map_id: Option<String>,
    walls: i64,
    doors: i64,
    lights: i64,
    imported_events: i64,
}

fn scene(instance: &Instance, world_id: Uuid, scene_id: Uuid) -> Scene {
    let mut conn = instance.state.db_pool.get().expect("conn");
    let background: Option<Uuid> = scenes::table
        .find(scene_id)
        .select(scenes::background_asset_id)
        .first(&mut conn)
        .expect("the scene");
    let base_map_id = background.and_then(|asset_id| {
        canvas_image_assets::table
            .filter(canvas_image_assets::asset_id.eq(asset_id))
            .select(canvas_image_assets::base_map_id)
            .first::<Option<String>>(&mut conn)
            .expect("the background's asset row")
    });
    let count_walls = |doors: bool, conn: &mut PgConnection| -> i64 {
        let query = walls::table
            .filter(walls::scene_id.eq(scene_id))
            .into_boxed();
        let query = if doors {
            query.filter(walls::door_state.ne("none"))
        } else {
            query.filter(walls::door_state.eq("none"))
        };
        query.count().get_result(conn).expect("walls")
    };
    Scene {
        base_map_id,
        walls: count_walls(false, &mut conn),
        doors: count_walls(true, &mut conn),
        lights: light_sources::table
            .filter(light_sources::scene_id.eq(scene_id))
            .count()
            .get_result(&mut conn)
            .expect("lights"),
        imported_events: world_events::table
            .filter(world_events::world_id.eq(world_id))
            .filter(world_events::event_code.eq(EVENT_CODE_MAP_IMPORTED))
            .count()
            .get_result(&mut conn)
            .expect("events"),
    }
}

fn no_errors(response: &async_graphql::Response) {
    assert!(response.errors.is_empty(), "{:?}", response.errors);
}

/// Left out, the world opens on the default map, with its walls, doors and
/// lights, and every client hears about it (FR-023, FR-024).
#[tokio::test]
async fn an_absent_base_map_opens_the_world_on_the_default() {
    let instance = instance(true);
    let response = create(&instance, None).await;
    no_errors(&response);
    let (world_id, scene_id) = created(&response);

    let scene = scene(&instance, world_id, scene_id);
    assert_eq!(scene.base_map_id.as_deref(), Some(MAP));
    assert_eq!((scene.walls, scene.doors, scene.lights), (1, 1, 1));
    assert_eq!(scene.imported_events, 1);
}

/// `null` is the GM choosing **None**: a blank Starting Scene.
#[tokio::test]
async fn a_null_base_map_leaves_the_scene_blank() {
    let instance = instance(true);
    let response = create(&instance, Some(Value::Null)).await;
    no_errors(&response);
    let (world_id, scene_id) = created(&response);

    let scene = scene(&instance, world_id, scene_id);
    assert_eq!(scene.base_map_id, None);
    assert_eq!((scene.walls, scene.doors, scene.lights), (0, 0, 0));
    assert_eq!(scene.imported_events, 0);
}

/// A map this instance does not have is refused before anything is made.
#[tokio::test]
async fn an_unknown_base_map_is_refused_and_creates_nothing() {
    let instance = instance(true);
    let response = create(&instance, Some(json!("../etc/passwd"))).await;
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    assert_eq!(response.errors[0].message, super::apply::UNKNOWN_MAP);

    let mut conn = instance.state.db_pool.get().expect("conn");
    let made: i64 = worlds::table
        .filter(worlds::created_by.eq(instance.gm))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(made, 0);
}

/// A map that cannot go on costs the GM their map, never their world
/// (FR-025): the world comes back with a `STARTING_MAP_FAILED` error beside it.
#[tokio::test]
async fn a_map_that_fails_to_apply_leaves_the_world_and_says_so() {
    let instance = instance(false);
    let response = create(&instance, Some(json!(MAP))).await;
    let (world_id, scene_id) = created(&response);
    assert_eq!(response.errors.len(), 1, "{:?}", response.errors);
    let code = response.errors[0]
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("code"))
        .cloned();
    assert_eq!(
        code,
        Some(async_graphql::Value::from(
            super::apply::STARTING_MAP_FAILED
        ))
    );

    let scene = scene(&instance, world_id, scene_id);
    assert_eq!(scene.base_map_id, None);
    assert_eq!(scene.walls + scene.doors + scene.lights, 0);
}

/// The rescue flow (spec 039) makes a world without a map, even on an
/// instance that has one (FR-026).
#[tokio::test]
async fn a_rescued_characters_world_opens_blank() {
    use crate::test_support::{insert_test_actor, insert_test_scene, insert_test_world};

    let instance = instance(true);
    let mut conn = instance.state.db_pool.get().expect("conn");
    let campaign = insert_test_world(&mut conn, instance.gm);
    let campaign_scene = insert_test_scene(&mut conn, campaign, instance.gm);
    let player = insert_test_user(&mut conn);
    insert_test_actor(&mut conn, campaign, campaign_scene, player);

    let moved = conn
        .transaction(|conn| {
            crate::collections::rescue::rescue_characters_sync(conn, instance.gm, &[campaign])
        })
        .expect("rescue");
    assert_eq!(moved, 1);

    let (home, home_scene): (Uuid, Option<Uuid>) = worlds::table
        .filter(worlds::created_by.eq(player))
        .select((worlds::id, worlds::active_scene_id))
        .first(&mut conn)
        .expect("the player's new world");
    drop(conn);
    let scene = scene(&instance, home, home_scene.expect("a Starting Scene"));
    assert_eq!(scene.base_map_id, None);
    assert_eq!(scene.imported_events, 0);
}

fn credits(response: async_graphql::Response) -> Vec<Value> {
    no_errors(&response);
    let data = response.data.into_json().expect("json");
    data["scenes"]
        .as_array()
        .expect("scenes")
        .iter()
        .map(|scene| scene["backgroundCredit"].clone())
        .collect()
}

async fn read_credits(instance: &Instance, world_id: Uuid) -> Vec<Value> {
    credits(
        instance
            .schema
            .execute(
                Request::new(SCENES)
                    .variables(Variables::from_json(json!({ "worldId": world_id })))
                    .data(as_user(instance.gm)),
            )
            .await,
    )
}

/// A scene on one of our maps carries its credit; any other scene, none
/// (FR-029).
#[tokio::test]
async fn a_scene_on_a_base_map_carries_its_credit_and_others_do_not() {
    let instance = instance(true);

    let on_map = created(&create(&instance, None).await).0;
    let credit = &read_credits(&instance, on_map).await[0];
    assert_eq!(credit["author"], "MBRound18");
    assert_eq!(credit["licence"], "CC BY-SA 4.0");
    assert_eq!(credit["source"], "https://github.com/mbround18/vtt-maps");
    assert_eq!(credit["catalog"], "https://vtt-maps.dnd-apps.dev/catalog");
    assert_eq!(credit["shareAlike"], super::SHARE_ALIKE);

    let blank = created(&create(&instance, Some(Value::Null)).await).0;
    assert_eq!(read_credits(&instance, blank).await, vec![Value::Null]);
}
