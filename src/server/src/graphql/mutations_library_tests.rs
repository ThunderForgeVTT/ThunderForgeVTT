//! The book list as a client meets it (spec 050 T059, T060, T062).
//!
//! `library::book_list` proves the rules against the database. These prove
//! that the wire does not widen them — which is the failure this arc is most
//! exposed to, because the store is account-scoped and these resolvers are
//! reached with a world id. Every one of them is a caller who should be
//! refused asking politely.

use super::*;
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState};
use crate::schema::worlds;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};
use diesel::prelude::*;

const SYSTEM: &str = "test-system";

fn a_book(title: &str, system: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: system.to_string(),
        parser_version: "reader-test".to_string(),
        page_count: 12,
        silent_page_count: 0,
    }
}

fn a_creature(name: &str) -> Entry {
    Entry {
        kind: "creature".to_string(),
        name: name.to_string(),
        name_state: NameState::Clear,
        page: 3,
        values: Default::default(),
        text: None,
        suspect: false,
        extras: None,
    }
}

fn a_world_running(conn: &mut PgConnection, owner: Uuid, system: &str) -> Uuid {
    let world_id = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some(system.to_string())))
        .execute(conn)
        .expect("failed to set the world's system");
    world_id
}

/// FR-035, both halves in one test: the player sees what the table is
/// running, and every route that would change it refuses them.
#[tokio::test]
async fn a_player_reads_the_list_and_cannot_change_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    insert_test_world_member(&mut conn, world, player, "Player");
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();
    drop(conn);

    switch_on_impl(&state, owner, world, book.id).await.unwrap();

    let seen = world_book_list_impl(&state, player, world).await.unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].book_title, "Monster Manual");
    // FR-036: the name and how much is in it, and nothing that is in it.
    assert_eq!(seen[0].entry_total, 1);

    assert!(
        switch_on_impl(&state, player, world, book.id)
            .await
            .is_err()
    );
    assert!(
        switch_off_impl(&state, player, world, book.id, true)
            .await
            .is_err()
    );
    assert!(
        switch_off_impl(&state, player, world, book.id, false)
            .await
            .is_err(),
        "not even the harmless-looking report, which names entries"
    );
    assert!(
        compendiums_offered_impl(&state, player, world)
            .await
            .is_err()
    );
    assert!(
        world_compendium_entries_impl(&state, player, world, book.id, None, None, None)
            .await
            .is_err(),
        "browsing a book is a Game Master's, not a player's (049 FR-042)"
    );

    // And after all that, the list is still what the Game Master set.
    let after = world_book_list_impl(&state, player, world).await.unwrap();
    assert_eq!(after.len(), 1);
}

/// The account boundary, at the wire. A co-Game Master reads the owner's book
/// at the owner's table and cannot put it on their own — the same two answers
/// the store gives, reached through a resolver that takes a world id.
#[tokio::test]
async fn a_co_game_master_uses_here_and_inherits_nowhere() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let co_gm = insert_test_user(&mut conn);
    let shared = a_world_running(&mut conn, owner, SYSTEM);
    insert_test_world_member(&mut conn, shared, co_gm, "GM");
    let theirs = a_world_running(&mut conn, co_gm, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin"), a_creature("Orc")],
    )
    .unwrap();
    drop(conn);

    switch_on_impl(&state, owner, shared, book.id)
        .await
        .unwrap();

    let browsed = world_compendium_entries_impl(&state, co_gm, shared, book.id, None, None, None)
        .await
        .unwrap();
    assert_eq!(browsed.total, 2, "a co-GM uses what the table is running");

    assert!(
        switch_on_impl(&state, co_gm, theirs, book.id)
            .await
            .is_err(),
        "and cannot take it home"
    );
    assert!(
        world_book_list_impl(&state, co_gm, theirs)
            .await
            .unwrap()
            .is_empty()
    );
}

/// A stranger is at no table, so the list is not theirs to read either. The
/// refusal says "you are not at this table" rather than "no such world", for
/// the reason every refusal in this arc is written that way.
#[tokio::test]
async fn a_stranger_reads_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();
    drop(conn);

    switch_on_impl(&state, owner, world, book.id).await.unwrap();

    assert!(world_book_list_impl(&state, stranger, world).await.is_err());
    assert!(
        world_compendium_entries_impl(&state, stranger, world, book.id, None, None, None)
            .await
            .is_err()
    );
}

/// FR-042 at the wire: after a world changes system, the list still names the
/// book — a Game Master must be told — and the fetch refuses it.
#[tokio::test]
async fn a_changed_system_is_reported_and_not_served() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();
    drop(conn);

    switch_on_impl(&state, owner, world, book.id).await.unwrap();

    let mut conn = state.db_pool.get().unwrap();
    diesel::update(worlds::table.filter(worlds::id.eq(world)))
        .set(worlds::game_system_id.eq(Some("other-system".to_string())))
        .execute(&mut conn)
        .unwrap();
    drop(conn);

    let listed = world_book_list_impl(&state, owner, world).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert!(!listed[0].system_matches);
    assert!(
        world_compendium_entries_impl(&state, owner, world, book.id, None, None, None)
            .await
            .is_err(),
        "a book the world no longer matches must not go on being served"
    );
}

/// FR-013: the report names what goes and takes nothing; the confirmation
/// takes it. Two calls, and the first one is not a dry run of the second in
/// name only.
#[tokio::test]
async fn switching_off_reports_before_it_takes() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin"), a_creature("Orc")],
    )
    .unwrap();
    drop(conn);

    switch_on_impl(&state, owner, world, book.id).await.unwrap();

    let warned = switch_off_impl(&state, owner, world, book.id, false)
        .await
        .unwrap();
    assert!(!warned.switched_off);
    assert_eq!(warned.entry_count, 2);
    assert!(warned.entry_names.contains(&"Goblin".to_string()));
    assert_eq!(
        world_book_list_impl(&state, owner, world)
            .await
            .unwrap()
            .len(),
        1
    );

    let done = switch_off_impl(&state, owner, world, book.id, true)
        .await
        .unwrap();
    assert!(done.switched_off);
    assert!(
        world_book_list_impl(&state, owner, world)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        world_compendium_entries_impl(&state, owner, world, book.id, None, None, None)
            .await
            .is_err(),
        "and the content goes with the link, because there was never a copy"
    );
}

/// 050 FR-060 and 049 FR-045, now that a world can depend on a book: taking a
/// book off the shelf names every world running it before anything goes. This
/// is the `usage_of` in `mutations_compendium` that answered "nowhere" until
/// the book list existed to be asked.
#[tokio::test]
async fn removing_a_book_from_the_shelf_names_the_worlds_running_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let first = a_world_running(&mut conn, owner, SYSTEM);
    let second = a_world_running(&mut conn, owner, SYSTEM);
    let untouched = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();
    drop(conn);

    switch_on_impl(&state, owner, first, book.id).await.unwrap();
    switch_on_impl(&state, owner, second, book.id)
        .await
        .unwrap();

    let report =
        crate::graphql::mutations_compendium::remove_compendium_impl(&state, owner, book.id, false)
            .await
            .unwrap();

    let named: Vec<Uuid> = report.in_use.iter().map(|usage| usage.world_id).collect();
    assert_eq!(named.len(), 2);
    assert!(named.contains(&first) && named.contains(&second));
    assert!(!named.contains(&untouched));
    assert!(
        report.in_use[0].entry_names.contains(&"Goblin".to_string()),
        "what leaves each table is named, not only counted"
    );
    assert!(!report.removed);
    assert_eq!(
        world_book_list_impl(&state, owner, first)
            .await
            .unwrap()
            .len(),
        1
    );
}

/// Spec 050 decision 8 at the wire, across every role and a stranger.
///
/// Each row is one caller doing the whole round — asking what is offered,
/// switching the owner's book on, asking what switching it off would take,
/// and taking it off — and the table says who gets through. Two things are
/// asserted for every caller that does: the book that went on is the
/// **owner's** (FR-010a), and the caller's own book of the same name is
/// neither offered nor accepted.
///
/// Browsing what a book says is a separate column, because ADR-099 trusts a
/// Trusted Player to arrange the books and not to read them.
#[tokio::test]
async fn the_book_list_answers_to_owner_game_master_and_trusted_player_alone() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let owners_book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin"), a_creature("Orc")],
    )
    .unwrap();

    let mut member = |role: &str| {
        let user = insert_test_user(&mut conn);
        insert_test_world_member(&mut conn, world, user, role);
        user
    };
    let game_master = member("GM");
    let trusted = member("TrustedPlayer");
    let player = member("Player");
    let stranger = insert_test_user(&mut conn);

    // (who, caller, arranges the list, browses a book, reads the list)
    let table = [
        ("Owner", owner, true, true, true),
        ("Game Master", game_master, true, true, true),
        ("Trusted Player", trusted, true, true, true),
        ("Player", player, false, false, true),
        ("stranger", stranger, false, false, false),
    ];

    let mut their_own = std::collections::HashMap::new();
    for (_, caller, ..) in table {
        if caller != owner {
            let book = store::import_book(&mut conn, caller, a_book("Monster Manual", SYSTEM), &[])
                .unwrap();
            their_own.insert(caller, book.id);
        }
    }
    drop(conn);

    for (who, caller, arranges, browses, reads) in table {
        let offered = compendiums_offered_impl(&state, caller, world).await;
        let on = switch_on_impl(&state, caller, world, owners_book.id).await;

        assert_eq!(offered.is_ok(), arranges, "{who}: offered");
        assert_eq!(on.is_ok(), arranges, "{who}: switch on");
        assert_eq!(
            world_book_list_impl(&state, caller, world).await.is_ok(),
            reads,
            "{who}: read the list"
        );

        if !arranges {
            // Put it on as somebody trusted, so the refusals below are
            // refusals to change something that is there.
            switch_on_impl(&state, trusted, world, owners_book.id)
                .await
                .unwrap();
        } else {
            // Asked before the switch: the owner's shelf, and not the
            // caller's own copy of the same book.
            let offered: Vec<Uuid> = offered.unwrap().into_iter().map(|b| b.id).collect();
            assert_eq!(
                offered,
                vec![owners_book.id],
                "{who}: the owner's shelf only"
            );
            let listed = on.unwrap();
            assert_eq!(listed.len(), 1, "{who}");
            assert_eq!(
                listed[0].compendium_id, owners_book.id,
                "{who}: the owner's book"
            );
        }

        assert_eq!(
            world_compendium_entries_impl(&state, caller, world, owners_book.id, None, None, None)
                .await
                .is_ok(),
            browses,
            "{who}: browse the book"
        );

        if let Some(theirs) = their_own.get(&caller) {
            assert!(
                switch_on_impl(&state, caller, world, *theirs)
                    .await
                    .is_err(),
                "{who} must not stock the table from their own shelf"
            );
        }

        let report = switch_off_impl(&state, caller, world, owners_book.id, false).await;
        assert_eq!(report.is_ok(), arranges, "{who}: switch-off report");
        if let Ok(report) = report {
            assert!(!report.switched_off);
            assert_eq!(report.entry_count, 2, "{who}");
        }

        let off = switch_off_impl(&state, caller, world, owners_book.id, true).await;
        assert_eq!(off.is_ok(), arranges, "{who}: switch off");

        let still_on = world_book_list_impl(&state, owner, world).await.unwrap();
        if arranges {
            assert!(still_on.is_empty(), "{who} took it off");
        } else {
            assert_eq!(still_on.len(), 1, "{who} changed nothing");
            switch_off_impl(&state, owner, world, owners_book.id, true)
                .await
                .unwrap();
        }
    }
}
