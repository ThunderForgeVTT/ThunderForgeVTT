//! A world's system settings, through the real schema and a real Postgres.
//!
//! The declaration's own rules — types, bounds, options — are tested beside
//! it in `thunderforge_pack_system_spec::settings` with no database. What needs both is
//! everything a declaration cannot say about itself: who may read, who may
//! write, that a missing row is a default rather than an error, that a stale
//! row is ignored and kept, and that a change leaves a record.

use async_graphql::Request;
use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::schema::{world_system_setting_changes, world_system_settings, worlds};
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
    try_test_connection,
};

/// A system that exists only here, so the test states its own declarations
/// and no bundled pack is named in shared source.
const SYSTEM: &str = "settings_probe";

const READ: &str = r#"
    query Settings($worldId: UUID!) {
        worldSystemSettings(worldId: $worldId) {
            key label kind min max defaultValue value isDefault
            options { value label }
        }
    }
"#;

const WRITE: &str = r#"
    mutation Set($worldId: UUID!, $key: String!, $value: JSON!) {
        setWorldSystemSetting(worldId: $worldId, key: $key, value: $value) {
            key value isDefault
        }
    }
"#;

fn manifest() -> Value {
    json!({
        "id": SYSTEM,
        "settings": [
            { "id": "inspiration", "label": "Inspiration", "type": "boolean", "default": true },
            { "id": "rests", "label": "Rests per day", "type": "integer",
              "default": 1, "min": 0, "max": 3 },
            { "id": "mode", "label": "Difficulty", "type": "choice", "default": "free",
              "options": [{ "value": "free", "label": "Free" },
                          { "value": "target", "label": "Target" }] },
        ],
    })
}

struct Table {
    schema: crate::graphql::AppSchema,
    world_id: Uuid,
    gm: Uuid,
    /// Held so the directory outlives the schema that reads it.
    _systems: tempfile::TempDir,
}

/// A world on the probe system, whose manifest is `manifest`.
fn table(conn: &mut PgConnection, manifest: &Value) -> Table {
    let systems = tempfile::tempdir().expect("a temporary systems directory");
    let dir = systems.path().join(SYSTEM);
    std::fs::create_dir_all(&dir).expect("the system's directory");
    std::fs::write(dir.join("system.json"), manifest.to_string()).expect("the manifest");

    let gm = insert_test_user(conn);
    let world_id = insert_test_world(conn, gm);
    diesel::update(worlds::table.find(world_id))
        .set(worlds::game_system_id.eq(SYSTEM))
        .execute(conn)
        .expect("put the world on the probe system");

    let mut state = test_app_state();
    state.directories.systems_dir = systems.path().to_str().expect("utf-8 path").to_string();
    let schema = async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish();

    Table {
        schema,
        world_id,
        gm,
        _systems: systems,
    }
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

async fn read(table: &Table, who: Uuid) -> async_graphql::Response {
    table
        .schema
        .execute(
            Request::new(READ)
                .variables(async_graphql::Variables::from_json(
                    json!({ "worldId": table.world_id }),
                ))
                .data(as_user(who)),
        )
        .await
}

async fn set(table: &Table, who: Uuid, key: &str, value: Value) -> async_graphql::Response {
    table
        .schema
        .execute(
            Request::new(WRITE)
                .variables(async_graphql::Variables::from_json(json!({
                    "worldId": table.world_id, "key": key, "value": value,
                })))
                .data(as_user(who)),
        )
        .await
}

fn settings(response: async_graphql::Response) -> Vec<Value> {
    assert!(
        response.errors.is_empty(),
        "unexpected errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().expect("response data as JSON");
    data["worldSystemSettings"]
        .as_array()
        .expect("a list of settings")
        .clone()
}

fn by<'a>(settings: &'a [Value], key: &str) -> &'a Value {
    settings
        .iter()
        .find(|s| s["key"] == key)
        .unwrap_or_else(|| panic!("no setting {key} in {settings:?}"))
}

fn stored_rows(conn: &mut PgConnection, world_id: Uuid) -> i64 {
    world_system_settings::table
        .filter(world_system_settings::world_id.eq(world_id))
        .count()
        .get_result(conn)
        .expect("count rows")
}

/// The invariant the feature rests on: no row is a default, not an error.
#[tokio::test]
async fn a_world_that_has_set_nothing_reads_every_default_and_stores_nothing() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());

    let read = settings(read(&table, table.gm).await);

    assert_eq!(read.len(), 3);
    assert_eq!(by(&read, "inspiration")["value"], json!(true));
    assert_eq!(by(&read, "inspiration")["isDefault"], json!(true));
    assert_eq!(by(&read, "rests")["value"], json!(1));
    assert_eq!(by(&read, "rests")["max"], json!(3));
    assert_eq!(by(&read, "mode")["options"][1]["label"], json!("Target"));
    assert_eq!(stored_rows(&mut conn, table.world_id), 0);
}

#[tokio::test]
async fn a_game_master_changes_a_setting_and_a_player_reads_it() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, table.world_id, player, "Player");

    let written = set(&table, table.gm, "inspiration", json!(false)).await;
    assert!(written.errors.is_empty(), "{:?}", written.errors);

    let read = settings(read(&table, player).await);
    assert_eq!(by(&read, "inspiration")["value"], json!(false));
    assert_eq!(by(&read, "inspiration")["isDefault"], json!(false));
    // The others are untouched: one call changes one setting.
    assert_eq!(by(&read, "rests")["isDefault"], json!(true));
}

#[tokio::test]
async fn a_player_may_not_change_a_setting() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, table.world_id, player, "Player");

    let refused = set(&table, player, "inspiration", json!(false)).await;

    assert_eq!(refused.errors.len(), 1, "{:?}", refused.errors);
    assert!(refused.errors[0].message.contains("Game Master"));
    assert_eq!(stored_rows(&mut conn, table.world_id), 0);
}

#[tokio::test]
async fn somebody_outside_the_world_may_not_read_its_settings() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());
    let stranger = insert_test_user(&mut conn);

    let refused = read(&table, stranger).await;

    assert_eq!(refused.errors.len(), 1, "{:?}", refused.errors);
}

/// Refused, never coerced — and the refusal names the setting.
#[tokio::test]
async fn a_value_the_declaration_does_not_allow_is_refused_by_name() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());

    for (key, value, label) in [
        ("inspiration", json!("yes"), "Inspiration"),
        ("rests", json!(4), "Rests per day"),
        ("mode", json!("rolled"), "Difficulty"),
    ] {
        let refused = set(&table, table.gm, key, value).await;
        assert_eq!(refused.errors.len(), 1, "{key}: {:?}", refused.errors);
        assert!(
            refused.errors[0].message.contains(label),
            "{key}: {}",
            refused.errors[0].message
        );
    }
    let undeclared = set(&table, table.gm, "homebrew", json!(true)).await;
    assert_eq!(undeclared.errors.len(), 1);
    assert_eq!(stored_rows(&mut conn, table.world_id), 0);
}

#[tokio::test]
async fn every_change_is_recorded_with_what_it_replaced() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());

    assert!(
        set(&table, table.gm, "rests", json!(2))
            .await
            .errors
            .is_empty()
    );
    assert!(
        set(&table, table.gm, "rests", json!(3))
            .await
            .errors
            .is_empty()
    );

    let changes: Vec<(Option<Value>, Value, Option<Uuid>)> = world_system_setting_changes::table
        .filter(world_system_setting_changes::world_id.eq(table.world_id))
        .order(world_system_setting_changes::id.asc())
        .select((
            world_system_setting_changes::old_value,
            world_system_setting_changes::new_value,
            world_system_setting_changes::changed_by,
        ))
        .load(&mut conn)
        .expect("read the changes");

    assert_eq!(
        changes,
        vec![
            (None, json!(2), Some(table.gm)),
            (Some(json!(2)), json!(3), Some(table.gm)),
        ]
    );
    // Two changes to one setting are one row, not two.
    assert_eq!(stored_rows(&mut conn, table.world_id), 1);
}

/// A row the manifest no longer vouches for is ignored and kept: an
/// undeclared key, and a value the declaration has stopped allowing.
#[tokio::test]
async fn a_stale_row_reads_as_absent_and_is_left_in_place() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());
    for (key, value) in [("retired", json!(true)), ("rests", json!(99))] {
        diesel::insert_into(world_system_settings::table)
            .values((
                world_system_settings::world_id.eq(table.world_id),
                world_system_settings::system_id.eq(SYSTEM),
                world_system_settings::key.eq(key),
                world_system_settings::value.eq(value),
            ))
            .execute(&mut conn)
            .expect("a stale row");
    }

    let read = settings(read(&table, table.gm).await);

    assert_eq!(read.len(), 3, "an undeclared key is not a setting");
    assert_eq!(by(&read, "rests")["value"], json!(1));
    assert_eq!(by(&read, "rests")["isDefault"], json!(true));
    assert_eq!(stored_rows(&mut conn, table.world_id), 2);
}

/// A world's answers belong to the system they were given for.
#[tokio::test]
async fn a_world_that_changes_system_and_back_finds_its_answers() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());
    assert!(
        set(&table, table.gm, "mode", json!("target"))
            .await
            .errors
            .is_empty()
    );

    diesel::update(worlds::table.find(table.world_id))
        .set(worlds::game_system_id.eq("somewhere_else"))
        .execute(&mut conn)
        .expect("change system");
    assert!(settings(read(&table, table.gm).await).is_empty());

    diesel::update(worlds::table.find(table.world_id))
        .set(worlds::game_system_id.eq(SYSTEM))
        .execute(&mut conn)
        .expect("change back");
    let read = settings(read(&table, table.gm).await);
    assert_eq!(by(&read, "mode")["value"], json!("target"));
}

/// The function server code reads a setting through (FR-011).
#[tokio::test]
async fn server_code_reads_the_effective_value_of_one_setting() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let table = table(&mut conn, &manifest());
    let systems_dir = table
        ._systems
        .path()
        .to_str()
        .expect("utf-8 path")
        .to_string();
    let read = |conn: &mut PgConnection, key: &str| {
        crate::world_system_settings::effective_value(conn, &systems_dir, table.world_id, key)
            .expect("a read never fails on a missing row")
    };

    assert_eq!(read(&mut conn, "rests"), Some(json!(1)));
    assert!(
        set(&table, table.gm, "rests", json!(0))
            .await
            .errors
            .is_empty()
    );
    assert_eq!(read(&mut conn, "rests"), Some(json!(0)));
    assert_eq!(read(&mut conn, "homebrew"), None);
}
