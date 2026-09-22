//! What `worldStatistics` counts, and what it refuses to tell a player.
//!
//! Driven through the real schema rather than the sync function alone, for
//! the reason `play_pause_tests` gives: what a player may ask for is a
//! property of the schema, and a test that calls the function directly proves
//! the arithmetic while leaving the gate untested.

use async_graphql::Request;
use diesel::prelude::*;
use serde_json::Value;
use uuid::Uuid;

use crate::auth_middleware::AuthenticatedUser;
use crate::schema::{world_actor_claims, world_actors, world_combatants, world_combats};
use crate::test_support::{
    insert_test_actor, insert_test_scene_named, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state, try_test_connection,
};

const QUERY: &str = r#"
    query Stats($worldId: UUID!) {
        worldStatistics(worldId: $worldId) {
            scenes
            members
            membersWithCharacter
            characters
            npcs
            tokens
            activeEncounter { round combatants }
        }
    }
"#;

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
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
    schema: &crate::graphql::AppSchema,
    who: AuthenticatedUser,
    world_id: Uuid,
) -> async_graphql::Response {
    schema
        .execute(
            Request::new(QUERY)
                .variables(async_graphql::Variables::from_json(serde_json::json!({
                    "worldId": world_id,
                })))
                .data(who),
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

/// Two scenes: one players may open and one they may not.
///
/// Both are set explicitly. `scenes.hidden` defaults to **true** (spec 022:
/// a world's auto-created scene starts hidden), so a scene inserted without
/// saying so is already hidden — which is exactly the mistake a count for a
/// player must not make, and the one this fixture first made.
fn seed_scenes(conn: &mut PgConnection, world_id: Uuid, gm: Uuid) -> (Uuid, Uuid) {
    use crate::schema::scenes;
    let open = insert_test_scene_named(conn, world_id, gm, "Open Scene");
    let hidden = insert_test_scene_named(conn, world_id, gm, "Hidden Scene");
    diesel::update(scenes::table.find(open))
        .set(scenes::hidden.eq(false))
        .execute(conn)
        .expect("open a scene");
    diesel::update(scenes::table.find(hidden))
        .set(scenes::hidden.eq(true))
        .execute(conn)
        .expect("hide a scene");
    (open, hidden)
}

/// One token standing on a scene.
fn insert_token(conn: &mut PgConnection, scene_id: Uuid) {
    use crate::schema::tokens;
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(tokens::table)
        .values((
            tokens::token_id.eq(Uuid::now_v7()),
            tokens::scene_id.eq(scene_id),
            tokens::x.eq(0.0),
            tokens::y.eq(0.0),
            tokens::rotation.eq(0.0),
            tokens::scale.eq(1.0),
            tokens::created_at.eq(now),
            tokens::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("place a token");
}

/// An NPC that is hidden from players, plus a character anyone may see.
fn seed_actors(conn: &mut PgConnection, world_id: Uuid, scene_id: Uuid, gm: Uuid) {
    // Two hidden NPCs, from `insert_test_actor`'s defaults (is_npc = true,
    // visible_to_players defaulting false).
    insert_test_actor(conn, world_id, scene_id, gm);
    insert_test_actor(conn, world_id, scene_id, gm);
    // One player character.
    let character = insert_test_actor(conn, world_id, scene_id, gm);
    diesel::update(world_actors::table.find(character))
        .set((
            world_actors::is_npc.eq(false),
            world_actors::actor_type.eq("character"),
        ))
        .execute(conn)
        .expect("make one actor a character");
}

#[tokio::test]
async fn a_game_master_is_told_every_figure_including_npcs() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let gm = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);
    let (open, hidden) = seed_scenes(&mut conn, world_id, gm);
    seed_actors(&mut conn, world_id, open, gm);
    insert_token(&mut conn, open);
    insert_token(&mut conn, hidden);

    let schema = schema(test_app_state());
    let stats = data(ask(&schema, as_user(gm), world_id).await);
    let stats = &stats["worldStatistics"];

    // Both scenes, the hidden one included: a Game Master's own scene is not
    // hidden from them.
    assert_eq!(stats["scenes"], 2);
    // The creator counts even with no `world_members` row of their own.
    assert_eq!(stats["members"], 1);
    assert_eq!(stats["membersWithCharacter"], 0);
    assert_eq!(stats["characters"], 1);
    assert_eq!(stats["npcs"], 2);
    assert_eq!(stats["tokens"], 2);
    assert!(stats["activeEncounter"].is_null());
}

#[tokio::test]
async fn a_player_is_told_fewer_figures_and_never_a_hidden_one() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let gm = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);
    insert_test_world_member(&mut conn, world_id, player, "Player");
    let (open, hidden) = seed_scenes(&mut conn, world_id, gm);
    seed_actors(&mut conn, world_id, open, gm);
    insert_token(&mut conn, open);
    insert_token(&mut conn, hidden);

    let schema = schema(test_app_state());
    let stats = data(ask(&schema, as_user(player), world_id).await);
    let stats = &stats["worldStatistics"];

    // The hidden scene is not in the total. A count that included it would
    // announce a place this player has not been shown.
    assert_eq!(stats["scenes"], 1);
    // Nor are the tokens standing on it, for the same reason.
    assert_eq!(stats["tokens"], 1);
    // The player's own row plus the creator, who has none.
    assert_eq!(stats["members"], 2);
    assert_eq!(stats["characters"], 1);
    // Absent, not zero and not "the ones you can see": the two hidden NPCs
    // must not be countable into a figure this player can read.
    assert!(
        stats["npcs"].is_null(),
        "a player was told an NPC figure: {:?}",
        stats["npcs"]
    );
}

#[tokio::test]
async fn a_claimed_character_counts_its_holder_and_a_running_fight_is_reported() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let gm = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);
    insert_test_world_member(&mut conn, world_id, player, "Player");
    let scene = insert_test_scene_named(&mut conn, world_id, gm, "Open Scene");
    let character = insert_test_actor(&mut conn, world_id, scene, gm);
    diesel::update(world_actors::table.find(character))
        .set(world_actors::is_npc.eq(false))
        .execute(&mut conn)
        .expect("make the actor a character");

    let member_id: Uuid = crate::schema::world_members::table
        .filter(crate::schema::world_members::world_id.eq(world_id))
        .filter(crate::schema::world_members::user_id.eq(player))
        .select(crate::schema::world_members::id)
        .first(&mut conn)
        .expect("the player's membership row");
    diesel::insert_into(world_actor_claims::table)
        .values((
            world_actor_claims::id.eq(Uuid::now_v7()),
            world_actor_claims::actor_id.eq(character),
            world_actor_claims::world_member_id.eq(member_id),
        ))
        .execute(&mut conn)
        .expect("claim the character");

    let combat_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_combats::table)
        .values((
            world_combats::id.eq(combat_id),
            world_combats::world_id.eq(world_id),
            world_combats::scene_id.eq(Some(scene)),
            world_combats::round.eq(3),
            world_combats::created_by.eq(gm),
            world_combats::created_at.eq(now),
            world_combats::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .expect("start a fight");
    for (index, label) in ["Hero", "Goblin"].iter().enumerate() {
        diesel::insert_into(world_combatants::table)
            .values((
                world_combatants::id.eq(Uuid::now_v7()),
                world_combatants::combat_id.eq(combat_id),
                world_combatants::label.eq(*label),
                world_combatants::initiative.eq(20 - index as i32),
                world_combatants::tiebreak.eq(index as i32),
                world_combatants::is_npc.eq(index == 1),
                world_combatants::active.eq(index == 0),
                world_combatants::kind.eq("creature"),
                world_combatants::created_at.eq(now),
                world_combatants::updated_at.eq(now),
            ))
            .execute(&mut conn)
            .expect("add a combatant");
    }

    let schema = schema(test_app_state());
    let stats = data(ask(&schema, as_user(player), world_id).await);
    let stats = &stats["worldStatistics"];

    assert_eq!(stats["members"], 2);
    assert_eq!(stats["membersWithCharacter"], 1);
    assert_eq!(stats["activeEncounter"]["round"], 3);
    assert_eq!(stats["activeEncounter"]["combatants"], 2);
}

#[tokio::test]
async fn a_stranger_is_refused() {
    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let gm = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, gm);

    let schema = schema(test_app_state());
    let response = ask(&schema, as_user(stranger), world_id).await;
    assert!(
        !response.errors.is_empty(),
        "a non-member was told this world's figures"
    );
}

/// The owner's five-year campaign: forty scenes and two thousand tokens.
///
/// The claim is that the dashboard costs the same at this size as at three
/// scenes, because the answer is a handful of counts rather than rows. The
/// bound is generous on purpose, because a shared test machine is noisy. A
/// regression to loading every token would still land far outside it. The
/// elapsed time is printed so a run can report it.
#[tokio::test]
// The timings are read by a person with --nocapture; the assertions are the test.
#[allow(clippy::print_stdout)]
async fn forty_scenes_and_two_thousand_tokens_are_counted_without_loading_them() {
    use crate::schema::{scenes, tokens};

    let Some(mut conn) = try_test_connection() else {
        return;
    };
    let gm = insert_test_user(&mut conn);
    let small_world = insert_test_world(&mut conn, gm);
    let big_world = insert_test_world(&mut conn, gm);
    for index in 0..3 {
        insert_test_scene_named(&mut conn, small_world, gm, &format!("Small {index}"));
    }

    let now = chrono::Utc::now().naive_utc();
    let mut scene_ids = Vec::with_capacity(40);
    for index in 0..40 {
        let id = insert_test_scene_named(&mut conn, big_world, gm, &format!("Session {index}"));
        scene_ids.push(id);
    }
    diesel::update(scenes::table.filter(scenes::world_id.eq(big_world)))
        .set(scenes::hidden.eq(false))
        .execute(&mut conn)
        .expect("open the scenes");
    let rows: Vec<_> = (0..2_000)
        .map(|index| {
            (
                tokens::token_id.eq(Uuid::now_v7()),
                tokens::scene_id.eq(scene_ids[index % scene_ids.len()]),
                tokens::x.eq(0.0),
                tokens::y.eq(0.0),
                tokens::rotation.eq(0.0),
                tokens::scale.eq(1.0),
                tokens::created_at.eq(now),
                tokens::updated_at.eq(now),
            )
        })
        .collect();
    for chunk in rows.chunks(500) {
        diesel::insert_into(tokens::table)
            .values(chunk)
            .execute(&mut conn)
            .expect("place two thousand tokens");
    }

    let schema = schema(test_app_state());
    // Warm the schema and the pool, so the first measurement is not paying
    // for either.
    let _ = ask(&schema, as_user(gm), small_world).await;

    let started = std::time::Instant::now();
    let small = data(ask(&schema, as_user(gm), small_world).await);
    let small_elapsed = started.elapsed();

    let started = std::time::Instant::now();
    let big = data(ask(&schema, as_user(gm), big_world).await);
    let big_elapsed = started.elapsed();

    println!(
        "[world-statistics] 3 scenes / 0 tokens: {small_elapsed:?}; 40 scenes / 2000 tokens: {big_elapsed:?}"
    );

    assert_eq!(small["worldStatistics"]["scenes"], 3);
    assert_eq!(big["worldStatistics"]["scenes"], 40);
    assert_eq!(big["worldStatistics"]["tokens"], 2_000);
    assert!(
        big_elapsed < std::time::Duration::from_millis(500),
        "counting a large world took {big_elapsed:?}"
    );
}
