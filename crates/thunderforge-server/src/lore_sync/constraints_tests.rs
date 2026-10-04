//! T017: the two constraints spec 034 leans on, exercised rather than assumed.
//!
//! FR-001 ("a world MUST have at most one connection") and FR-033 ("two worlds
//! MUST NOT synchronise into the same directory of the same repository") are
//! both enforced in the schema rather than in application code, because
//! enforcing them in application code would be enforcing them nowhere — every
//! future insert path would have to remember, and one of them eventually will
//! not.
//!
//! That argument is only worth anything if the constraints actually reject. A
//! `UNIQUE` written into a migration that never ran, or that was quietly
//! dropped by a later one, looks identical in the source to one that works.
//! These two tests are the difference between the rule being enforced and the
//! rule being commented.

use diesel::prelude::*;
use uuid::Uuid;

use crate::models::LoreRepositoryConnection;
use crate::schema::lore_repository_connections;

fn connection_row(world_id: Uuid, owner: Uuid, repo: &str, dir: &str) -> LoreRepositoryConnection {
    let now = chrono::Utc::now().naive_utc();
    LoreRepositoryConnection {
        id: Uuid::now_v7(),
        world_id,
        host_kind: "test".to_string(),
        installation_ref: "test-installation".to_string(),
        repository_ref: repo.to_string(),
        branch: "main".to_string(),
        directory: dir.to_string(),
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
    }
}

/// FR-001. A second connection for one world is refused by the database, not
/// by whichever code path happened to check first.
#[test]
fn a_world_cannot_have_two_connections() {
    let mut conn = crate::test_support::test_app_state()
        .db_pool
        .get()
        .expect("a connection");

    let owner = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, owner);
    let repo = format!("owner/{}", Uuid::now_v7());

    diesel::insert_into(lore_repository_connections::table)
        .values(connection_row(world, owner, &repo, "lore"))
        .execute(&mut conn)
        .expect("the first connection is accepted");

    let second = diesel::insert_into(lore_repository_connections::table)
        .values(connection_row(world, owner, &repo, "somewhere-else"))
        .execute(&mut conn);

    assert!(
        second.is_err(),
        "a world accepted a second connection — FR-001 is not enforced",
    );
}

/// FR-033. Two worlds writing into one directory of one repository would
/// interleave two histories into one tree, and neither owner would be able to
/// tell which commits were theirs.
#[test]
fn two_worlds_cannot_claim_one_directory() {
    let mut conn = crate::test_support::test_app_state()
        .db_pool
        .get()
        .expect("a connection");

    let owner = crate::test_support::insert_test_user(&mut conn);
    let first_world = crate::test_support::insert_test_world(&mut conn, owner);
    let second_world = crate::test_support::insert_test_world(&mut conn, owner);
    let repo = format!("owner/{}", Uuid::now_v7());

    diesel::insert_into(lore_repository_connections::table)
        .values(connection_row(first_world, owner, &repo, "lore"))
        .execute(&mut conn)
        .expect("the first world claims the directory");

    let clash = diesel::insert_into(lore_repository_connections::table)
        .values(connection_row(second_world, owner, &repo, "lore"))
        .execute(&mut conn);

    assert!(
        clash.is_err(),
        "two worlds claimed one repository directory — FR-033 is not enforced",
    );

    // The same repository with a *different* directory is allowed, which is
    // the other half of FR-033 and the half a too-broad constraint would break:
    // a user with several worlds may keep them in one repository.
    diesel::insert_into(lore_repository_connections::table)
        .values(connection_row(second_world, owner, &repo, "other-lore"))
        .execute(&mut conn)
        .expect("a second world may use a different directory of the same repository");
}

/// The state column's CHECK. A typo'd state would otherwise persist and then
/// fail to match anywhere it is read, which surfaces far from its cause.
#[test]
fn an_unknown_connection_state_is_refused() {
    let mut conn = crate::test_support::test_app_state()
        .db_pool
        .get()
        .expect("a connection");

    let owner = crate::test_support::insert_test_user(&mut conn);
    let world = crate::test_support::insert_test_world(&mut conn, owner);
    let mut row = connection_row(world, owner, &format!("owner/{}", Uuid::now_v7()), "lore");
    row.state = "definitely-not-a-state".to_string();

    assert!(
        diesel::insert_into(lore_repository_connections::table)
            .values(row)
            .execute(&mut conn)
            .is_err(),
        "an unknown state was accepted",
    );
}
