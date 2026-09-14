//! Spec 046 T041 (FR-015, FR-016, FR-017, SC-008; ADR-102): linked tokens,
//! unlinked copies and unique NPCs.
//!
//! Placement is tested through `create_token_impl`, the resolver's own core,
//! so a default decided anywhere but the server would fail here. Damage is
//! tested through `combat::hit_points`, the one path hit points take.

use super::*;
use crate::combat::hit_points::{HitPointChangeKind, apply_hit_point_change};
use crate::graphql::GraphQLCreateTokenInput;
use crate::graphql::mutations_tokens::create_token_impl;
use crate::schema::{world_combatants, world_combats, world_events};
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};
use serde_json::json;

const SYSTEMS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packs/systems");

fn actor(is_npc: bool, is_unique: bool, actor_type: &str) -> PlacementActor {
    PlacementActor {
        world_id: Uuid::nil(),
        actor_type: actor_type.to_string(),
        is_npc,
        is_unique,
    }
}

// ---------------------------------------------------------------------------
// The rule, alone
// ---------------------------------------------------------------------------

#[test]
fn placement_defaults_follow_what_the_actor_is() {
    assert!(default_linked(Some(&actor(false, false, "character"))));
    assert!(!default_linked(Some(&actor(true, false, "npc"))));
    assert!(default_linked(Some(&actor(true, true, "npc"))));
    assert!(!default_linked(None));
    // A unique flag on a character changes nothing: characters are linked.
    assert!(default_linked(Some(&actor(false, true, "character"))));
}

#[test]
fn a_game_master_may_choose_otherwise_but_not_link_a_token_to_nothing() {
    let npc = actor(true, false, "npc");
    assert_eq!(
        placement_for(Some(&npc), Some(true)),
        Ok(Placement {
            linked: true,
            seeds_from_actor: false
        })
    );
    let pc = actor(false, false, "character");
    assert_eq!(
        placement_for(Some(&pc), Some(false)),
        Ok(Placement {
            linked: false,
            seeds_from_actor: true
        })
    );
    assert!(placement_for(None, Some(true)).is_err());
    assert_eq!(
        placement_for(None, None),
        Ok(Placement {
            linked: false,
            seeds_from_actor: false
        })
    );
}

#[test]
fn an_npc_token_is_typed_npc_even_when_its_actor_says_character() {
    // Real rows carry `actor_type = 'character'` with `is_npc = true`.
    assert_eq!(
        token_kind_for_actor(&actor(true, false, "character")),
        TokenKind::Npc
    );
    assert_eq!(
        token_kind_for_actor(&actor(false, false, "character")),
        TokenKind::Character
    );
    assert_eq!(
        token_kind_for_actor(&actor(true, false, "vehicle")),
        TokenKind::Vehicle
    );
    assert_eq!(
        token_kind_for_actor(&actor(false, false, "prop")),
        TokenKind::Object
    );
}

// ---------------------------------------------------------------------------
// Against the database
// ---------------------------------------------------------------------------

struct Table {
    gm: Uuid,
    player: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
}

fn table(conn: &mut PgConnection) -> Table {
    let gm = insert_test_user(conn);
    let player = insert_test_user(conn);
    let world_id = insert_test_world(conn, gm);
    insert_test_world_member(conn, world_id, player, "Player");
    diesel::update(crate::schema::worlds::table.filter(crate::schema::worlds::id.eq(world_id)))
        .set(crate::schema::worlds::game_system_id.eq("dnd5e"))
        .execute(conn)
        .expect("world system");
    let scene_id = insert_test_scene(conn, world_id, gm);
    Table {
        gm,
        player,
        world_id,
        scene_id,
    }
}

/// A dnd5e actor with hit points.
fn creature(
    conn: &mut PgConnection,
    t: &Table,
    label: &str,
    is_npc: bool,
    resources: serde_json::Value,
) -> Uuid {
    let id = Uuid::now_v7();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(id),
            world_actors::world_id.eq(t.world_id),
            world_actors::scene_id.eq(t.scene_id),
            world_actors::actor_type.eq(if is_npc { "npc" } else { "character" }),
            world_actors::game_system_id.eq("dnd5e"),
            world_actors::label.eq(label),
            world_actors::created_by.eq(t.gm),
            world_actors::owned_by.eq(t.gm),
            world_actors::is_npc.eq(is_npc),
        ))
        .execute(conn)
        .expect("actor");
    diesel::insert_into(world_actor_system_data::table)
        .values((
            world_actor_system_data::actor_id.eq(id),
            world_actor_system_data::game_system_id.eq("dnd5e"),
            world_actor_system_data::resource_data.eq(resources),
            world_actor_system_data::created_by.eq(t.gm),
            world_actor_system_data::updated_by.eq(t.gm),
        ))
        .execute(conn)
        .expect("system data");
    id
}

fn input(t: &Table, actor_id: Option<Uuid>, linked: Option<bool>) -> GraphQLCreateTokenInput {
    GraphQLCreateTokenInput {
        scene_id: t.scene_id,
        actor_id,
        x: 0.0,
        y: 0.0,
        rotation: None,
        scale: None,
        metadata: None,
        token_type: None,
        linked,
    }
}

type Stored = (bool, Option<serde_json::Value>, String);

async fn place(state: &AppState, t: &Table, actor_id: Option<Uuid>, linked: Option<bool>) -> Uuid {
    create_token_impl(state, t.gm, false, input(t, actor_id, linked))
        .await
        .expect("placed")
        .id()
}

fn stored_token(conn: &mut PgConnection, token_id: Uuid) -> Stored {
    tokens::table
        .filter(tokens::token_id.eq(token_id))
        .select((tokens::linked, tokens::system_data, tokens::token_type))
        .first(conn)
        .expect("token")
}

fn actor_data(conn: &mut PgConnection, actor_id: Uuid) -> serde_json::Value {
    actor_resource_data(conn, actor_id)
        .expect("read")
        .expect("present")
}

fn damage(conn: &mut PgConnection, t: &Table, token_id: Uuid, amount: i32) -> i32 {
    apply_hit_point_change(
        conn,
        SYSTEMS_DIR,
        token_id,
        HitPointChangeKind::Damage,
        amount,
        t.gm,
    )
    .expect("applied")
    .after
    .current
}

#[tokio::test]
async fn placement_defaults_for_character_npc_unique_npc_and_actorless() {
    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let (aria, goblin, boblin) = {
        let mut conn = state.db_pool.get().expect("conn");
        let aria = creature(
            &mut conn,
            &t,
            "Aria",
            false,
            json!({ "current_hp": 12, "max_hp": 12 }),
        );
        let goblin = creature(
            &mut conn,
            &t,
            "Goblin",
            true,
            json!({ "current_hp": 7, "max_hp": 7 }),
        );
        let boblin = creature(
            &mut conn,
            &t,
            "Boblin",
            true,
            json!({ "current_hp": 9, "max_hp": 9 }),
        );
        (aria, goblin, boblin)
    };
    set_actor_unique_impl(&state, t.gm, false, boblin, true)
        .await
        .expect("a Game Master marks Boblin unique");

    let aria_token = place(&state, &t, Some(aria), None).await;
    let goblin_token = place(&state, &t, Some(goblin), None).await;
    let boblin_token = place(&state, &t, Some(boblin), None).await;
    let marker = place(&state, &t, None, None).await;

    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(
        stored_token(&mut conn, aria_token),
        (true, None, "character".into())
    );
    assert_eq!(
        stored_token(&mut conn, goblin_token),
        (
            false,
            Some(json!({ "current_hp": 7, "max_hp": 7 })),
            "npc".into()
        ),
        "an NPC's token is a copy, seeded from the NPC, and typed npc"
    );
    assert_eq!(
        stored_token(&mut conn, boblin_token),
        (true, None, "npc".into()),
        "a unique NPC's token is linked"
    );
    assert_eq!(
        stored_token(&mut conn, marker),
        (false, None, "character".into())
    );
}

#[tokio::test]
async fn a_token_with_no_actor_cannot_be_placed_linked_nor_one_from_another_world() {
    let state = test_app_state();
    let (t, stranger) = {
        let mut conn = state.db_pool.get().expect("conn");
        let t = table(&mut conn);
        let elsewhere = table(&mut conn);
        let stranger = creature(
            &mut conn,
            &elsewhere,
            "Elsewhere",
            true,
            json!({ "max_hp": 3 }),
        );
        (t, stranger)
    };
    assert!(
        create_token_impl(&state, t.gm, false, input(&t, None, Some(true)))
            .await
            .is_err()
    );
    assert!(
        create_token_impl(&state, t.gm, false, input(&t, Some(stranger), None))
            .await
            .is_err(),
        "an actor from another world is not copied onto this board"
    );
}

/// SC-008: copies of one NPC take damage separately, and none of it reaches
/// the NPC.
#[tokio::test]
async fn damage_to_a_copy_changes_that_copy_alone_and_is_announced_as_a_token_change() {
    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let goblin = {
        let mut conn = state.db_pool.get().expect("conn");
        creature(
            &mut conn,
            &t,
            "Goblin",
            true,
            json!({ "current_hp": 7, "max_hp": 7 }),
        )
    };
    let a = place(&state, &t, Some(goblin), None).await;
    let b = place(&state, &t, Some(goblin), None).await;

    let mut conn = state.db_pool.get().expect("conn");
    let before_events: i64 = world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .filter(world_events::event_code.eq(EVENT_CODE_ACTOR_SHEET_CHANGED))
        .count()
        .get_result(&mut conn)
        .expect("count");

    assert_eq!(damage(&mut conn, &t, a, 5), 2);
    assert_eq!(
        stored_token(&mut conn, a).1,
        Some(json!({ "current_hp": 2, "max_hp": 7 }))
    );
    assert_eq!(
        stored_token(&mut conn, b).1,
        Some(json!({ "current_hp": 7, "max_hp": 7 }))
    );
    assert_eq!(actor_data(&mut conn, goblin)["current_hp"], json!(7));

    let token_event = world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .filter(world_events::event_code.eq(EVENT_CODE_TOKEN_CHANGED))
        .order(world_events::id.desc())
        .select(world_events::token_event)
        .first::<Option<serde_json::Value>>(&mut conn)
        .expect("event 14")
        .expect("payload");
    assert_eq!(token_event["token_id"], json!(a));
    let after_events: i64 = world_events::table
        .filter(world_events::world_id.eq(t.world_id))
        .filter(world_events::event_code.eq(EVENT_CODE_ACTOR_SHEET_CHANGED))
        .count()
        .get_result(&mut conn)
        .expect("count");
    assert_eq!(
        before_events, after_events,
        "a copy's hit is not a sheet change"
    );
}

#[tokio::test]
async fn damage_to_a_linked_token_changes_its_actors_sheet() {
    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let aria = {
        let mut conn = state.db_pool.get().expect("conn");
        creature(
            &mut conn,
            &t,
            "Aria",
            false,
            json!({ "current_hp": 12, "max_hp": 12 }),
        )
    };
    let token = place(&state, &t, Some(aria), None).await;
    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(damage(&mut conn, &t, token, 4), 8);
    assert_eq!(actor_data(&mut conn, aria)["current_hp"], json!(8));
    assert_eq!(stored_token(&mut conn, token).1, None);
}

/// The risk Phase 3 named: a copy's zero must never fall back to the actor.
/// Three combatants share the goblin NPC — two copies and one with no token.
/// One copy dropping marks out that copy and nobody else.
#[tokio::test]
async fn a_copy_at_zero_marks_out_its_own_combatant_and_no_other_sharing_its_npc() {
    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let goblin = {
        let mut conn = state.db_pool.get().expect("conn");
        creature(
            &mut conn,
            &t,
            "Goblin",
            true,
            json!({ "current_hp": 7, "max_hp": 7 }),
        )
    };
    let a = place(&state, &t, Some(goblin), None).await;
    let b = place(&state, &t, Some(goblin), None).await;

    let mut conn = state.db_pool.get().expect("conn");
    let combat_id = Uuid::now_v7();
    diesel::insert_into(world_combats::table)
        .values(&crate::models::NewCombat {
            id: combat_id,
            world_id: t.world_id,
            scene_id: Some(t.scene_id),
            created_by: t.gm,
        })
        .execute(&mut conn)
        .expect("combat");
    let mut combatant = |token_id: Option<Uuid>, label: &str| {
        let id = Uuid::now_v7();
        diesel::insert_into(world_combatants::table)
            .values(&crate::models::NewCombatant {
                id,
                combat_id,
                actor_id: Some(goblin),
                token_id,
                label: label.into(),
                initiative: 10,
                tiebreak: 0,
                is_npc: true,
            })
            .execute(&mut conn)
            .expect("combatant");
        id
    };
    let ca = combatant(Some(a), "Goblin A");
    let cb = combatant(Some(b), "Goblin B");
    let tokenless = combatant(None, "Goblin (no token)");

    assert_eq!(damage(&mut conn, &t, a, 7), 0);

    let active = |conn: &mut PgConnection, id: Uuid| -> bool {
        world_combatants::table
            .filter(world_combatants::id.eq(id))
            .select(world_combatants::active)
            .first(conn)
            .expect("combatant")
    };
    assert!(!active(&mut conn, ca), "goblin A is out");
    assert!(active(&mut conn, cb), "goblin B is still in");
    assert!(
        active(&mut conn, tokenless),
        "a tokenless combatant sharing the NPC is not the copy that dropped"
    );
}

#[tokio::test]
async fn relinking_discards_the_copy_and_unlinking_seeds_one() {
    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let goblin = {
        let mut conn = state.db_pool.get().expect("conn");
        creature(
            &mut conn,
            &t,
            "Goblin",
            true,
            json!({ "current_hp": 7, "max_hp": 7 }),
        )
    };
    let a = place(&state, &t, Some(goblin), None).await;
    {
        let mut conn = state.db_pool.get().expect("conn");
        damage(&mut conn, &t, a, 5);
    }

    // Unlinking what is already a copy changes nothing — above all, it does
    // not re-seed and quietly heal it.
    set_token_link_impl(&state, t.gm, false, a, false)
        .await
        .expect("no-op");
    {
        let mut conn = state.db_pool.get().expect("conn");
        assert_eq!(
            stored_token(&mut conn, a).1.unwrap()["current_hp"],
            json!(2)
        );
    }

    set_token_link_impl(&state, t.gm, false, a, true)
        .await
        .expect("relinked");
    {
        let mut conn = state.db_pool.get().expect("conn");
        assert!(stored_token(&mut conn, a).0);
        assert_eq!(
            stored_token(&mut conn, a).1,
            None,
            "the copy's record is gone"
        );
        // Its hit points are now the NPC's, and damage lands there.
        assert_eq!(damage(&mut conn, &t, a, 1), 6);
        assert_eq!(actor_data(&mut conn, goblin)["current_hp"], json!(6));
    }

    set_token_link_impl(&state, t.gm, false, a, false)
        .await
        .expect("unlinked");
    let mut conn = state.db_pool.get().expect("conn");
    assert_eq!(
        stored_token(&mut conn, a).1,
        Some(json!({ "current_hp": 6, "max_hp": 7 })),
        "unlinking starts the copy from the actor as it stands"
    );
}

#[tokio::test]
async fn a_player_may_neither_relink_a_token_nor_mark_an_npc_unique() {
    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let goblin = {
        let mut conn = state.db_pool.get().expect("conn");
        creature(
            &mut conn,
            &t,
            "Goblin",
            true,
            json!({ "current_hp": 7, "max_hp": 7 }),
        )
    };
    let a = place(&state, &t, Some(goblin), None).await;

    let refused = set_token_link_impl(&state, t.player, false, a, true)
        .await
        .expect_err("refused");
    assert!(
        refused.message.contains("Game Master"),
        "{}",
        refused.message
    );
    let refused = set_actor_unique_impl(&state, t.player, false, goblin, true)
        .await
        .expect_err("refused");
    assert!(
        refused.message.contains("Game Master"),
        "{}",
        refused.message
    );

    let mut conn = state.db_pool.get().expect("conn");
    assert!(!stored_token(&mut conn, a).0);
    let unique: bool = world_actors::table
        .filter(world_actors::id.eq(goblin))
        .select(world_actors::is_unique)
        .first(&mut conn)
        .expect("actor");
    assert!(!unique);
}

/// T040 (research R7's known gap): a claimed character arrives as its
/// player's token, not the Game Master's.
#[tokio::test]
async fn a_claimed_character_arrives_owned_by_the_player_who_claimed_it() {
    use crate::graphql::mutations_party::{BringPartyToSceneInput, bring_party_to_scene_impl};
    use crate::schema::{world_actor_claims, world_members};

    let state = test_app_state();
    let t = {
        let mut conn = state.db_pool.get().expect("conn");
        table(&mut conn)
    };
    let aria = {
        let mut conn = state.db_pool.get().expect("conn");
        let aria = creature(
            &mut conn,
            &t,
            "Aria",
            false,
            json!({ "current_hp": 12, "max_hp": 12 }),
        );
        let member_id: Uuid = world_members::table
            .filter(world_members::world_id.eq(t.world_id))
            .filter(world_members::user_id.eq(t.player))
            .select(world_members::id)
            .first(&mut conn)
            .expect("member");
        diesel::insert_into(world_actor_claims::table)
            .values((
                world_actor_claims::id.eq(Uuid::now_v7()),
                world_actor_claims::actor_id.eq(aria),
                world_actor_claims::world_member_id.eq(member_id),
            ))
            .execute(&mut conn)
            .expect("claim");
        aria
    };

    bring_party_to_scene_impl(
        &state,
        t.gm,
        false,
        BringPartyToSceneInput {
            scene_id: t.scene_id,
            actor_ids: Some(vec![aria]),
        },
    )
    .await
    .expect("the party arrives");

    let mut conn = state.db_pool.get().expect("conn");
    let (owner, linked): (Option<Uuid>, bool) = tokens::table
        .filter(tokens::scene_id.eq(t.scene_id))
        .filter(tokens::actor_id.eq(aria))
        .select((tokens::owner_user_id, tokens::linked))
        .first(&mut conn)
        .expect("token");
    assert_eq!(owner, Some(t.player));
    assert!(linked);
}
