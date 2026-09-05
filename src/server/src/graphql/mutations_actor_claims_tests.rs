use super::*;
use crate::test_support::{
    insert_test_scene, insert_test_user, insert_test_world, insert_test_world_member,
    test_app_state,
};

fn mark_available(conn: &mut PgConnection, actor_id: Uuid, available: bool) {
    diesel::update(world_actors::table.filter(world_actors::id.eq(actor_id)))
        .set(world_actors::available_for_claim.eq(available))
        .execute(conn)
        .expect("failed to mark actor availability");
}

fn insert_test_pc(
    conn: &mut PgConnection,
    world_id: Uuid,
    scene_id: Uuid,
    owner_id: Uuid,
    label: &str,
) -> Uuid {
    let id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(id),
            world_actors::world_id.eq(world_id),
            world_actors::scene_id.eq(scene_id),
            world_actors::actor_type.eq("character"),
            world_actors::game_system_id.eq("generic"),
            world_actors::label.eq(label),
            world_actors::created_by.eq(owner_id),
            world_actors::owned_by.eq(owner_id),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(false),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
        ))
        .execute(conn)
        .expect("failed to insert test PC actor");
    id
}

/// The member ids currently claiming `actor_id`. Spec 031 tests assert on
/// this rather than on what a mutation returned, because "exactly one
/// player got the character" is a statement about the table — the same
/// reason `mutations_pickup.rs` counts inventory rows.
fn claimants_of(conn: &mut PgConnection, actor_id: Uuid) -> Vec<Uuid> {
    world_actor_claims::table
        .filter(world_actor_claims::actor_id.eq(actor_id))
        .select(world_actor_claims::world_member_id)
        .load::<Uuid>(conn)
        .expect("failed to read claims")
}

/// The actor ids `member_id` is bound to. More than one is the other
/// half of FR-034 and would be just as much a bug as two claimants.
fn characters_of(conn: &mut PgConnection, member_id: Uuid) -> Vec<Uuid> {
    world_actor_claims::table
        .filter(world_actor_claims::world_member_id.eq(member_id))
        .select(world_actor_claims::actor_id)
        .load::<Uuid>(conn)
        .expect("failed to read claims")
}

fn member_id_of(conn: &mut PgConnection, world_id: Uuid, user_id: Uuid) -> Uuid {
    world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(user_id))
        .select(world_members::id)
        .first::<Uuid>(conn)
        .expect("failed to read world member")
}

fn error_code(error: &Error) -> String {
    error
        .extensions
        .as_ref()
        .and_then(|ext| ext.get("code"))
        .map(|v| format!("{v:?}"))
        .unwrap_or_default()
}

fn set_allow_player_created(conn: &mut PgConnection, world_id: Uuid, allow: bool) {
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::allow_player_created_actors.eq(allow))
        .execute(conn)
        .expect("failed to set allow_player_created_actors");
}

#[tokio::test]
async fn non_member_cannot_claim() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);
    let outsider_id = insert_test_user(&mut conn);
    drop(conn);

    let result = claim_actor_impl(&state, outsider_id, world_id, actor_id).await;
    assert!(
        result.is_err(),
        "a non-member must not be able to claim a character"
    );
}

#[tokio::test]
async fn gm_never_gated_myactorclaim_always_none() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    drop(conn);

    let claim = my_actor_claim_impl(&state, owner_id, world_id)
        .await
        .expect("query should succeed for the owner");
    assert!(
        claim.is_none(),
        "the GM/Owner must never be shown a claim gate"
    );
}

#[tokio::test]
async fn claiming_unavailable_actor_errors() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    // available_for_claim defaults to false — never marked available.
    let result = claim_actor_impl(&state, player_id, world_id, actor_id).await;
    assert!(result.is_err(), "an unflagged actor must not be claimable");
}

#[tokio::test]
async fn claim_succeeds_and_actor_disappears_from_available_list() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let before = available_actors_impl(&state, world_id).await.unwrap();
    assert_eq!(before.len(), 1);

    let claim = claim_actor_impl(&state, player_id, world_id, actor_id)
        .await
        .expect("claim should succeed");
    assert_eq!(claim.actor_id, actor_id);

    let after = available_actors_impl(&state, world_id).await.unwrap();
    assert!(
        after.is_empty(),
        "a claimed actor must disappear from the available list"
    );

    let my_claim = my_actor_claim_impl(&state, player_id, world_id)
        .await
        .unwrap();
    assert!(
        my_claim.is_some(),
        "the claiming player should now see their claim"
    );
}

// ===== Spec 023: claimed_actor_impl (the Players section's roster join) =====

#[tokio::test]
async fn claimed_actor_impl_returns_none_before_a_claim_and_the_actor_after() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    // Need the member's own `world_members.id` (not `user_id`) — fetch
    // it the same way `claimed_by_impl`'s reverse resolver does.
    let mut conn = state.db_pool.get().unwrap();
    let member_id: Uuid = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(player_id))
        .select(world_members::id)
        .first(&mut conn)
        .unwrap();
    drop(conn);

    let before = claimed_actor_impl(&state, member_id).await.unwrap();
    assert!(
        before.is_none(),
        "no claim yet — must be None, not an error"
    );

    let claim = claim_actor_impl(&state, player_id, world_id, actor_id)
        .await
        .expect("claim should succeed");
    assert_eq!(claim.world_member_id, member_id);

    let after = claimed_actor_impl(&state, member_id).await.unwrap();
    assert_eq!(
        after.map(|a| a.id),
        Some(actor_id),
        "after claiming, claimed_actor_impl must return that same actor"
    );
}

#[tokio::test]
async fn member_with_existing_claim_cannot_claim_second_actor() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let first_actor = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let second_actor = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Borin");
    mark_available(&mut conn, first_actor, true);
    mark_available(&mut conn, second_actor, true);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    claim_actor_impl(&state, player_id, world_id, first_actor)
        .await
        .expect("first claim should succeed");

    let result = claim_actor_impl(&state, player_id, world_id, second_actor).await;
    assert!(
        result.is_err(),
        "a member with an existing claim must not claim a second character"
    );
}

#[tokio::test]
async fn create_and_claim_rejected_when_setting_off() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_scene(&mut conn, world_id, owner_id);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let result = create_and_claim_actor_impl(
        &state,
        player_id,
        world_id,
        "Homebrew Hero".to_string(),
        None,
    )
    .await;
    assert!(
        result.is_err(),
        "creation must be rejected when the world setting is off"
    );
}

#[tokio::test]
async fn create_and_claim_succeeds_when_setting_on() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    insert_test_scene(&mut conn, world_id, owner_id);
    set_allow_player_created(&mut conn, world_id, true);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let claim = create_and_claim_actor_impl(
        &state,
        player_id,
        world_id,
        "Homebrew Hero".to_string(),
        None,
    )
    .await
    .expect("creation should succeed when the setting is on");

    let my_claim = my_actor_claim_impl(&state, player_id, world_id)
        .await
        .unwrap();
    assert_eq!(my_claim.unwrap().actor_id, claim.actor_id);
}

#[tokio::test]
async fn set_availability_rejects_non_owner() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    let result = set_actor_availability_impl(&state, player_id, false, actor_id, true).await;
    assert!(
        result.is_err(),
        "a non-Owner caller must not be able to set availability"
    );
}

#[tokio::test]
async fn set_availability_rejects_npc() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let npc_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_actors::table)
        .values((
            world_actors::id.eq(npc_id),
            world_actors::world_id.eq(world_id),
            world_actors::scene_id.eq(scene_id),
            world_actors::actor_type.eq("npc"),
            world_actors::game_system_id.eq("generic"),
            world_actors::label.eq("Goblin"),
            world_actors::created_by.eq(owner_id),
            world_actors::owned_by.eq(owner_id),
            world_actors::is_public.eq(false),
            world_actors::is_npc.eq(true),
            world_actors::created_at.eq(now),
            world_actors::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let result = set_actor_availability_impl(&state, owner_id, false, npc_id, true).await;
    assert!(
        result.is_err(),
        "an NPC-classified actor must not be markable as available"
    );
}

#[tokio::test]
async fn unclaim_makes_actor_available_again_without_reflagging() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    drop(conn);

    claim_actor_impl(&state, player_id, world_id, actor_id)
        .await
        .expect("claim should succeed");
    assert!(
        available_actors_impl(&state, world_id)
            .await
            .unwrap()
            .is_empty()
    );

    unclaim_actor_impl(&state, owner_id, false, actor_id, None)
        .await
        .expect("the DM should be able to unclaim");

    let available = available_actors_impl(&state, world_id).await.unwrap();
    assert_eq!(
        available.len(),
        1,
        "the actor should reappear as available without re-flagging"
    );

    let previous_claimant = my_actor_claim_impl(&state, player_id, world_id)
        .await
        .unwrap();
    assert!(
        previous_claimant.is_none(),
        "the previous claimant should return to the no-character-selected state"
    );

    // The un-claimed player's world_members row must remain untouched
    // (they stay a full world member, per FR-013).
    let still_member: bool = diesel::select(diesel::dsl::exists(
        world_members::table
            .filter(world_members::world_id.eq(world_id))
            .filter(world_members::user_id.eq(player_id)),
    ))
    .get_result::<bool>(&mut state.db_pool.get().unwrap())
    .unwrap();
    assert!(
        still_member,
        "un-claiming must not remove the player from the world"
    );
}

#[tokio::test]
async fn concurrent_claims_exactly_one_succeeds() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);
    let player_a = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_a, "Player");
    let player_b = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_b, "Player");
    drop(conn);

    let (result_a, result_b) = tokio::join!(
        claim_actor_impl(&state, player_a, world_id, actor_id),
        claim_actor_impl(&state, player_b, world_id, actor_id),
    );

    let successes = [result_a.is_ok(), result_b.is_ok()]
        .iter()
        .filter(|ok| **ok)
        .count();
    assert_eq!(
        successes, 1,
        "exactly one of two concurrent claims must succeed (FR-006/SC-003)"
    );

    // Sanity: the unique constraint is genuinely load-bearing, not
    // just the app-level pre-check — force a raw duplicate insert
    // past the app-level guard to confirm the DB itself rejects it.
    let mut conn = state.db_pool.get().unwrap();
    let member_id: Uuid = world_members::table
        .filter(world_members::world_id.eq(world_id))
        .filter(world_members::user_id.eq(player_a))
        .select(world_members::id)
        .first(&mut conn)
        .unwrap();
    let dup = diesel::sql_query(
        "INSERT INTO world_actor_claims (actor_id, world_member_id) VALUES ($1, $2)",
    )
    .bind::<diesel::sql_types::Uuid, _>(actor_id)
    .bind::<diesel::sql_types::Uuid, _>(member_id)
    .execute(&mut conn);
    assert!(
        dup.is_err(),
        "the UNIQUE(actor_id) constraint must reject a duplicate claim row"
    );
}

/// FR-034: a GM binds a player from the players section, and the row
/// that results is the same relation every other surface reads.
#[tokio::test]
async fn gm_binds_a_player_to_a_character() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    let member_id = member_id_of(&mut conn, world_id, player_id);
    drop(conn);

    set_player_character_binding_impl(&state, owner_id, false, world_id, member_id, Some(actor_id))
        .await
        .expect("the world's owner may set a player's character");

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(claimants_of(&mut conn, actor_id), vec![member_id]);

    // The binding is visible to the player's own surfaces too — one
    // relation, not a parallel one that only the GM screen knows about.
    let claim = my_actor_claim_impl(&state, player_id, world_id)
        .await
        .unwrap()
        .expect("the bound player must see the character as theirs");
    assert_eq!(claim.actor_id, actor_id);
}

/// A GM correcting a binding replaces it. Two rows for one player would
/// be the "player bound to two characters" FR-034 forbids.
#[tokio::test]
async fn rebinding_a_player_replaces_the_previous_character() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let first = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let second = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Bran");
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    let member_id = member_id_of(&mut conn, world_id, player_id);
    drop(conn);

    for actor_id in [first, second] {
        set_player_character_binding_impl(
            &state,
            owner_id,
            false,
            world_id,
            member_id,
            Some(actor_id),
        )
        .await
        .expect("a GM may re-bind a player");
    }

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(characters_of(&mut conn, member_id), vec![second]);
    assert!(
        claimants_of(&mut conn, first).is_empty(),
        "the character the player was moved off must be free again"
    );
}

/// Clearing a binding leaves the player a member with no character.
#[tokio::test]
async fn clearing_a_binding_leaves_the_player_without_a_character() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let player_id = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player_id, "Player");
    let member_id = member_id_of(&mut conn, world_id, player_id);
    drop(conn);

    set_player_character_binding_impl(&state, owner_id, false, world_id, member_id, Some(actor_id))
        .await
        .unwrap();
    let cleared =
        set_player_character_binding_impl(&state, owner_id, false, world_id, member_id, None)
            .await
            .expect("a GM may clear a binding");
    assert!(cleared.is_none());

    let mut conn = state.db_pool.get().unwrap();
    assert!(characters_of(&mut conn, member_id).is_empty());
    assert!(claimants_of(&mut conn, actor_id).is_empty());
}

/// A character somebody else plays is refused with the code the client
/// keys on, and the existing binding is untouched.
#[tokio::test]
async fn binding_a_character_another_player_plays_is_refused() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let held_by = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, held_by, "Player");
    let holder_member = member_id_of(&mut conn, world_id, held_by);
    let latecomer = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, latecomer, "Player");
    let latecomer_member = member_id_of(&mut conn, world_id, latecomer);
    drop(conn);

    set_player_character_binding_impl(
        &state,
        owner_id,
        false,
        world_id,
        holder_member,
        Some(actor_id),
    )
    .await
    .unwrap();

    let refusal = set_player_character_binding_impl(
        &state,
        owner_id,
        false,
        world_id,
        latecomer_member,
        Some(actor_id),
    )
    .await
    .expect_err("a character already played may not be handed to a second player");
    assert!(
        error_code(&refusal).contains(ALREADY_CLAIMED),
        "the refusal must be distinguishable from a malfunction; got {refusal:?}"
    );

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        claimants_of(&mut conn, actor_id),
        vec![holder_member],
        "a refused binding must leave the standing one exactly as it was"
    );
}

/// FR-034 / Constitution III: the picker is chrome. A player calling the
/// mutation directly for somebody else's binding is refused server-side.
#[tokio::test]
async fn a_player_may_not_set_another_players_binding() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let meddler = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, meddler, "Player");
    let victim = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, victim, "Player");
    let victim_member = member_id_of(&mut conn, world_id, victim);
    drop(conn);

    set_player_character_binding_impl(
        &state,
        meddler,
        false,
        world_id,
        victim_member,
        Some(actor_id),
    )
    .await
    .expect_err("a Player may not bind characters for other players");

    let mut conn = state.db_pool.get().unwrap();
    assert!(
        claimants_of(&mut conn, actor_id).is_empty(),
        "a refused binding must write nothing"
    );
}

/// The T067 case itself: the players section and a player's own claim
/// screen going for the same character at the same moment. Exactly one
/// row exists afterwards, and whoever lost is told which thing happened.
#[tokio::test]
async fn a_gm_binding_and_a_player_claim_cannot_both_win() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    mark_available(&mut conn, actor_id, true);

    // Two different players, so a double-write shows up as two rows
    // rather than as one row written twice.
    let self_claimer = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, self_claimer, "Player");
    let bound_player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, bound_player, "Player");
    let bound_member = member_id_of(&mut conn, world_id, bound_player);
    drop(conn);

    let binding = {
        let state = state.clone();
        tokio::spawn(async move {
            set_player_character_binding_impl(
                &state,
                owner_id,
                false,
                world_id,
                bound_member,
                Some(actor_id),
            )
            .await
            .map(|_| ())
        })
    };
    let self_claim = {
        let state = state.clone();
        tokio::spawn(async move {
            claim_actor_impl(&state, self_claimer, world_id, actor_id)
                .await
                .map(|_| ())
        })
    };

    let mut winners = 0;
    for attempt in [binding, self_claim] {
        match attempt.await.expect("claim task must not panic") {
            Ok(()) => winners += 1,
            Err(e) => assert!(
                error_code(&e).contains(ALREADY_CLAIMED),
                "the loser of a contested character must be told exactly \
                 that; got {e:?}"
            ),
        }
    }

    assert_eq!(winners, 1, "two writers, one character, one winner");

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        claimants_of(&mut conn, actor_id).len(),
        1,
        "a contested character must end up claimed exactly once"
    );
}

/// The actor page's release, aimed at a claim that has since moved.
/// Without the conditional delete this test's binding would vanish.
#[tokio::test]
async fn unclaiming_a_stale_claim_changes_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner_id = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner_id);
    let scene_id = insert_test_scene(&mut conn, world_id, owner_id);
    let actor_id = insert_test_pc(&mut conn, world_id, scene_id, owner_id, "Aria");
    let first_player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, first_player, "Player");
    let first_member = member_id_of(&mut conn, world_id, first_player);
    let second_player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, second_player, "Player");
    let second_member = member_id_of(&mut conn, world_id, second_player);
    drop(conn);

    // What the actor page rendered.
    set_player_character_binding_impl(
        &state,
        owner_id,
        false,
        world_id,
        first_member,
        Some(actor_id),
    )
    .await
    .unwrap();

    // What the players section did while that page sat open.
    set_player_character_binding_impl(&state, owner_id, false, world_id, first_member, None)
        .await
        .unwrap();
    set_player_character_binding_impl(
        &state,
        owner_id,
        false,
        world_id,
        second_member,
        Some(actor_id),
    )
    .await
    .unwrap();

    let refusal = unclaim_actor_impl(&state, owner_id, false, actor_id, Some(first_member))
        .await
        .expect_err("releasing a claim that has moved on must be refused");
    assert!(
        error_code(&refusal).contains(CLAIM_CHANGED),
        "a stale release is a changed claim, not a malfunction; got {refusal:?}"
    );

    let mut conn = state.db_pool.get().unwrap();
    assert_eq!(
        claimants_of(&mut conn, actor_id),
        vec![second_member],
        "a stale release must not erase the binding it never saw"
    );

    // The release the page would issue after re-reading does work.
    drop(conn);
    unclaim_actor_impl(&state, owner_id, false, actor_id, Some(second_member))
        .await
        .expect("releasing the claim that is actually there must succeed");
    let mut conn = state.db_pool.get().unwrap();
    assert!(claimants_of(&mut conn, actor_id).is_empty());
}
