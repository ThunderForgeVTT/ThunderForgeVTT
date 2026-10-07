//! Shape mutations: who may write which shape (spec 082), and the older
//! world-role and visibility tests moved here from `mutations_shapes.rs`.

use super::*;
use diesel::PgConnection;

/// Establishes a connection to the test database (see
/// `test_support::test_database_url`). Skips (rather than fails)
/// when no dev database is reachable, since this is a real-DB
/// integration test, not a unit test.
fn try_connect() -> Option<PgConnection> {
    crate::test_support::try_test_connection()
}

/// The rule every shape mutation (`create_shape`/`update_shape`/`delete_shape`) now asks, in the one place they all ask
/// it: authority to author content on a scene is the caller's **world
/// role** — Owner or GM — not who happened to create the scene.
///
/// This replaces `shape_mutations_are_scoped_to_scene_owner`, which asserted the old rule faithfully.
/// That rule was the bug: two people both holding GM authority in one
/// world, writing to one scene, had exactly half the writes refused,
/// because whichever of them had not created the scene was refused every
/// time. Both directions of that break are asserted below, along with the
/// two answers that must stay refusals — a GM's new authority must not
/// leak down to Players or out to non-members.
#[test]
fn shape_authority_follows_the_world_role_not_the_scene_creator() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping shape_authority_follows_the_world_role_not_the_scene_creator: no test database reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        use crate::auth::world_membership::is_dm_of_scene;
        use crate::test_support::{
            insert_test_scene_named, insert_test_user, insert_test_world, insert_test_world_member,
        };

        let owner_id = insert_test_user(conn);
        let world_id = insert_test_world(conn, owner_id);

        let gm_id = insert_test_user(conn);
        insert_test_world_member(conn, world_id, gm_id, "GM");
        let player_id = insert_test_user(conn);
        insert_test_world_member(conn, world_id, player_id, "Player");
        let stranger_id = insert_test_user(conn);

        // Two scenes in the same world, created by two different people.
        // Under the old rule each of them was an island.
        let owners_scene = insert_test_scene_named(conn, world_id, owner_id, "Owner's Scene");
        let gms_scene = insert_test_scene_named(conn, world_id, gm_id, "GM's Scene");

        assert!(
            is_dm_of_scene(conn, gm_id, false, owners_scene)?,
            "a member promoted to GM must be able to edit shapes on a scene the Owner created"
        );
        assert!(
            is_dm_of_scene(conn, owner_id, false, gms_scene)?,
            "the world's Owner must be able to edit shapes on a scene a GM created"
        );
        assert!(
            !is_dm_of_scene(conn, player_id, false, owners_scene)?,
            "a plain Player must not gain content authority from world membership"
        );
        assert!(
            !is_dm_of_scene(conn, stranger_id, false, owners_scene)?,
            "a non-member must not be able to edit shapes in this world at all"
        );

        Ok(())
    });
}

#[test]
fn shape_kind_round_trips_through_db_string_representation() {
    for kind in [
        GraphQLShapeKind::Stroke,
        GraphQLShapeKind::Rect,
        GraphQLShapeKind::Ellipse,
        GraphQLShapeKind::Line,
        GraphQLShapeKind::Text,
    ] {
        assert_eq!(GraphQLShapeKind::from_db_str(kind.as_db_str()), kind);
    }

    // Unknown stored values fall back to "stroke" rather than panicking.
    assert_eq!(
        GraphQLShapeKind::from_db_str("garbage"),
        GraphQLShapeKind::Stroke
    );
}

/// Verifies the `shapes` query's player-visibility filter (FR-009):
/// a non-owner caller must only see `visible_to_players = true`
/// shapes, while the scene owner sees everything regardless of the
/// flag. Runs entirely inside a `test_transaction`, which Diesel
/// always rolls back.
#[test]
fn shapes_query_hides_gm_only_shapes_from_non_owners() {
    let Some(mut conn) = try_connect() else {
        eprintln!(
            "skipping shapes_query_hides_gm_only_shapes_from_non_owners: no test database reachable"
        );
        return;
    };

    conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
        use crate::schema::{scenes, shapes, users, worlds};

        let owner_id = uuid::Uuid::now_v7();
        let intruder_id = uuid::Uuid::now_v7();
        let world_id = uuid::Uuid::now_v7();
        let scene_id = uuid::Uuid::now_v7();
        let gm_only_shape_id = uuid::Uuid::now_v7();
        let player_visible_shape_id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();

        for (id, username) in [
            (owner_id, "shape-query-owner"),
            (intruder_id, "shape-query-intruder"),
        ] {
            diesel::insert_into(users::table)
                .values((
                    users::id.eq(id),
                    users::username.eq(format!("{username}-{id}")),
                    users::password_hash.eq("test-hash"),
                    users::email.eq(format!("{username}-{id}@example.test")),
                    users::created_at.eq(now),
                    users::updated_at.eq(now),
                ))
                .execute(conn)?;
        }

        diesel::insert_into(worlds::table)
            .values((
                worlds::id.eq(world_id),
                worlds::name.eq("Shape Query Test World"),
                worlds::created_by.eq(owner_id),
                worlds::updated_by.eq(owner_id),
                worlds::created_at.eq(now),
                worlds::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(scenes::table)
            .values((
                scenes::scene_id.eq(scene_id),
                scenes::world_id.eq(world_id),
                scenes::name.eq("Shape Query Test Scene"),
                scenes::type_.eq("battlemap"),
                scenes::grid_size.eq(32),
                scenes::grid_type.eq("square"),
                scenes::width.eq(1000),
                scenes::height.eq(1000),
                scenes::owner_id.eq(owner_id),
                scenes::created_at.eq(now),
                scenes::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(shapes::table)
            .values((
                shapes::shape_id.eq(gm_only_shape_id),
                shapes::scene_id.eq(scene_id),
                shapes::kind.eq("rect"),
                shapes::geometry.eq(serde_json::json!({"x": 0, "y": 0, "w": 10, "h": 10})),
                shapes::visible_to_players.eq(false),
                shapes::created_by.eq(owner_id),
                shapes::updated_by.eq(owner_id),
                shapes::created_at.eq(now),
                shapes::updated_at.eq(now),
            ))
            .execute(conn)?;

        diesel::insert_into(shapes::table)
            .values((
                shapes::shape_id.eq(player_visible_shape_id),
                shapes::scene_id.eq(scene_id),
                shapes::kind.eq("ellipse"),
                shapes::geometry.eq(serde_json::json!({"x": 5, "y": 5, "rx": 3, "ry": 3})),
                shapes::visible_to_players.eq(true),
                shapes::created_by.eq(owner_id),
                shapes::updated_by.eq(owner_id),
                shapes::created_at.eq(now),
                shapes::updated_at.eq(now),
            ))
            .execute(conn)?;

        // Owner sees both shapes (no visible_to_players filter applied).
        let owner_is_owner = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .filter(scenes::owner_id.eq(owner_id))
            .select(scenes::scene_id)
            .first::<uuid::Uuid>(conn)
            .optional()?
            .is_some();
        assert!(owner_is_owner);
        let owner_visible_ids: Vec<uuid::Uuid> = shapes::table
            .filter(shapes::scene_id.eq(scene_id))
            .select(shapes::shape_id)
            .load(conn)?;
        assert_eq!(
            owner_visible_ids.len(),
            2,
            "the scene owner must see all shapes, including GM-only ones"
        );

        // Non-owner (intruder/player) sees only the visible_to_players shape.
        let intruder_is_owner = scenes::table
            .filter(scenes::scene_id.eq(scene_id))
            .filter(scenes::owner_id.eq(intruder_id))
            .select(scenes::scene_id)
            .first::<uuid::Uuid>(conn)
            .optional()?
            .is_some();
        assert!(!intruder_is_owner);
        let player_visible_ids: Vec<uuid::Uuid> = shapes::table
            .filter(shapes::scene_id.eq(scene_id))
            .filter(shapes::visible_to_players.eq(true))
            .select(shapes::shape_id)
            .load(conn)?;
        assert_eq!(
            player_visible_ids,
            vec![player_visible_shape_id],
            "a non-owner must only see visible_to_players = true shapes"
        );

        Ok(())
    });
}

// ---------------------------------------------------------------------------
// Spec 082: players draw, and edit only their own.
// ---------------------------------------------------------------------------

pub(crate) mod players_draw {
    use async_graphql::{Request, Variables};
    use diesel::prelude::*;
    use serde_json::{Value, json};
    use uuid::Uuid;

    use crate::auth_middleware::AuthenticatedUser;
    use crate::models::NewWorldAuthoringToolRevocation;
    use crate::schema::{shapes, world_authoring_tool_revocations, world_events, world_members};
    use crate::state::AppState;
    use crate::test_support::{
        insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
        test_app_state,
    };

    pub(crate) struct Table {
        pub state: AppState,
        pub owner: Uuid,
        pub world: Uuid,
        pub scene: Uuid,
        pub a: Uuid,
        pub b: Uuid,
    }

    /// A world with an Owner, two players (A and B) and one scene.
    pub(crate) fn table() -> Table {
        let state = test_app_state();
        let mut conn = state.db_pool.get().unwrap();
        let owner = insert_test_user(&mut conn);
        let world = insert_test_world(&mut conn, owner);
        insert_test_world_member(&mut conn, world, owner, "Owner");
        let scene = insert_test_scene(&mut conn, world, owner);
        let a = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world, a, "Player");
        let b = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world, b, "Player");
        drop(conn);
        Table {
            state,
            owner,
            world,
            scene,
            a,
            b,
        }
    }

    fn signed_in(user_id: Uuid, is_admin: bool) -> AuthenticatedUser {
        AuthenticatedUser {
            user_id,
            session_id: Uuid::now_v7(),
            expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
            is_admin,
            role: if is_admin { "Admin" } else { "User" }.to_string(),
            disabled: false,
        }
    }

    pub(crate) async fn run(
        state: &AppState,
        user: Uuid,
        is_admin: bool,
        document: &str,
        variables: Value,
    ) -> async_graphql::Response {
        let schema = async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .data(state.clone())
        .finish();
        schema
            .execute(
                Request::new(document)
                    .variables(Variables::from_json(variables))
                    .data(signed_in(user, is_admin)),
            )
            .await
    }

    pub(crate) fn data(response: async_graphql::Response) -> Value {
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        response.data.into_json().unwrap()
    }

    const CREATE: &str = r#"mutation ($input: GraphQLCreateShapeInput!) {
        createShape(input: $input) { shapeId createdBy visibleToPlayers }
    }"#;
    const UPDATE: &str = r#"mutation ($id: UUID!, $input: GraphQLUpdateShapeInput!) {
        updateShape(shapeId: $id, input: $input) { shapeId geometry visibleToPlayers }
    }"#;
    const DELETE: &str = r#"mutation ($id: UUID!) { deleteShape(shapeId: $id) }"#;

    /// Draws a rectangle on the table's scene as `user`, answering its id.
    pub(crate) async fn draw(t: &Table, user: Uuid, visible: bool) -> Uuid {
        let answer = data(
            run(
                &t.state,
                user,
                false,
                CREATE,
                json!({ "input": {
                    "sceneId": t.scene, "kind": "RECT",
                    "geometry": { "x": 0, "y": 0, "w": 10, "h": 10 },
                    "visibleToPlayers": visible,
                }}),
            )
            .await,
        );
        answer["createShape"]["shapeId"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap()
    }

    /// The row as stored, and the world's event count: what a refused write
    /// must leave exactly as it was.
    fn snapshot(t: &Table, shape: Uuid) -> (Option<(Value, bool, Uuid)>, i64) {
        let mut conn = t.state.db_pool.get().unwrap();
        let row = shapes::table
            .filter(shapes::shape_id.eq(shape))
            .select((
                shapes::geometry,
                shapes::visible_to_players,
                shapes::updated_by,
            ))
            .first::<(Value, bool, Uuid)>(&mut conn)
            .optional()
            .unwrap();
        let events = world_events::table
            .filter(world_events::world_id.eq(t.world))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap();
        (row, events)
    }

    #[tokio::test]
    async fn a_player_draws_a_shape_that_is_theirs_and_always_visible() {
        let t = table();
        let answer = data(
            run(
                &t.state,
                t.a,
                false,
                CREATE,
                json!({ "input": {
                    "sceneId": t.scene, "kind": "RECT",
                    "geometry": { "x": 0, "y": 0, "w": 10, "h": 10 },
                    "visibleToPlayers": false,
                }}),
            )
            .await,
        );
        assert_eq!(answer["createShape"]["createdBy"], json!(t.a.to_string()));
        assert_eq!(
            answer["createShape"]["visibleToPlayers"],
            json!(true),
            "a player's shape is always visible to players"
        );
    }

    #[tokio::test]
    async fn a_player_moves_and_deletes_their_own_but_cannot_hide_it() {
        let t = table();
        let mine = draw(&t, t.a, true).await;

        let moved = data(
            run(
                &t.state,
                t.a,
                false,
                UPDATE,
                json!({ "id": mine, "input": {
                    "geometry": { "x": 5, "y": 5, "w": 10, "h": 10 },
                    "visibleToPlayers": false,
                }}),
            )
            .await,
        );
        assert_eq!(moved["updateShape"]["geometry"]["x"], json!(5));
        assert_eq!(moved["updateShape"]["visibleToPlayers"], json!(true));

        let deleted = data(run(&t.state, t.a, false, DELETE, json!({ "id": mine })).await);
        assert_eq!(deleted["deleteShape"], json!(true));
        assert!(snapshot(&t, mine).0.is_none());
    }

    #[tokio::test]
    async fn a_player_cannot_touch_the_gms_shape_or_another_players() {
        let t = table();
        let gms = data(
            run(
                &t.state,
                t.owner,
                false,
                CREATE,
                json!({ "input": {
                    "sceneId": t.scene, "kind": "RECT",
                    "geometry": { "x": 0, "y": 0 }, "visibleToPlayers": true,
                }}),
            )
            .await,
        )["createShape"]["shapeId"]
            .as_str()
            .unwrap()
            .parse::<Uuid>()
            .unwrap();
        let bs = draw(&t, t.b, true).await;

        for shape in [gms, bs] {
            let before = snapshot(&t, shape);
            let refused = run(
                &t.state,
                t.a,
                false,
                UPDATE,
                json!({ "id": shape, "input": { "geometry": { "x": 99, "y": 99 } } }),
            )
            .await;
            assert!(
                !refused.errors.is_empty(),
                "a player edited a shape not theirs"
            );
            let deleted = data(run(&t.state, t.a, false, DELETE, json!({ "id": shape })).await);
            assert_eq!(deleted["deleteShape"], json!(false));
            assert_eq!(snapshot(&t, shape), before, "a refusal must write nothing");
        }
    }

    #[tokio::test]
    async fn a_player_whose_shapes_tool_was_taken_cannot_draw() {
        let t = table();
        let mut conn = t.state.db_pool.get().unwrap();
        let member = world_members::table
            .filter(world_members::world_id.eq(t.world))
            .filter(world_members::user_id.eq(t.a))
            .select(world_members::id)
            .first::<Uuid>(&mut conn)
            .unwrap();
        diesel::insert_into(world_authoring_tool_revocations::table)
            .values(&NewWorldAuthoringToolRevocation {
                world_member_id: member,
                tool: "shapes".to_string(),
                revoked_by: Some(t.owner),
            })
            .execute(&mut conn)
            .unwrap();
        drop(conn);

        let refused = run(
            &t.state,
            t.a,
            false,
            CREATE,
            json!({ "input": { "sceneId": t.scene, "kind": "RECT", "geometry": {} } }),
        )
        .await;
        assert!(!refused.errors.is_empty());
    }

    #[tokio::test]
    async fn a_stranger_cannot_draw() {
        let t = table();
        let stranger = insert_test_user(&mut t.state.db_pool.get().unwrap());
        let refused = run(
            &t.state,
            stranger,
            false,
            CREATE,
            json!({ "input": { "sceneId": t.scene, "kind": "RECT", "geometry": {} } }),
        )
        .await;
        assert!(!refused.errors.is_empty());
    }

    #[tokio::test]
    async fn the_gm_still_draws_hidden_shapes() {
        let t = table();
        let answer = data(
            run(
                &t.state,
                t.owner,
                false,
                CREATE,
                json!({ "input": {
                    "sceneId": t.scene, "kind": "RECT",
                    "geometry": {}, "visibleToPlayers": false,
                }}),
            )
            .await,
        );
        assert_eq!(answer["createShape"]["visibleToPlayers"], json!(false));
    }

    #[tokio::test]
    async fn a_paused_world_refuses_a_players_drawing() {
        let t = table();
        let operator = insert_test_user(&mut t.state.db_pool.get().unwrap());
        data(
            run(
                &t.state,
                operator,
                true,
                r#"mutation ($world: UUID!) {
                    pauseWorldPlay(worldId: $world, grounds: "spec 082") { alreadyPaused }
                }"#,
                json!({ "world": t.world }),
            )
            .await,
        );
        let refused = run(
            &t.state,
            t.a,
            false,
            CREATE,
            json!({ "input": { "sceneId": t.scene, "kind": "RECT", "geometry": {} } }),
        )
        .await;
        assert!(!refused.errors.is_empty());
        let mut conn = t.state.db_pool.get().unwrap();
        let drawn = shapes::table
            .filter(shapes::scene_id.eq(t.scene))
            .count()
            .get_result::<i64>(&mut conn)
            .unwrap();
        assert_eq!(drawn, 0);
    }
}
