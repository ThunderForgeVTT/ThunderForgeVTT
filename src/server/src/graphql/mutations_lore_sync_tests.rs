use super::*;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

/// A connection row for a world that already has one, inserted directly:
/// these tests are about the *refusals*, and going through
/// `complete_lore_repository_connection_impl` to create the first one
/// would mean going through the grant boundary T014 has not built yet.
fn insert_connection_row(
    conn: &mut PgConnection,
    world_id: Uuid,
    owner: Uuid,
    repository_ref: &str,
    directory: &str,
) -> Uuid {
    let now = chrono::Utc::now().naive_utc();
    let id = Uuid::now_v7();
    diesel::insert_into(lore_repository_connections::table)
        .values(LoreRepositoryConnection {
            id,
            world_id,
            host_kind: "test".to_string(),
            installation_ref: "test-installation".to_string(),
            repository_ref: repository_ref.to_string(),
            branch: "main".to_string(),
            directory: directory.to_string(),
            incoming_enabled: false,
            notice_acknowledged_at: None,
            state: "never_configured".to_string(),
            state_reason: None,
            repository_is_public: None,
            visibility_checked_at: None,
            deactivated_at: None,
            deactivated_reason: None,
            last_synced_at: None,
            last_written_commit: None,
            created_by: owner,
            updated_by: owner,
            created_at: now,
            updated_at: now,
        })
        .execute(conn)
        .expect("failed to insert test connection");
    id
}

fn complete_input(world_id: Uuid, directory: &str) -> CompleteConnectionInput {
    CompleteConnectionInput {
        world_id,
        grant_response: "{}".to_string(),
        // These tests are about the refusals, which all happen before a
        // repository is chosen.
        repository_ref: None,
        branch: None,
        directory: Some(directory.to_string()),
    }
}

/// **FR-022, at the boundary a client can actually reach.**
///
/// The gate is enforced inside `incoming`, by a type. This asserts the
/// GraphQL surface asks for it rather than reimplementing a flag check —
/// a world that never opted in must be unable to have a change applied
/// even by someone with every permission.
#[tokio::test]
async fn a_world_that_never_opted_in_cannot_have_a_change_applied() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    // incoming_enabled defaults to false — the state every connection
    // starts in, and the one FR-022 protects.
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );

    let attempt = accept_lore_incoming_change_impl(
        &state,
        owner,
        true, // even as an administrator
        world,
        Uuid::now_v7(),
    )
    .await;

    let message = attempt.expect_err("a world that never opted in accepted a change");
    assert!(
        message.message.contains("has not enabled"),
        "the refusal did not name the reason: {}",
        message.message,
    );
}

/// The same gate on the declining path. Declining is harmless, but a world
/// that never opted in has nothing to decline and should not be told it
/// does.
#[tokio::test]
async fn a_world_that_never_opted_in_cannot_decline_either() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );

    assert!(
        decline_lore_incoming_change_impl(&state, owner, false, world, Uuid::now_v7())
            .await
            .is_err(),
    );
}

/// FR-041a again, through this door. An enforcement deactivation must
/// close every write path, not only the synchronising one.
#[tokio::test]
async fn a_deactivated_connection_cannot_apply_incoming_changes() {
    use crate::schema::lore_repository_connections as c;
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );

    // Opted in, and then deactivated. The flag alone would let this pass.
    diesel::update(c::table.filter(c::world_id.eq(world)))
        .set((c::incoming_enabled.eq(true), c::state.eq("deactivated")))
        .execute(&mut conn)
        .expect("deactivate");

    assert!(
        accept_lore_incoming_change_impl(&state, owner, true, world, Uuid::now_v7())
            .await
            .is_err(),
        "a deactivated connection applied a change from its repository",
    );
}

/// FR-041a. A deactivation the owner can undo is not a deactivation, and a
/// commitment made to a rights holder that the product cannot carry out is
/// worse than no commitment.
#[tokio::test]
async fn only_an_administrator_may_deactivate_a_connection() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );

    let by_owner = deactivate_lore_sync_impl(&state, false, world, "not allowed".to_string()).await;
    assert!(
        by_owner.is_err(),
        "a world owner deactivated their own connection"
    );

    let by_admin =
        deactivate_lore_sync_impl(&state, true, world, "repeat infringer".to_string()).await;
    assert!(by_admin.expect("an administrator may"));
}

/// FR-041c and FR-031 together. Resolving a divergence must not be a side
/// door out of an enforcement action — otherwise "cannot be lifted by its
/// owner" is false, and false in the one place it matters.
#[tokio::test]
async fn a_deactivated_connection_cannot_be_resumed_by_resolving_a_divergence() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );
    deactivate_lore_sync_impl(&state, true, world, "enforcement".to_string())
        .await
        .expect("deactivated");

    let attempt = resolve_lore_sync_divergence_impl(
        &state,
        owner,
        false,
        world,
        DivergenceResolution::OverwriteRemote,
    )
    .await;

    assert!(
        attempt.is_err(),
        "an enforcement action was lifted by its owner"
    );
}

/// Overwriting authorises the next push by clearing what the lease is
/// taken against. A stored fact rather than a flag carried in memory, so a
/// restart cannot lose it and nothing else can invent it.
#[tokio::test]
async fn overwriting_clears_the_lease_the_next_push_would_fail_against() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );

    diesel::update(
        lore_repository_connections::table.filter(lore_repository_connections::world_id.eq(world)),
    )
    .set((
        lore_repository_connections::last_written_commit.eq(Some("deadbeef".to_string())),
        lore_repository_connections::state.eq("needs_attention"),
    ))
    .execute(&mut conn)
    .expect("set up a diverged connection");

    resolve_lore_sync_divergence_impl(
        &state,
        owner,
        false,
        world,
        DivergenceResolution::OverwriteRemote,
    )
    .await
    .expect("the owner may overwrite");

    let after = load_connection(&state, world)
        .await
        .expect("loaded")
        .expect("present");
    assert_eq!(after.last_written_commit, None, "the stale lease survived");
    assert_eq!(after.state, "working");
}

/// The other answer. Abandoning leaves the repository exactly as it is,
/// including whatever diverged — FR-005 says removing a connection touches
/// nothing in the repository, and a divergence is not an exception.
#[tokio::test]
async fn abandoning_a_divergence_removes_the_connection() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("a connection");
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_connection_row(
        &mut conn,
        world,
        owner,
        &format!("o/{}", Uuid::now_v7()),
        "lore",
    );

    resolve_lore_sync_divergence_impl(
        &state,
        owner,
        false,
        world,
        DivergenceResolution::AbandonConnection,
    )
    .await
    .expect("the owner may abandon");

    assert!(
        load_connection(&state, world)
            .await
            .expect("loaded")
            .is_none(),
        "the connection survived being abandoned",
    );
}

/// FR-002. A player in the world is not an owner, and the refusal happens
/// before anything else — in particular before the caller is told whether
/// the world has a connection at all.
#[tokio::test]
async fn a_non_owner_cannot_manage_the_connection() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    insert_test_world_member(&mut conn, world, player, "Player");
    drop(conn);

    let begun = begin_lore_repository_connection_impl(&state, player, false, world).await;
    assert!(begun.is_err(), "a player was allowed to begin a connection");

    let acknowledged = acknowledge_lore_sync_notice_impl(&state, player, false, world).await;
    assert!(
        acknowledged.is_err(),
        "a player was allowed to acknowledge the notice",
    );

    let removed = remove_lore_repository_connection_impl(&state, player, false, world).await;
    assert!(
        removed.is_err(),
        "a player was allowed to remove the connection",
    );
}

/// FR-001. The second connection is refused with a sentence naming the
/// remedy, and — the part that matters — refused *before* the user is
/// handed off to the host, so nothing has to be undone at the host to
/// recover.
#[tokio::test]
async fn a_world_cannot_take_a_second_connection() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let repo = format!("owner/{}", Uuid::now_v7());
    insert_connection_row(&mut conn, world, owner, &repo, "lore");
    drop(conn);

    let begun = begin_lore_repository_connection_impl(&state, owner, false, world).await;
    let message = begun.expect_err("a second connection was begun").message;
    assert!(message.contains("already connected"), "{message}");

    let completed = complete_lore_repository_connection_impl(
        &state,
        owner,
        false,
        complete_input(world, "elsewhere"),
    )
    .await;
    let message = completed
        .expect_err("a second connection was completed")
        .message;
    assert!(message.contains("already connected"), "{message}");
}

/// FR-033. Two worlds writing into one directory of one repository would
/// interleave two histories into one tree.
///
/// Asserted at the database, which is where the rule actually lives: the
/// pre-check in `complete_lore_repository_connection_impl` runs after the
/// grant boundary T014 has not built, so testing only the pre-check today
/// would be testing an unreachable branch.
#[tokio::test]
async fn two_worlds_cannot_claim_one_repository_directory() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let first_world = insert_test_world(&mut conn, owner);
    let second_world = insert_test_world(&mut conn, owner);
    let repo = format!("owner/{}", Uuid::now_v7());
    insert_connection_row(&mut conn, first_world, owner, &repo, "lore");

    let claimed = repository_directory_is_claimed(&state, &repo, "lore", second_world).await;
    assert!(
        claimed.expect("the claim check should answer"),
        "the second world was told the directory was free",
    );

    let duplicate = diesel::insert_into(lore_repository_connections::table)
        .values(LoreRepositoryConnection {
            id: Uuid::now_v7(),
            world_id: second_world,
            host_kind: "test".to_string(),
            installation_ref: "test-installation".to_string(),
            repository_ref: repo.clone(),
            branch: "main".to_string(),
            directory: "lore".to_string(),
            incoming_enabled: false,
            notice_acknowledged_at: None,
            state: "never_configured".to_string(),
            state_reason: None,
            repository_is_public: None,
            visibility_checked_at: None,
            deactivated_at: None,
            deactivated_reason: None,
            last_synced_at: None,
            last_written_commit: None,
            created_by: owner,
            updated_by: owner,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        })
        .execute(&mut conn);
    assert!(
        duplicate.is_err(),
        "two worlds claimed one repository directory",
    );
}

/// FR-005. Removing a connection is the platform forgetting, and forgetting
/// must not take the world's lore with it.
///
/// This is the test that would catch a future `ON DELETE CASCADE` pointed
/// the wrong way, which is exactly the mistake that would be invisible in
/// review.
#[tokio::test]
async fn removing_a_connection_leaves_the_worlds_lore_intact() {
    use crate::schema::world_lore_entries;

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let repo = format!("owner/{}", Uuid::now_v7());
    insert_connection_row(&mut conn, world, owner, &repo, "lore");

    let entry_id = Uuid::now_v7();
    let now = chrono::Utc::now().naive_utc();
    diesel::insert_into(world_lore_entries::table)
        .values((
            world_lore_entries::id.eq(entry_id),
            world_lore_entries::world_id.eq(world),
            world_lore_entries::title.eq("The Salt Road"),
            world_lore_entries::slug.eq(format!("salt-road-{}", entry_id.simple())),
            world_lore_entries::content.eq("It runs east."),
            world_lore_entries::created_by.eq(owner),
            world_lore_entries::created_at.eq(now),
            world_lore_entries::updated_at.eq(now),
        ))
        .execute(&mut conn)
        .expect("failed to insert test lore entry");
    drop(conn);

    let removed = remove_lore_repository_connection_impl(&state, owner, false, world)
        .await
        .expect("the owner should be able to remove the connection");
    assert!(removed, "the connection was not removed");

    let mut conn = state.db_pool.get().unwrap();
    let surviving = world_lore_entries::table
        .filter(world_lore_entries::id.eq(entry_id))
        .select(world_lore_entries::content)
        .first::<String>(&mut conn)
        .expect("the lore entry should have survived the removal");
    assert_eq!(surviving, "It runs east.");

    assert!(
        load_connection(&state, world)
            .await
            .expect("the connection query should answer")
            .is_none(),
        "the connection row is still present",
    );
}

/// A second removal is not an error. A client that asks twice — a retried
/// request, two tabs — should see "already gone", because that is what is
/// true.
#[tokio::test]
async fn removing_a_missing_connection_is_not_an_error() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    drop(conn);

    let removed = remove_lore_repository_connection_impl(&state, owner, false, world)
        .await
        .expect("removing nothing should succeed");
    assert!(!removed);
}

/// The directory becomes a path on this server's disk before it becomes a
/// path in someone's repository, so `..` is refused rather than
/// normalised away.
#[test]
fn a_directory_cannot_escape_the_repository() {
    assert!(normalize_directory("../../etc").is_err());
    assert!(normalize_directory("lore/../..").is_err());
    assert!(normalize_directory("   ").is_err());
    assert_eq!(normalize_directory("/lore/").unwrap(), "lore");
    assert_eq!(
        normalize_directory("campaigns/lore").unwrap(),
        "campaigns/lore"
    );
}

/// A branch name starting with `-` would be read as an option by the git
/// invocations that carry it.
#[test]
fn a_branch_name_cannot_be_an_option() {
    assert!(normalize_branch(Some("--upload-pack=evil")).is_err());
    assert!(normalize_branch(Some("has space")).is_err());
    assert_eq!(normalize_branch(None).unwrap(), DEFAULT_BRANCH);
    assert_eq!(
        normalize_branch(Some("release/1.0")).unwrap(),
        "release/1.0"
    );
}
