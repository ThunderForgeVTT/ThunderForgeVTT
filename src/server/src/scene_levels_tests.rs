//! Levels against a real database: what the migration promises, what a Game
//! Master may and may not do to a floor, and — through the GraphQL schema
//! itself — that a player downstairs is not shown who is upstairs.

use super::*;
use crate::auth_middleware::AuthenticatedUser;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};
use async_graphql::Request;

/// A world with a Game Master and two players, and one scene.
pub(crate) struct Table {
    pub state: crate::state::AppState,
    pub gm: Uuid,
    pub ann: Uuid,
    pub ben: Uuid,
    pub world_id: Uuid,
    pub scene_id: Uuid,
    /// The level the scene was born with.
    pub ground: Uuid,
}

pub(crate) fn seat_a_table() -> Table {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);
    let scene_id = insert_test_scene(&mut conn, world_id, gm);
    let ann = insert_test_user(&mut conn);
    let ben = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, ann, "Player");
    insert_test_world_member(&mut conn, world_id, ben, "Player");
    let ground = crate::auth::level_visibility::entry_level(&mut conn, scene_id)
        .expect("a scene is born with an entry level");
    drop(conn);
    Table {
        state,
        gm,
        ann,
        ben,
        world_id,
        scene_id,
        ground,
    }
}

impl Table {
    pub fn conn(
        &self,
    ) -> diesel::r2d2::PooledConnection<diesel::r2d2::ConnectionManager<PgConnection>> {
        self.state.db_pool.get().unwrap()
    }

    /// A level added by the Game Master.
    pub fn level(&self, name: &str) -> Uuid {
        create_level(
            &mut self.conn(),
            self.gm,
            false,
            self.scene_id,
            NewLevel {
                name: name.to_string(),
                ..NewLevel::default()
            },
        )
        .expect("the Game Master adds a level")
        .level_id
    }

    /// A token owned by `owner`, on `level` when one is named and wherever
    /// the database puts it when not.
    pub fn token(&self, owner: Uuid, level: Option<Uuid>, at: (f64, f64)) -> Uuid {
        use crate::schema::tokens;
        let token_id = Uuid::now_v7();
        let now = chrono::Utc::now().naive_utc();
        diesel::insert_into(tokens::table)
            .values((
                tokens::token_id.eq(token_id),
                tokens::scene_id.eq(self.scene_id),
                tokens::x.eq(at.0),
                tokens::y.eq(at.1),
                tokens::rotation.eq(0.0),
                tokens::scale.eq(1.0),
                tokens::owner_user_id.eq(owner),
                level.map(|level| tokens::level_id.eq(level)),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            ))
            .execute(&mut self.conn())
            .expect("a token is placed");
        token_id
    }

    pub fn level_of_token(&self, token_id: Uuid) -> Uuid {
        use crate::schema::tokens;
        tokens::table
            .filter(tokens::token_id.eq(token_id))
            .select(tokens::level_id)
            .first(&mut self.conn())
            .unwrap()
    }

    /// Ask the real schema, as `user`.
    pub async fn ask(&self, user: Uuid, document: String) -> async_graphql::Response {
        async_graphql::Schema::build(
            crate::graphql::QueryRoot::default(),
            crate::graphql::MutationRoot::default(),
            crate::graphql::SubscriptionRoot,
        )
        .data(self.state.clone())
        .finish()
        .execute(Request::new(document).data(AuthenticatedUser {
            user_id: user,
            session_id: Uuid::now_v7(),
            expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
            is_admin: false,
            role: "Player".to_string(),
            disabled: false,
        }))
        .await
    }

    /// The token ids `user` is shown by `tokens(sceneId, levelId)`.
    pub async fn tokens_seen(&self, user: Uuid, level: Option<Uuid>) -> Vec<String> {
        let level = level.map_or(String::new(), |level| format!(", levelId: \"{level}\""));
        let response = self
            .ask(
                user,
                format!(
                    "{{ tokens(sceneId: \"{}\"{level}) {{ tokenId }} }}",
                    self.scene_id
                ),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        response.data.into_json().unwrap()["tokens"]
            .as_array()
            .unwrap()
            .iter()
            .map(|token| token["tokenId"].as_str().unwrap().to_string())
            .collect()
    }
}

fn why<T: std::fmt::Debug>(result: Result<T, LevelError>) -> String {
    match result {
        Err(LevelError::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn every_scene_has_exactly_one_entry_level_and_new_things_land_on_it() {
    use crate::schema::{scene_levels, walls};
    let t = seat_a_table();
    let mut conn = t.conn();

    // The invariant the migration's backfill and its trigger both exist
    // for, asked of every scene in the database, not only this one.
    let offenders: i64 = diesel::dsl::sql::<diesel::sql_types::BigInt>(
        "SELECT count(*) FROM scenes s WHERE \
         (SELECT count(*) FROM scene_levels l WHERE l.scene_id = s.scene_id AND l.is_entry) <> 1",
    )
    .get_result(&mut conn)
    .unwrap();
    assert_eq!(offenders, 0, "every scene has exactly one entry level");

    let (name, order): (String, i32) = scene_levels::table
        .filter(scene_levels::level_id.eq(t.ground))
        .select((scene_levels::name, scene_levels::sort_order))
        .first(&mut conn)
        .unwrap();
    assert_eq!((name.as_str(), order), ("Ground", 0));

    // Writers that predate levels name none. They land on the entry level,
    // even when it is no longer the lowest floor.
    t.level("Loft");
    let token = t.token(t.ann, None, (10.0, 10.0));
    assert_eq!(t.level_of_token(token), t.ground);

    let wall_level: Uuid = diesel::insert_into(walls::table)
        .values((
            walls::scene_id.eq(t.scene_id),
            walls::x1.eq(0.0),
            walls::y1.eq(0.0),
            walls::x2.eq(10.0),
            walls::y2.eq(0.0),
            walls::created_by.eq(t.gm),
            walls::updated_by.eq(t.gm),
        ))
        .returning(walls::level_id)
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(wall_level, t.ground);
}

#[test]
fn only_a_game_master_makes_levels_and_only_twelve() {
    let t = seat_a_table();
    let mut conn = t.conn();
    let new = |name: &str| NewLevel {
        name: name.to_string(),
        ..NewLevel::default()
    };

    assert_eq!(
        why(create_level(
            &mut conn,
            t.ann,
            false,
            t.scene_id,
            new("Loft")
        )),
        "Only the Game Master may change a scene's levels"
    );
    assert_eq!(
        why(create_level(&mut conn, t.gm, false, t.scene_id, new("  "))),
        "A level needs a name"
    );

    // Eleven more on top of the one the scene was born with.
    for floor in 1..MAX_LEVELS_PER_SCENE {
        let level = create_level(
            &mut conn,
            t.gm,
            false,
            t.scene_id,
            new(&format!("Floor {floor}")),
        )
        .expect("up to twelve");
        assert_eq!(i64::from(level.sort_order), floor, "each goes on top");
        assert!(!level.is_entry);
    }
    assert_eq!(
        why(create_level(
            &mut conn,
            t.gm,
            false,
            t.scene_id,
            new("Thirteenth")
        )),
        "A scene has at most 12 levels"
    );
}

#[test]
fn a_level_is_not_deleted_while_it_is_the_last_the_entry_or_stood_upon() {
    use crate::schema::{scene_levels, walls};
    let t = seat_a_table();
    let mut conn = t.conn();

    assert_eq!(
        why(delete_level(&mut conn, t.gm, false, t.ground)),
        "A scene keeps at least one level"
    );

    let loft = t.level("Loft");
    assert_eq!(
        why(delete_level(&mut conn, t.gm, false, t.ground)),
        "This is the scene's entry level. Make another level the entry before deleting it"
    );
    assert_eq!(
        why(delete_level(&mut conn, t.ann, false, loft)),
        "Only the Game Master may change a scene's levels"
    );

    let token = t.token(t.ann, Some(loft), (5.0, 5.0));
    diesel::insert_into(walls::table)
        .values((
            walls::scene_id.eq(t.scene_id),
            walls::level_id.eq(loft),
            walls::x1.eq(0.0),
            walls::y1.eq(0.0),
            walls::x2.eq(10.0),
            walls::y2.eq(0.0),
            walls::created_by.eq(t.gm),
            walls::updated_by.eq(t.gm),
        ))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(
        why(delete_level(&mut conn, t.gm, false, loft)),
        "Move the tokens off this level before deleting it"
    );

    // Carried downstairs, the floor can go — and its scenery goes with it,
    // while the token is still on the board.
    move_tokens_to_level(&mut conn, t.gm, false, &[token], t.ground, None).unwrap();
    delete_level(&mut conn, t.gm, false, loft).expect("an empty upper floor is deleted");
    let left: i64 = scene_levels::table
        .filter(scene_levels::scene_id.eq(t.scene_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(left, 1);
    let walls_left: i64 = walls::table
        .filter(walls::scene_id.eq(t.scene_id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(walls_left, 0, "a floor's walls go with it");
    assert_eq!(t.level_of_token(token), t.ground);
}

#[test]
fn the_entry_level_and_the_scene_stay_one_board() {
    use crate::schema::scenes;
    let t = seat_a_table();
    let mut conn = t.conn();
    let board = |conn: &mut PgConnection| -> (i32, i32, String) {
        scenes::table
            .filter(scenes::scene_id.eq(t.scene_id))
            .select((scenes::width, scenes::height, scenes::ambient_light))
            .first(conn)
            .unwrap()
    };

    // Editing the entry level edits the scene.
    let ground = update_level(
        &mut conn,
        t.gm,
        false,
        t.ground,
        LevelChanges {
            name: Some("Taproom".to_string()),
            width: Some(640),
            ambient_light: Some("Dim".to_string()),
            ..LevelChanges::default()
        },
    )
    .unwrap();
    assert_eq!((ground.name.as_str(), ground.width), ("Taproom", 640));
    assert_eq!(ground.ambient_light, "dim");
    let (width, _, light) = board(&mut conn);
    assert_eq!((width, light.as_str()), (640, "dim"));

    // Editing another level does not.
    let cellar = t.level("Cellar");
    update_level(
        &mut conn,
        t.gm,
        false,
        cellar,
        LevelChanges {
            width: Some(320),
            ambient_light: Some("dark".to_string()),
            ..LevelChanges::default()
        },
    )
    .unwrap();
    assert_eq!(board(&mut conn).0, 640);

    // Making it the entry brings its board to the scene, and leaves one entry.
    let now_entry = update_level(
        &mut conn,
        t.gm,
        false,
        cellar,
        LevelChanges {
            make_entry: true,
            ..LevelChanges::default()
        },
    )
    .unwrap();
    assert!(now_entry.is_entry);
    let (width, _, light) = board(&mut conn);
    assert_eq!((width, light.as_str()), (320, "dark"));
    assert_eq!(
        crate::auth::level_visibility::entry_level(&mut conn, t.scene_id).unwrap(),
        cellar
    );
    // The old entry keeps the board it had.
    let taproom = load_level(&mut conn, t.ground).unwrap();
    assert_eq!((taproom.is_entry, taproom.width), (false, 640));

    assert_eq!(
        why(update_level(
            &mut conn,
            t.gm,
            false,
            cellar,
            LevelChanges {
                ambient_light: Some("gloomy".to_string()),
                ..LevelChanges::default()
            },
        )),
        "A level's light is bright, dim or dark"
    );
    assert_eq!(
        why(update_level(
            &mut conn,
            t.gm,
            false,
            cellar,
            LevelChanges {
                background_asset_id: Some(Uuid::now_v7()),
                ..LevelChanges::default()
            },
        )),
        "That image is not in this world"
    );
}

#[test]
fn levels_are_reordered_only_by_naming_all_of_them() {
    let t = seat_a_table();
    let mut conn = t.conn();
    let loft = t.level("Loft");
    let roof = t.level("Roof");

    assert_eq!(
        why(reorder_levels(
            &mut conn,
            t.gm,
            false,
            t.scene_id,
            &[roof, loft]
        )),
        "Name every level of the scene once, in the order you want"
    );
    assert_eq!(
        why(reorder_levels(
            &mut conn,
            t.gm,
            false,
            t.scene_id,
            &[roof, loft, loft]
        )),
        "Name every level of the scene once, in the order you want"
    );

    let order: Vec<Uuid> =
        reorder_levels(&mut conn, t.gm, false, t.scene_id, &[roof, t.ground, loft])
            .unwrap()
            .into_iter()
            .map(|level| level.level_id)
            .collect();
    assert_eq!(order, vec![roof, t.ground, loft]);
    assert_eq!(
        crate::auth::level_visibility::entry_level(&mut conn, t.scene_id).unwrap(),
        t.ground,
        "order and entry are separate facts"
    );
}

#[test]
fn a_game_master_carries_tokens_between_levels_with_their_lights() {
    use crate::schema::{light_sources, tokens};
    let t = seat_a_table();
    let mut conn = t.conn();
    let loft = t.level("Loft");
    let ann = t.token(t.ann, None, (100.0, 100.0));
    let ben = t.token(t.ben, None, (200.0, 100.0));

    // A torch in Ann's hand. Placed without a level, it takes hers.
    let torch: Uuid = diesel::insert_into(light_sources::table)
        .values((
            light_sources::scene_id.eq(t.scene_id),
            light_sources::x.eq(100.0),
            light_sources::y.eq(100.0),
            light_sources::radius.eq(40.0),
            light_sources::bright_radius.eq(20.0),
            light_sources::intensity.eq(1.0),
            light_sources::attached_token_id.eq(ann),
            light_sources::created_by.eq(t.gm),
            light_sources::updated_by.eq(t.gm),
        ))
        .returning(light_sources::light_id)
        .get_result(&mut conn)
        .unwrap();

    assert_eq!(
        why(move_tokens_to_level(
            &mut conn,
            t.ann,
            false,
            &[ann],
            loft,
            None
        )),
        "Only the Game Master may change a scene's levels"
    );

    // A token from another scene is not carried anywhere.
    let elsewhere =
        crate::test_support::insert_test_scene_named(&mut conn, t.world_id, t.gm, "Elsewhere");
    let stranger = Uuid::now_v7();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(stranger),
            tokens::scene_id.eq(elsewhere),
            tokens::x.eq(0.0),
            tokens::y.eq(0.0),
            tokens::rotation.eq(0.0),
            tokens::scale.eq(1.0),
        ))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(
        why(move_tokens_to_level(
            &mut conn,
            t.gm,
            false,
            &[ann, stranger],
            loft,
            None
        )),
        "Every token must be in the same scene as the level"
    );
    assert_eq!(
        t.level_of_token(ann),
        t.ground,
        "a refused move moves nobody"
    );

    // Both, to one point: they arrive beside each other, not stacked.
    let moved = move_tokens_to_level(
        &mut conn,
        t.gm,
        false,
        &[ann, ben],
        loft,
        Some((400.0, 400.0)),
    )
    .unwrap();
    assert_eq!(moved.len(), 2);
    assert!(moved.iter().all(|token| token.level_id == loft));
    assert_eq!((moved[0].x, moved[0].y), (400.0, 400.0));
    assert_ne!((moved[1].x, moved[1].y), (400.0, 400.0));

    let (torch_level, x, y): (Uuid, f64, f64) = light_sources::table
        .filter(light_sources::light_id.eq(torch))
        .select((light_sources::level_id, light_sources::x, light_sources::y))
        .first(&mut conn)
        .unwrap();
    assert_eq!(
        (torch_level, x, y),
        (loft, 400.0, 400.0),
        "the torch went up too"
    );

    let counts = token_counts(&mut conn, t.scene_id).unwrap();
    assert_eq!(counts.get(&loft), Some(&2));
    assert_eq!(counts.get(&t.ground), None);
}

#[tokio::test]
async fn a_player_downstairs_is_not_shown_who_is_upstairs() {
    let t = seat_a_table();
    let loft = t.level("Loft");
    let ann = t.token(t.ann, Some(loft), (10.0, 10.0)).to_string();
    let ben = t.token(t.ben, None, (10.0, 10.0)).to_string();

    // Ben, downstairs: himself, and nothing of the loft however he asks.
    assert_eq!(t.tokens_seen(t.ben, None).await, vec![ben.clone()]);
    assert_eq!(
        t.tokens_seen(t.ben, Some(t.ground)).await,
        vec![ben.clone()]
    );
    assert!(t.tokens_seen(t.ben, Some(loft)).await.is_empty());
    assert!(
        t.tokens_seen(t.ben, Some(Uuid::now_v7())).await.is_empty(),
        "a floor that does not exist reads the same as one that is not his"
    );

    // Ann, upstairs: herself, and — because she is not on it — not the
    // ground floor either, entry level though it is.
    assert_eq!(t.tokens_seen(t.ann, None).await, vec![ann.clone()]);
    assert!(t.tokens_seen(t.ann, Some(t.ground)).await.is_empty());

    // The Game Master sees both floors, one at a time.
    assert_eq!(t.tokens_seen(t.gm, None).await, vec![ben.clone()]);
    assert_eq!(t.tokens_seen(t.gm, Some(loft)).await, vec![ann.clone()]);

    // And the list of floors says the same thing.
    let levels = async |user: Uuid| {
        let response = t
            .ask(
                user,
                format!(
                    "{{ sceneLevels(sceneId: \"{}\") {{ levelId name tokenCount }} }}",
                    t.scene_id
                ),
            )
            .await;
        assert!(response.errors.is_empty(), "{:?}", response.errors);
        response.data.into_json().unwrap()["sceneLevels"].clone()
    };
    assert_eq!(
        levels(t.ben).await,
        serde_json::json!([{ "levelId": t.ground.to_string(), "name": "Ground", "tokenCount": null }])
    );
    assert_eq!(
        levels(t.ann).await,
        serde_json::json!([{ "levelId": loft.to_string(), "name": "Loft", "tokenCount": null }])
    );
    assert_eq!(
        levels(t.gm).await,
        serde_json::json!([
            { "levelId": t.ground.to_string(), "name": "Ground", "tokenCount": 1 },
            { "levelId": loft.to_string(), "name": "Loft", "tokenCount": 1 },
        ])
    );
}

#[tokio::test]
async fn the_level_mutations_are_in_the_schema_and_refuse_a_player() {
    let t = seat_a_table();
    let create = format!(
        "mutation {{ createSceneLevel(input: {{ sceneId: \"{}\", name: \"Loft\" }}) \
         {{ levelId sortOrder isEntry tokenCount backgroundUrl }} }}",
        t.scene_id
    );

    let refused = t.ask(t.ann, create.clone()).await;
    assert_eq!(
        refused.errors[0].message,
        "Only the Game Master may change a scene's levels"
    );

    let made = t.ask(t.gm, create).await;
    assert!(made.errors.is_empty(), "{:?}", made.errors);
    let made = made.data.into_json().unwrap();
    let loft = made["createSceneLevel"]["levelId"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(made["createSceneLevel"]["sortOrder"], 1);
    assert_eq!(made["createSceneLevel"]["tokenCount"], 0);

    let token = t.token(t.ann, None, (10.0, 10.0));
    let lift = format!(
        "mutation {{ moveTokensToLevel(tokenIds: [\"{token}\"], levelId: \"{loft}\", x: 64, y: 64) \
         {{ tokenId levelId x y }} }}"
    );
    assert!(!t.ask(t.ann, lift.clone()).await.errors.is_empty());
    let lifted = t.ask(t.gm, lift).await;
    assert!(lifted.errors.is_empty(), "{:?}", lifted.errors);
    assert_eq!(
        lifted.data.into_json().unwrap()["moveTokensToLevel"][0]["levelId"],
        loft
    );

    let delete = format!("mutation {{ deleteSceneLevel(levelId: \"{loft}\") }}");
    assert_eq!(
        t.ask(t.gm, delete).await.errors[0].message,
        "Move the tokens off this level before deleting it"
    );
}

#[test]
fn copying_a_scene_copies_its_levels_and_keeps_things_on_them() {
    use crate::schema::{scene_levels, walls};
    let t = seat_a_table();
    let mut conn = t.conn();
    let loft = t.level("Loft");
    update_level(
        &mut conn,
        t.gm,
        false,
        t.ground,
        LevelChanges {
            name: Some("Taproom".to_string()),
            ..LevelChanges::default()
        },
    )
    .unwrap();
    for (level, x) in [(t.ground, 1.0), (loft, 2.0)] {
        diesel::insert_into(walls::table)
            .values((
                walls::scene_id.eq(t.scene_id),
                walls::level_id.eq(level),
                walls::x1.eq(x),
                walls::y1.eq(0.0),
                walls::x2.eq(x),
                walls::y2.eq(10.0),
                walls::created_by.eq(t.gm),
                walls::updated_by.eq(t.gm),
            ))
            .execute(&mut conn)
            .unwrap();
    }

    let mut ctx = crate::collections::copy::CopyContext {
        destination_world_id: t.world_id,
        user_id: t.gm,
        ability_map: Default::default(),
        item_map: Default::default(),
        actor_map: Default::default(),
        lore_map: Default::default(),
        scene_map: Default::default(),
        created: Vec::new(),
        notes: Vec::new(),
    };
    crate::collections::scene_copy::copy_scene(&mut conn, &mut ctx, t.scene_id)
        .unwrap_or_else(|_| panic!("the scene copies"));
    let copy = ctx.scene_map[&t.scene_id];

    let levels: Vec<(Uuid, String, i32, bool)> = scene_levels::table
        .filter(scene_levels::scene_id.eq(copy))
        .order(scene_levels::sort_order.asc())
        .select((
            scene_levels::level_id,
            scene_levels::name,
            scene_levels::sort_order,
            scene_levels::is_entry,
        ))
        .load(&mut conn)
        .unwrap();
    assert_eq!(
        levels
            .iter()
            .map(|(_, name, order, entry)| (name.as_str(), *order, *entry))
            .collect::<Vec<_>>(),
        vec![("Taproom", 0, true), ("Loft", 1, false)]
    );
    assert!(
        levels.iter().all(|(id, ..)| *id != t.ground && *id != loft),
        "the copy has levels of its own"
    );

    // Each wall is on the copy's counterpart of the floor it was on.
    let placed: Vec<(f64, Uuid)> = walls::table
        .filter(walls::scene_id.eq(copy))
        .order(walls::x1.asc())
        .select((walls::x1, walls::level_id))
        .load(&mut conn)
        .unwrap();
    assert_eq!(placed, vec![(1.0, levels[0].0), (2.0, levels[1].0)]);
}
