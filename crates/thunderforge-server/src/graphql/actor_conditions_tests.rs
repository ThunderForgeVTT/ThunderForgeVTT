//! A character's conditions, through the real schema and a real Postgres.
//!
//! The manifest's own rules — glyphs, colours, unique ids — are tested beside
//! it in `thunderforge_pack_system_spec::conditions` with no database. What needs both is
//! everything a declaration cannot say about itself: who may apply, that an
//! undeclared condition is refused, that a stale row is ignored and kept, and
//! that a condition travels on the token to whoever is sent the token and to
//! nobody else.

use async_graphql::Request;
use diesel::prelude::*;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::schema::{tokens, world_actor_conditions, world_events, worlds};
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state, try_test_connection,
};
use crate::world_events::{EVENT_CODE_ACTOR_SHEET_CHANGED, EVENT_CODE_TOKEN_CHANGED};

/// A system that exists only here, so the test states its own declarations
/// and no bundled pack is named in shared source.
const SYSTEM: &str = "conditions_probe";

const DECLARED: &str = r#"
    query Declared($worldId: UUID!) {
        worldSystemConditions(worldId: $worldId) { id label description glyph color }
    }
"#;

const APPLY: &str = r#"
    mutation Apply($actorId: UUID!, $conditionId: String!) {
        applyActorCondition(actorId: $actorId, conditionId: $conditionId) { id glyph color }
    }
"#;

const CLEAR: &str = r#"
    mutation Clear($actorId: UUID!, $conditionId: String!) {
        clearActorCondition(actorId: $actorId, conditionId: $conditionId) { id glyph color }
    }
"#;

const BOARD: &str = r#"
    query Board($sceneId: UUID!) {
        tokens(sceneId: $sceneId) { tokenId conditions { id glyph color } }
    }
"#;

fn manifest() -> Value {
    json!({
        "id": SYSTEM,
        "conditions": [
            { "id": "prone", "label": "Prone", "description": "On the ground.",
              "marker": { "glyph": "bar", "color": "warning" } },
            { "id": "stunned", "label": "Stunned",
              "marker": { "glyph": "cross", "color": "danger" } },
        ],
    })
}

struct Table {
    schema: crate::graphql::AppSchema,
    world_id: Uuid,
    scene_id: Uuid,
    gm: Uuid,
    player: Uuid,
    /// A creature with a token on the scene.
    ogre: Uuid,
    ogre_token: Uuid,
    /// A token that stands for no actor.
    crate_token: Uuid,
    /// Held so the directory outlives the schema that reads it.
    _systems: tempfile::TempDir,
}

fn insert_token(conn: &mut PgConnection, scene_id: Uuid, actor_id: Option<Uuid>) -> Uuid {
    let token_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(token_id),
            tokens::scene_id.eq(scene_id),
            tokens::actor_id.eq(actor_id),
            tokens::x.eq(0.0_f64),
            tokens::y.eq(0.0_f64),
            tokens::created_at.eq(now),
            tokens::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test token");
    token_id
}

/// A world on the probe system, with a Game Master, a player, and an ogre
/// standing on its one scene.
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
    let player = insert_test_user(conn);
    insert_test_world_member(conn, world_id, player, "Player");

    let scene_id = insert_test_scene(conn, world_id, gm);
    let ogre = insert_test_actor(conn, world_id, scene_id, gm);
    let ogre_token = insert_token(conn, scene_id, Some(ogre));
    let crate_token = insert_token(conn, scene_id, None);

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
        scene_id,
        gm,
        player,
        ogre,
        ogre_token,
        crate_token,
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

async fn ask(
    table: &Table,
    who: Uuid,
    document: &str,
    variables: Value,
) -> async_graphql::Response {
    table
        .schema
        .execute(
            Request::new(document)
                .variables(async_graphql::Variables::from_json(variables))
                .data(as_user(who)),
        )
        .await
}

async fn change(
    table: &Table,
    who: Uuid,
    document: &str,
    condition: &str,
) -> async_graphql::Response {
    ask(
        table,
        who,
        document,
        json!({ "actorId": table.ogre, "conditionId": condition }),
    )
    .await
}

fn data(response: async_graphql::Response) -> Value {
    assert!(
        response.errors.is_empty(),
        "unexpected errors: {:?}",
        response.errors
    );
    response.data.into_json().expect("response data as JSON")
}

fn refusal(response: async_graphql::Response) -> String {
    response
        .errors
        .first()
        .map(|error| error.message.clone())
        .unwrap_or_else(|| panic!("expected a refusal, got {:?}", response.data))
}

/// The conditions `who` is sent on one token of the scene.
async fn on_token(table: &Table, who: Uuid, token_id: Uuid) -> Value {
    let board = data(ask(table, who, BOARD, json!({ "sceneId": table.scene_id })).await);
    board["tokens"]
        .as_array()
        .expect("a list of tokens")
        .iter()
        .find(|token| token["tokenId"] == json!(token_id))
        .unwrap_or_else(|| panic!("token {token_id} not on the board sent: {board}"))["conditions"]
        .clone()
}

fn stored(conn: &mut PgConnection, actor_id: Uuid) -> Vec<String> {
    world_actor_conditions::table
        .filter(world_actor_conditions::actor_id.eq(actor_id))
        .select(world_actor_conditions::condition_id)
        .order(world_actor_conditions::condition_id.asc())
        .load(conn)
        .expect("the actor's stored conditions")
}

fn announcements(conn: &mut PgConnection, world_id: Uuid, code: i32) -> Vec<Value> {
    world_events::table
        .filter(world_events::world_id.eq(world_id))
        .filter(world_events::event_code.eq(code))
        .select(world_events::token_event)
        .load::<Option<Value>>(conn)
        .expect("the world's events")
        .into_iter()
        .flatten()
        .collect()
}

#[tokio::test]
async fn any_member_reads_what_the_system_declares_in_its_own_order() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());

    let declared = data(ask(&t, t.player, DECLARED, json!({ "worldId": t.world_id })).await);
    assert_eq!(
        declared["worldSystemConditions"],
        json!([
            { "id": "prone", "label": "Prone", "description": "On the ground.",
              "glyph": "bar", "color": "warning" },
            { "id": "stunned", "label": "Stunned", "description": null,
              "glyph": "cross", "color": "danger" },
        ])
    );

    let outsider = insert_test_user(&mut conn);
    let refused = refusal(ask(&t, outsider, DECLARED, json!({ "worldId": t.world_id })).await);
    assert!(refused.contains("Not a member"), "{refused}");
}

#[tokio::test]
async fn an_applied_condition_rides_on_the_token_to_everyone_sent_the_token() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());

    // Applied out of manifest order, and one of them twice.
    data(change(&t, t.gm, APPLY, "stunned").await);
    data(change(&t, t.gm, APPLY, "stunned").await);
    let answered = data(change(&t, t.gm, APPLY, "prone").await);

    let both = json!([
        { "id": "prone", "glyph": "bar", "color": "warning" },
        { "id": "stunned", "glyph": "cross", "color": "danger" },
    ]);
    assert_eq!(answered["applyActorCondition"], both);
    assert_eq!(on_token(&t, t.gm, t.ogre_token).await, both);
    assert_eq!(on_token(&t, t.player, t.ogre_token).await, both);
    // A token that stands for no actor carries none.
    assert_eq!(on_token(&t, t.gm, t.crate_token).await, json!([]));

    // Someone outside the world is sent neither the token nor what is on it.
    let outsider = insert_test_user(&mut conn);
    let response = ask(&t, outsider, BOARD, json!({ "sceneId": t.scene_id })).await;
    assert!(
        !format!("{:?} {:?}", response.data, response.errors).contains("prone"),
        "an outsider was told of a condition"
    );
    assert!(!response.errors.is_empty(), "an outsider read the board");
}

#[tokio::test]
async fn clearing_lifts_one_condition_and_leaves_the_rest() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());
    data(change(&t, t.gm, APPLY, "prone").await);
    data(change(&t, t.gm, APPLY, "stunned").await);

    let answered = data(change(&t, t.gm, CLEAR, "prone").await);
    let stunned = json!([{ "id": "stunned", "glyph": "cross", "color": "danger" }]);
    assert_eq!(answered["clearActorCondition"], stunned);
    assert_eq!(on_token(&t, t.player, t.ogre_token).await, stunned);

    // Clearing what is not there is not an error.
    let again = data(change(&t, t.gm, CLEAR, "prone").await);
    assert_eq!(again["clearActorCondition"], stunned);
}

#[tokio::test]
async fn a_condition_the_system_does_not_declare_is_refused() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());

    let refused = refusal(change(&t, t.gm, APPLY, "petrified").await);
    assert!(refused.contains("no condition \"petrified\""), "{refused}");
    assert!(stored(&mut conn, t.ogre).is_empty());
}

#[tokio::test]
async fn only_a_game_master_changes_a_condition() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());

    // Not found, as for an actor that does not exist: a refusal naming the
    // creature would tell a player it is there.
    assert_eq!(
        refusal(change(&t, t.player, APPLY, "prone").await),
        "Actor not found"
    );
    assert!(stored(&mut conn, t.ogre).is_empty());

    data(change(&t, t.gm, APPLY, "prone").await);
    assert_eq!(
        refusal(change(&t, t.player, CLEAR, "prone").await),
        "Actor not found"
    );
    assert_eq!(stored(&mut conn, t.ogre), vec!["prone".to_string()]);
}

#[tokio::test]
async fn a_stored_condition_nothing_declares_is_not_sent_and_not_deleted() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());
    data(change(&t, t.gm, APPLY, "prone").await);
    // As an older manifest would have left it.
    crate::actor_conditions::apply(&mut conn, t.ogre, "petrified", t.gm).expect("a stale row");

    let prone = json!([{ "id": "prone", "glyph": "bar", "color": "warning" }]);
    assert_eq!(on_token(&t, t.gm, t.ogre_token).await, prone);
    assert_eq!(
        stored(&mut conn, t.ogre),
        vec!["petrified".to_string(), "prone".to_string()]
    );

    // The Game Master can still clear it by name.
    data(change(&t, t.gm, CLEAR, "petrified").await);
    assert_eq!(stored(&mut conn, t.ogre), vec!["prone".to_string()]);
}

#[tokio::test]
async fn a_change_tells_the_sheet_and_the_board_with_ids_only() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let t = table(&mut conn, &manifest());
    // A second token of the ogre's on the same scene: one announcement a
    // scene, not one a token.
    insert_token(&mut conn, t.scene_id, Some(t.ogre));

    data(change(&t, t.gm, APPLY, "prone").await);

    let sheet = announcements(&mut conn, t.world_id, EVENT_CODE_ACTOR_SHEET_CHANGED);
    assert_eq!(
        sheet,
        vec![json!({ "action": "changed", "actorId": t.ogre, "dataType": "conditions" })]
    );
    let board = announcements(&mut conn, t.world_id, EVENT_CODE_TOKEN_CHANGED);
    assert_eq!(board.len(), 1, "{board:?}");
    assert_eq!(board[0]["scene_id"], json!(t.scene_id));
    assert!(
        !format!("{sheet:?}{board:?}").contains("prone"),
        "an announcement named the condition"
    );
}
