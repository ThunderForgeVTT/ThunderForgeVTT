//! Spec 044 T071: one test per clause of B6 (contracts §5).
//!
//! Every scenario asks the gate twice: through `may_change_actor_imagery`,
//! which is what `myMayChangeImagery` reports, and through a real image
//! mutation. The two answering differently is the bug a client-side button
//! would hide, so `agrees` checks them against each other.

use super::*;
use crate::graphql::mutations_actor_claims::{create_and_claim_actor_impl, unclaim_actor_impl};
use crate::graphql::mutations_actor_images::{
    ROLE_PORTRAIT, ROLE_TOKEN, remove_actor_image_impl, upload_actor_image_impl,
};
use crate::graphql::mutations_actors::{
    UpdateActorInput, set_actor_art_locked_impl, update_actor_impl,
};
use crate::graphql::mutations_worlds::{
    UpdateWorldAllowPlayerActorArtInput, update_world_allow_player_actor_art_impl,
};
use crate::schema::world_actor_images;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state, tiny_png_bytes,
};

struct Table {
    gm: Uuid,
    world: Uuid,
    scene: Uuid,
}

fn a_table(state: &AppState) -> Table {
    let mut conn = state.db_pool.get().unwrap();
    let gm = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, gm);
    let scene = insert_test_scene(&mut conn, world, gm);
    Table { gm, world, scene }
}

/// A player in the table's world holding a character of their own.
fn a_player_holding(state: &AppState, t: &Table, label: &str) -> (Uuid, Uuid) {
    let mut conn = state.db_pool.get().unwrap();
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, t.world, player, "Player");
    let member: Uuid = world_members::table
        .filter(world_members::world_id.eq(t.world))
        .filter(world_members::user_id.eq(player))
        .select(world_members::id)
        .first(&mut conn)
        .unwrap();
    let actor = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(actor),
            world_actors::world_id.eq(t.world),
            world_actors::scene_id.eq(t.scene),
            world_actors::actor_type.eq("character"),
            world_actors::game_system_id.eq("generic"),
            world_actors::label.eq(label),
            world_actors::created_by.eq(t.gm),
            world_actors::owned_by.eq(t.gm),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(false),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .unwrap();
    diesel::insert_into(world_actor_claims::table)
        .values((
            world_actor_claims::id.eq(Uuid::now_v7()),
            world_actor_claims::actor_id.eq(actor),
            world_actor_claims::world_member_id.eq(member),
        ))
        .execute(&mut conn)
        .unwrap();
    (player, actor)
}

async fn gate(state: &AppState, user: Uuid, actor: Uuid) -> Result<(), ImageryRefusal> {
    may_change_actor_imagery(state, user, false, actor)
        .await
        .expect("the gate answers")
}

/// The gate and a real image mutation give the same answer, and a refusal
/// arrives with the refusal's own words.
async fn agrees(state: &AppState, user: Uuid, actor: Uuid) -> Result<(), ImageryRefusal> {
    let expected = gate(state, user, actor).await;
    let removed = remove_actor_image_impl(state, user, false, actor, ROLE_TOKEN.into()).await;
    match expected {
        Ok(()) => assert!(removed.is_ok(), "the gate allowed what removal refused"),
        Err(refusal) => {
            let err = removed.expect_err("the gate refused what removal allowed");
            assert_eq!(err.message, refusal.message());
        }
    }
    expected
}

async fn set_world(state: &AppState, t: &Table, allow: bool) {
    update_world_allow_player_actor_art_impl(
        state,
        t.gm,
        false,
        UpdateWorldAllowPlayerActorArtInput {
            world_id: t.world,
            allow,
        },
    )
    .await
    .expect("the Game Master sets the world");
}

fn portraits_of(state: &AppState, actor: Uuid) -> i64 {
    let mut conn = state.db_pool.get().unwrap();
    world_actor_images::table
        .filter(world_actor_images::actor_id.eq(actor))
        .filter(world_actor_images::role.eq(ROLE_PORTRAIT))
        .count()
        .get_result(&mut conn)
        .unwrap()
}

/// FR-030: the holder may, and another player in the same world may not.
#[tokio::test]
async fn a_holder_may_and_another_player_may_not() {
    let state = test_app_state();
    let t = a_table(&state);
    let (holder, actor) = a_player_holding(&state, &t, "Mirela");
    let (other, _) = a_player_holding(&state, &t, "Tobin");

    assert_eq!(agrees(&state, holder, actor).await, Ok(()));
    assert_eq!(
        agrees(&state, other, actor).await,
        Err(ImageryRefusal::NotHolder)
    );

    upload_actor_image_impl(
        &state,
        holder,
        false,
        actor,
        ROLE_PORTRAIT.into(),
        tiny_png_bytes(),
    )
    .await
    .expect("the holder uploads a portrait");
    assert_eq!(portraits_of(&state, actor), 1);
}

/// A player who created their own character holds it, and stops holding it
/// when the claim ends.
#[tokio::test]
async fn a_created_character_is_held_until_released() {
    let state = test_app_state();
    let t = a_table(&state);
    let player = {
        let mut conn = state.db_pool.get().unwrap();
        diesel::update(worlds::table.filter(worlds::id.eq(t.world)))
            .set(worlds::allow_player_created_actors.eq(true))
            .execute(&mut conn)
            .unwrap();
        let player = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, t.world, player, "Player");
        player
    };
    let claim = create_and_claim_actor_impl(&state, player, t.world, "Wren".into(), None)
        .await
        .expect("the player creates a character");
    let actor = claim.actor_id;

    assert_eq!(agrees(&state, player, actor).await, Ok(()));

    unclaim_actor_impl(&state, t.gm, false, actor, None)
        .await
        .expect("the Game Master releases the character");
    assert_eq!(
        agrees(&state, player, actor).await,
        Err(ImageryRefusal::NotHolder)
    );
}

/// The grant is imagery only: the holder still may not rename the character.
#[tokio::test]
async fn the_grant_reaches_no_other_actor_mutation() {
    let state = test_app_state();
    let t = a_table(&state);
    let (holder, actor) = a_player_holding(&state, &t, "Mirela");

    assert_eq!(gate(&state, holder, actor).await, Ok(()));
    let renamed = update_actor_impl(
        &state,
        holder,
        false,
        UpdateActorInput {
            actor_id: actor,
            label: Some("Renamed".into()),
            is_npc: None,
            actor_type: None,
            description: None,
        },
    )
    .await;
    assert!(renamed.is_err(), "the holder may not rename the character");
}

/// FR-030a: the world setting off refuses every holder in the world, and
/// turning it back on restores them.
#[tokio::test]
async fn the_world_setting_off_refuses_every_actor() {
    let state = test_app_state();
    let t = a_table(&state);
    let (first, a) = a_player_holding(&state, &t, "Mirela");
    let (second, b) = a_player_holding(&state, &t, "Tobin");

    set_world(&state, &t, false).await;
    assert_eq!(
        agrees(&state, first, a).await,
        Err(ImageryRefusal::WorldSettingOff)
    );
    assert_eq!(
        agrees(&state, second, b).await,
        Err(ImageryRefusal::WorldSettingOff)
    );

    set_world(&state, &t, true).await;
    assert_eq!(agrees(&state, first, a).await, Ok(()));
}

/// FR-030b: a lock refuses that character only, whatever the setting.
#[tokio::test]
async fn a_lock_refuses_that_actor_only() {
    let state = test_app_state();
    let t = a_table(&state);
    let (first, a) = a_player_holding(&state, &t, "Mirela");
    let (second, b) = a_player_holding(&state, &t, "Tobin");

    let locked = set_actor_art_locked_impl(&state, t.gm, false, a, true)
        .await
        .expect("the Game Master locks a look");
    assert!(locked.art_locked);

    assert_eq!(agrees(&state, first, a).await, Err(ImageryRefusal::Locked));
    assert_eq!(agrees(&state, second, b).await, Ok(()));

    // The lock is reported even when the world setting is also off.
    set_world(&state, &t, false).await;
    assert_eq!(gate(&state, first, a).await, Err(ImageryRefusal::Locked));
}

/// FR-032: neither switch touches the Game Master, who may replace a
/// player's art; and only the Game Master may flip either switch.
#[tokio::test]
async fn the_game_master_is_refused_by_neither() {
    let state = test_app_state();
    let t = a_table(&state);
    let (holder, actor) = a_player_holding(&state, &t, "Mirela");

    upload_actor_image_impl(
        &state,
        holder,
        false,
        actor,
        ROLE_PORTRAIT.into(),
        tiny_png_bytes(),
    )
    .await
    .expect("the holder uploads a portrait");

    set_world(&state, &t, false).await;
    set_actor_art_locked_impl(&state, t.gm, false, actor, true)
        .await
        .unwrap();
    assert_eq!(agrees(&state, t.gm, actor).await, Ok(()));
    upload_actor_image_impl(
        &state,
        t.gm,
        false,
        actor,
        ROLE_PORTRAIT.into(),
        tiny_png_bytes(),
    )
    .await
    .expect("the Game Master replaces the player's portrait");
    assert_eq!(portraits_of(&state, actor), 1, "replaced, not added");

    assert!(
        set_actor_art_locked_impl(&state, holder, false, actor, false)
            .await
            .is_err(),
        "a player may not unlock their own look"
    );
    assert!(
        update_world_allow_player_actor_art_impl(
            &state,
            holder,
            false,
            UpdateWorldAllowPlayerActorArtInput {
                world_id: t.world,
                allow: true,
            },
        )
        .await
        .is_err(),
        "a player may not turn the world setting back on"
    );
}

/// SC-010: a refused player is told which rule stopped them.
#[test]
fn the_three_refusals_say_different_things() {
    let all = [
        ImageryRefusal::NotHolder,
        ImageryRefusal::WorldSettingOff,
        ImageryRefusal::Locked,
    ];
    let messages: std::collections::HashSet<_> = all.iter().map(|r| r.message()).collect();
    let reasons: std::collections::HashSet<_> = all.iter().map(|r| r.reason()).collect();
    assert_eq!(messages.len(), 3);
    assert_eq!(reasons.len(), 3);
}
