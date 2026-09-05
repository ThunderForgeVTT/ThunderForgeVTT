use super::update_world_interface_pack_impl;
use crate::graphql::input_types::UpdateWorldInterfacePackInput;
use crate::test_support::*;

/// Test state's directories point at a temp dir, so the pack directory has
/// to be aimed at the repository's real packs — this mutation's whole job
/// is refusing a pack that cannot be applied, and it cannot do that
/// against an empty directory.
fn state_with_real_packs() -> crate::state::AppState {
    let mut state = test_app_state();
    state.directories.interface_packs_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/interface")
        .to_string_lossy()
        .into_owned();
    state
}

fn input(world_id: uuid::Uuid, pack: Option<&str>) -> UpdateWorldInterfacePackInput {
    UpdateWorldInterfacePackInput {
        world_id,
        interface_pack_id: pack.map(str::to_string),
    }
}

#[tokio::test]
async fn a_dm_can_choose_the_worlds_interface_pack() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    let updated =
        update_world_interface_pack_impl(&state, owner, false, input(world_id, Some("forge")))
            .await
            .expect("the DM chooses the world's look");

    assert_eq!(updated.interface_pack_id.as_deref(), Some("forge"));
}

#[tokio::test]
async fn a_player_is_refused_and_told_what_authority_is_required() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    let player = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world_id, player, "Player");
    drop(conn);

    let error =
        update_world_interface_pack_impl(&state, player, false, input(world_id, Some("forge")))
            .await
            .expect_err("a player does not choose the table's look");

    assert!(
        error.message.contains("DM"),
        "the refusal names the authority required rather than failing \
         silently: {}",
        error.message
    );
}

/// Accepting an id for a pack that cannot be applied would manufacture the
/// degraded state of FR-019 from the one place that knows better.
#[tokio::test]
async fn a_pack_that_is_not_installed_is_refused() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    let error = update_world_interface_pack_impl(
        &state,
        owner,
        false,
        input(world_id, Some("no-such-pack")),
    )
    .await
    .expect_err("an uninstallable pack must not be stored");

    assert!(error.message.contains("no-such-pack"), "{}", error.message);
}

/// Clearing is a real thing a Game Master may want: it means "the
/// default", and the default now has a name to show for it.
#[tokio::test]
async fn null_clears_the_binding() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    update_world_interface_pack_impl(&state, owner, false, input(world_id, Some("forge")))
        .await
        .expect("set");
    let cleared = update_world_interface_pack_impl(&state, owner, false, input(world_id, None))
        .await
        .expect("clear");

    assert_eq!(cleared.interface_pack_id, None);
}

/// Whitespace is not a pack id. Treated as clearing rather than as a
/// lookup for `"  "`, which would fail with a message about a pack nobody
/// typed.
#[tokio::test]
async fn a_blank_id_clears_rather_than_failing_to_find_a_pack_called_nothing() {
    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    let updated =
        update_world_interface_pack_impl(&state, owner, false, input(world_id, Some("   ")))
            .await
            .expect("blank clears");
    assert_eq!(updated.interface_pack_id, None);
}

/// Everyone in the world re-resolves on receipt, which is what makes
/// SC-001's "without reloading" true.
#[tokio::test]
async fn choosing_a_pack_records_a_world_event() {
    use crate::schema::world_events;
    use diesel::prelude::*;

    let state = state_with_real_packs();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world_id = insert_test_world(&mut conn, owner);
    drop(conn);

    update_world_interface_pack_impl(&state, owner, false, input(world_id, Some("forge")))
        .await
        .expect("set");

    let mut conn = state.db_pool.get().unwrap();
    let codes: Vec<i32> = world_events::table
        .filter(world_events::world_id.eq(world_id))
        .select(world_events::event_code)
        .load(&mut conn)
        .expect("events readable");

    assert!(
        codes.contains(&crate::world_events::EVENT_CODE_WORLD_APPEARANCE_CHANGED),
        "the table has to be told, or it sees the change only on reload"
    );
}
