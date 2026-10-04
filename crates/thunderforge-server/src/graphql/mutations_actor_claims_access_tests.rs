use super::tests::{
    claimants_of, error_code, insert_test_pc, mark_available, member_id_of,
    set_allow_player_created,
};
use super::*;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

// ===== Spec 063: a claim grants Editor, and a release takes it back =====
//
// Every test below reads the grant row and the resolved level, not what a
// mutation returned. "The player may edit" is a statement about what the
// permission resolver answers, and the provenance flag is only visible on
// the row; a returned claim says nothing about either.

use crate::auth::actor_permissions::effective_actor_permission;
use crate::graphql::mutations_actor_permissions::{
    SetActorPermissionInput, remove_actor_permission_impl, set_actor_permission_impl,
};
use crate::graphql::mutations_actors::{UpdateActorInput, update_actor_impl};
use crate::schema::world_events;
use crate::world_events::EVENT_CODE_ACTOR_ACCESS_CHANGED;

/// One world, its Game Master, one offered character, one player.
struct Stage {
    owner_id: Uuid,
    world_id: Uuid,
    scene_id: Uuid,
    actor_id: Uuid,
    player_id: Uuid,
    member_id: Uuid,
}

fn a_stage(state: &AppState) -> Stage {
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    let member_id = member_id_of(&mut conn, world_id, player_id);
    Stage {
        owner_id,
        world_id,
        scene_id,
        actor_id,
        player_id,
        member_id,
    }
}

/// The explicit grant `user_id` holds on `actor_id`, as `(level, came from
/// a claim)`. `None` is no row, which resolves to Viewer.
fn grant_of(state: &AppState, actor_id: Uuid, user_id: Uuid) -> Option<(String, bool)> {
    let mut conn = state.db_pool.get().unwrap();
    world_actor_permissions::table
        .filter(world_actor_permissions::actor_id.eq(actor_id))
        .filter(world_actor_permissions::user_id.eq(user_id))
        .select((
            world_actor_permissions::level,
            world_actor_permissions::granted_by_claim,
        ))
        .first::<(String, bool)>(&mut conn)
        .optional()
        .expect("failed to read the grant")
}

fn claim_grant() -> Option<(String, bool)> {
    Some(("Editor".to_string(), true))
}

fn hand_grant(level: &str) -> Option<(String, bool)> {
    Some((level.to_string(), false))
}

async fn level_of(state: &AppState, user_id: Uuid, actor_id: Uuid) -> ActorPermissionLevel {
    effective_actor_permission(state, user_id, false, actor_id)
        .await
        .expect("the level resolves")
}

async fn grant_by_hand(state: &AppState, stage: &Stage, level: ActorPermissionLevel) {
    set_actor_permission_impl(
        state,
        stage.owner_id,
        false,
        SetActorPermissionInput {
            actor_id: stage.actor_id,
            user_id: stage.player_id,
            level,
        },
    )
    .await
    .expect("the Game Master sets access by hand");
}

async fn release(state: &AppState, stage: &Stage) {
    unclaim_actor_impl(state, stage.owner_id, false, stage.actor_id, None)
        .await
        .expect("the Game Master releases the character");
}

/// A write that asks the ladder for Editor, made as `user_id`.
async fn may_write(state: &AppState, user_id: Uuid, actor_id: Uuid) -> bool {
    update_actor_impl(
        state,
        user_id,
        false,
        UpdateActorInput {
            actor_id,
            label: None,
            is_npc: None,
            actor_type: None,
            description: Some("Scarred by the crypt.".into()),
        },
    )
    .await
    .is_ok()
}

/// The access-changed events recorded for `actor_id`, as their payloads.
fn access_events(state: &AppState, world_id: Uuid, actor_id: Uuid) -> Vec<serde_json::Value> {
    let mut conn = state.db_pool.get().unwrap();
    world_events::table
        .filter(world_events::world_id.eq(world_id))
        .filter(world_events::event_code.eq(EVENT_CODE_ACTOR_ACCESS_CHANGED))
        .order(world_events::id.asc())
        .select(world_events::token_event)
        .load::<Option<serde_json::Value>>(&mut conn)
        .expect("failed to read world events")
        .into_iter()
        .flatten()
        .filter(|payload| payload["actorId"] == serde_json::json!(actor_id))
        .collect()
}

/// FR-001, FR-004, FR-006, FR-007: the claim grants Editor, the row says a
/// claim made it, and a write made with no interface in between is accepted.
#[tokio::test]
async fn claiming_grants_editor_the_claim_owns() {
    let state = test_app_state();
    let stage = a_stage(&state);

    assert_eq!(
        level_of(&state, stage.player_id, stage.actor_id).await,
        ActorPermissionLevel::Viewer
    );
    assert!(!may_write(&state, stage.player_id, stage.actor_id).await);

    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("claim should succeed");

    assert_eq!(
        grant_of(&state, stage.actor_id, stage.player_id),
        claim_grant()
    );
    assert_eq!(
        level_of(&state, stage.player_id, stage.actor_id).await,
        ActorPermissionLevel::Editor,
        "Editor, not Owner: enough to play it, short of handing it on"
    );
    assert!(
        may_write(&state, stage.player_id, stage.actor_id).await,
        "a claimed character must accept its player's writes"
    );
}

/// FR-003: bound from the players section, the player holds exactly what
/// they would have by claiming.
#[tokio::test]
async fn a_binding_from_the_roster_grants_what_a_claim_grants() {
    let state = test_app_state();
    let stage = a_stage(&state);

    set_player_character_binding_impl(
        &state,
        stage.owner_id,
        false,
        stage.world_id,
        stage.member_id,
        Some(stage.actor_id),
    )
    .await
    .expect("the Game Master binds the player");

    assert_eq!(
        grant_of(&state, stage.actor_id, stage.player_id),
        claim_grant()
    );
    assert!(may_write(&state, stage.player_id, stage.actor_id).await);
}

/// FR-003: created and claimed in one step, the character is editable at
/// once.
#[tokio::test]
async fn a_created_character_is_editable_by_its_creator() {
    let state = test_app_state();
    let stage = a_stage(&state);
    set_allow_player_created(&mut state.db_pool.get().unwrap(), stage.world_id, true);

    let claim = create_and_claim_actor_impl(
        &state,
        stage.player_id,
        stage.world_id,
        "Wren".to_string(),
        None,
    )
    .await
    .expect("the player creates a character");

    assert_eq!(
        grant_of(&state, claim.actor_id, stage.player_id),
        claim_grant()
    );
    assert!(may_write(&state, stage.player_id, claim.actor_id).await);
}

/// A character a player makes for themselves plays the world's game. It
/// used to be made as "generic", which no pack declares, so in a Roll for
/// Shoes world it had no sheet and refused every write as a system mismatch.
#[tokio::test]
async fn a_created_character_takes_its_worlds_system() {
    let state = test_app_state();
    let stage = a_stage(&state);
    let mut conn = state.db_pool.get().unwrap();
    set_allow_player_created(&mut conn, stage.world_id, true);
    diesel::update(worlds::table.filter(worlds::id.eq(stage.world_id)))
        .set(worlds::game_system_id.eq("roll_for_shoes"))
        .execute(&mut conn)
        .unwrap();

    let claim = create_and_claim_actor_impl(
        &state,
        stage.player_id,
        stage.world_id,
        "Wren".to_string(),
        None,
    )
    .await
    .expect("the player creates a character");

    let system: Option<String> = world_actors::table
        .filter(world_actors::id.eq(claim.actor_id))
        .select(world_actors::game_system_id)
        .first(&mut conn)
        .unwrap();
    assert_eq!(system.as_deref(), Some("roll_for_shoes"));
}

/// FR-002 and the "refused claim" edge case: a claim that is refused grants
/// nothing, whether it lost to another player or was never allowed.
#[tokio::test]
async fn a_refused_claim_grants_nothing() {
    let state = test_app_state();
    let stage = a_stage(&state);
    let (late_id, hidden_actor_id) = {
        let mut conn = state.db_pool.get().unwrap();
        let late_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, stage.world_id, late_id, "Player");
        // Never marked available.
        let hidden = insert_test_pc(
            &mut conn,
            stage.world_id,
            stage.scene_id,
            stage.owner_id,
            "Brannoc",
        );
        (late_id, hidden)
    };

    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("the first claim succeeds");

    let lost = claim_actor_impl(&state, late_id, stage.world_id, stage.actor_id)
        .await
        .expect_err("the character is already played");
    assert!(error_code(&lost).contains(ALREADY_CLAIMED));
    assert_eq!(grant_of(&state, stage.actor_id, late_id), None);
    assert!(!may_write(&state, late_id, stage.actor_id).await);

    claim_actor_impl(&state, late_id, stage.world_id, hidden_actor_id)
        .await
        .expect_err("a character not offered may not be claimed");
    assert_eq!(grant_of(&state, hidden_actor_id, late_id), None);
}

/// FR-012, FR-015, User Story 3 scenarios 1 and 5: the release removes what
/// the claim gave, and the next claimant is granted afresh.
#[tokio::test]
async fn releasing_takes_back_what_the_claim_gave() {
    let state = test_app_state();
    let stage = a_stage(&state);
    let next_id = {
        let mut conn = state.db_pool.get().unwrap();
        let next_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, stage.world_id, next_id, "Player");
        next_id
    };

    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("claim should succeed");
    release(&state, &stage).await;

    assert_eq!(grant_of(&state, stage.actor_id, stage.player_id), None);
    assert_eq!(
        level_of(&state, stage.player_id, stage.actor_id).await,
        level_of(&state, next_id, stage.actor_id).await,
        "a released player holds what a member who never claimed it holds"
    );
    assert!(!may_write(&state, stage.player_id, stage.actor_id).await);

    claim_actor_impl(&state, next_id, stage.world_id, stage.actor_id)
        .await
        .expect("the released character may be claimed again");
    assert_eq!(grant_of(&state, stage.actor_id, next_id), claim_grant());
    assert_eq!(
        grant_of(&state, stage.actor_id, stage.player_id),
        None,
        "the new claim grants its own player and nobody else"
    );
}

/// FR-005, FR-013, User Story 3 scenario 2: Editor or Owner granted by hand
/// before the claim is left exactly as it was by the claim, and by the
/// release.
#[tokio::test]
async fn a_hand_grant_made_before_the_claim_survives_the_release() {
    for level in [ActorPermissionLevel::Editor, ActorPermissionLevel::Owner] {
        let state = test_app_state();
        let stage = a_stage(&state);
        grant_by_hand(&state, &stage, level).await;

        claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
            .await
            .expect("claim should succeed");
        assert_eq!(
            grant_of(&state, stage.actor_id, stage.player_id),
            hand_grant(level.as_db_str()),
            "the claim must not take a hand grant over"
        );

        release(&state, &stage).await;
        assert_eq!(
            grant_of(&state, stage.actor_id, stage.player_id),
            hand_grant(level.as_db_str()),
            "the release must not sweep up a hand grant"
        );
    }
}

/// The "hand grant already exists" edge case, below Editor: the claim raises
/// an explicit Viewer, the raise is the claim's, and the release leaves the
/// player able to view — which is what explicit Viewer gave them.
#[tokio::test]
async fn a_claim_raises_an_explicit_viewer_and_the_release_takes_the_raise_back() {
    let state = test_app_state();
    let stage = a_stage(&state);
    grant_by_hand(&state, &stage, ActorPermissionLevel::Viewer).await;

    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("claim should succeed");
    assert_eq!(
        grant_of(&state, stage.actor_id, stage.player_id),
        claim_grant()
    );

    release(&state, &stage).await;
    assert_eq!(grant_of(&state, stage.actor_id, stage.player_id), None);
    assert_eq!(
        level_of(&state, stage.player_id, stage.actor_id).await,
        ActorPermissionLevel::Viewer
    );
}

/// FR-008, FR-018, User Story 3 scenarios 3 and 4: whatever the Game Master
/// sets while the claim is live is what stands, and the release leaves it
/// alone — raised, lowered, re-set at the same level, or removed.
#[tokio::test]
async fn what_the_game_master_sets_during_a_claim_outlives_it() {
    for level in [
        ActorPermissionLevel::Owner,
        ActorPermissionLevel::Editor,
        ActorPermissionLevel::Viewer,
    ] {
        let state = test_app_state();
        let stage = a_stage(&state);
        claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
            .await
            .expect("claim should succeed");

        grant_by_hand(&state, &stage, level).await;
        assert_eq!(
            grant_of(&state, stage.actor_id, stage.player_id),
            hand_grant(level.as_db_str()),
            "a hand edit takes the decision over from the claim"
        );

        release(&state, &stage).await;
        assert_eq!(
            grant_of(&state, stage.actor_id, stage.player_id),
            hand_grant(level.as_db_str()),
            "the release changes nothing the Game Master set"
        );
    }

    // Removed outright: the claim does not put it back, and the release has
    // nothing to take.
    let state = test_app_state();
    let stage = a_stage(&state);
    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("claim should succeed");
    let removed = remove_actor_permission_impl(
        &state,
        stage.owner_id,
        false,
        stage.actor_id,
        stage.player_id,
    )
    .await
    .expect("the Game Master removes the player's access");
    assert!(removed);
    assert!(
        !may_write(&state, stage.player_id, stage.actor_id).await,
        "a claim sets a floor on the day it is made; it does not re-assert itself"
    );
    assert_eq!(
        claimants_of(&mut state.db_pool.get().unwrap(), stage.actor_id),
        vec![stage.member_id],
        "removing access is not a release"
    );

    release(&state, &stage).await;
    assert_eq!(grant_of(&state, stage.actor_id, stage.player_id), None);
}

/// FR-012 by the players section's route: re-binding takes the old
/// character's access with its claim and grants the new one; clearing the
/// binding takes it and grants nothing.
#[tokio::test]
async fn rebinding_moves_the_access_with_the_claim() {
    let state = test_app_state();
    let stage = a_stage(&state);
    let second_actor_id = insert_test_pc(
        &mut state.db_pool.get().unwrap(),
        stage.world_id,
        stage.scene_id,
        stage.owner_id,
        "Brannoc",
    );
    let bind = |actor_id: Option<Uuid>| {
        set_player_character_binding_impl(
            &state,
            stage.owner_id,
            false,
            stage.world_id,
            stage.member_id,
            actor_id,
        )
    };

    bind(Some(stage.actor_id)).await.expect("first binding");
    bind(Some(second_actor_id)).await.expect("re-binding");
    assert_eq!(grant_of(&state, stage.actor_id, stage.player_id), None);
    assert_eq!(
        grant_of(&state, second_actor_id, stage.player_id),
        claim_grant()
    );

    // Bound again to the character they already have: still one grant, still
    // the claim's.
    bind(Some(second_actor_id)).await.expect("the same binding");
    assert_eq!(
        grant_of(&state, second_actor_id, stage.player_id),
        claim_grant()
    );

    bind(None).await.expect("clearing the binding");
    assert_eq!(grant_of(&state, second_actor_id, stage.player_id), None);
}

/// A re-binding that is refused rolls back the release it began with, and
/// the access goes back with the claim: the player is left on the character
/// they had, still able to write to it, and holds nothing on the one they
/// were refused.
#[tokio::test]
async fn a_refused_rebinding_leaves_the_access_where_it_was() {
    let state = test_app_state();
    let stage = a_stage(&state);
    let (taken_actor_id, other_member_id) = {
        let mut conn = state.db_pool.get().unwrap();
        let taken = insert_test_pc(
            &mut conn,
            stage.world_id,
            stage.scene_id,
            stage.owner_id,
            "Brannoc",
        );
        let other_id = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, stage.world_id, other_id, "Player");
        (taken, member_id_of(&mut conn, stage.world_id, other_id))
    };
    for (member_id, actor_id) in [
        (stage.member_id, stage.actor_id),
        (other_member_id, taken_actor_id),
    ] {
        set_player_character_binding_impl(
            &state,
            stage.owner_id,
            false,
            stage.world_id,
            member_id,
            Some(actor_id),
        )
        .await
        .expect("binding should succeed");
    }

    set_player_character_binding_impl(
        &state,
        stage.owner_id,
        false,
        stage.world_id,
        stage.member_id,
        Some(taken_actor_id),
    )
    .await
    .expect_err("a character somebody else plays is refused");

    assert_eq!(
        grant_of(&state, stage.actor_id, stage.player_id),
        claim_grant()
    );
    assert_eq!(grant_of(&state, taken_actor_id, stage.player_id), None);
}

/// A release refused as stale destroys neither the claim it never saw nor
/// the access that claim gave.
#[tokio::test]
async fn a_stale_release_takes_no_access() {
    let state = test_app_state();
    let stage = a_stage(&state);
    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("claim should succeed");

    let stale = unclaim_actor_impl(
        &state,
        stage.owner_id,
        false,
        stage.actor_id,
        Some(Uuid::now_v7()),
    )
    .await
    .expect_err("the claim named is not the claim on the row");
    assert!(error_code(&stale).contains(CLAIM_CHANGED));

    assert_eq!(
        grant_of(&state, stage.actor_id, stage.player_id),
        claim_grant()
    );
}

/// FR-016: being able to edit a character offers it to nobody. A co-Game-
/// Master granted Editor on a character that is not available cannot claim
/// it, and it appears on nobody's selection screen.
#[tokio::test]
async fn editor_does_not_make_a_character_claimable() {
    let state = test_app_state();
    let stage = a_stage(&state);
    mark_available(&mut state.db_pool.get().unwrap(), stage.actor_id, false);
    grant_by_hand(&state, &stage, ActorPermissionLevel::Editor).await;

    assert!(
        available_actors_impl(&state, stage.world_id)
            .await
            .unwrap()
            .is_empty()
    );
    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect_err("Editor on a character is not an offer of it");
}

/// The access change is announced by every route a claim can begin or end,
/// naming the character and nobody's user id: the event reaches every member
/// of the world, and who holds what is the Game Master's to read.
#[tokio::test]
async fn a_claim_and_its_release_each_announce_the_access_change() {
    let state = test_app_state();
    let stage = a_stage(&state);
    let events = || access_events(&state, stage.world_id, stage.actor_id);
    assert!(events().is_empty());

    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect("claim should succeed");
    assert_eq!(events().len(), 1);

    release(&state, &stage).await;
    assert_eq!(events().len(), 2);

    // Nothing was held, so nothing changed and nothing is announced.
    release(&state, &stage).await;
    assert_eq!(events().len(), 2);

    set_player_character_binding_impl(
        &state,
        stage.owner_id,
        false,
        stage.world_id,
        stage.member_id,
        Some(stage.actor_id),
    )
    .await
    .expect("binding should succeed");
    assert_eq!(events().len(), 3);

    set_player_character_binding_impl(
        &state,
        stage.owner_id,
        false,
        stage.world_id,
        stage.member_id,
        None,
    )
    .await
    .expect("clearing the binding should succeed");
    let all = events();
    assert_eq!(all.len(), 4);

    for payload in all {
        assert_eq!(
            payload,
            serde_json::json!({ "action": "changed", "actorId": stage.actor_id }),
            "the payload carries the character and nothing about who"
        );
    }
}

/// A refused claim announces nothing, as it grants nothing.
#[tokio::test]
async fn a_refused_claim_announces_nothing() {
    let state = test_app_state();
    let stage = a_stage(&state);
    mark_available(&mut state.db_pool.get().unwrap(), stage.actor_id, false);

    claim_actor_impl(&state, stage.player_id, stage.world_id, stage.actor_id)
        .await
        .expect_err("the character is not offered");
    assert!(access_events(&state, stage.world_id, stage.actor_id).is_empty());
}
