//! The book list, against a real database (spec 050 T058 to T061).
//!
//! Two of these matter more than the rest, and they are the two that sit on
//! the boundary this phase exists to get right: a co-Game Master cannot take
//! the owner's book home, and the owner's book cannot be put on a world that
//! is not theirs. Both are attempted from **both sides** — through this module
//! and around it, with SQL — because a rule that lives only in the calling
//! code is a rule until somebody writes different calling code.

use super::*;
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState};
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

const SYSTEM: &str = "test-system";

fn a_book(title: &str, system: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: system.to_string(),
        parser_version: "reader-test".to_string(),
        page_count: 320,
        silent_page_count: 0,
    }
}

fn a_creature(name: &str) -> Entry {
    Entry {
        kind: "creature".to_string(),
        name: name.to_string(),
        name_state: NameState::Clear,
        page: 166,
        values: Default::default(),
        text: None,
        suspect: false,
        extras: None,
    }
}

/// A world that has said what it runs. `insert_test_world` deliberately
/// leaves the system unset, and an unset system matches no book at all.
fn a_world_running(conn: &mut PgConnection, owner: Uuid, system: &str) -> Uuid {
    let world_id = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some(system.to_string())))
        .execute(conn)
        .expect("failed to set the world's system");
    world_id
}

fn entry_count(conn: &mut PgConnection, compendium_id: Uuid) -> i64 {
    compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .count()
        .get_result(conn)
        .unwrap()
}

/// FR-010 and FR-015: the book goes on, and the row names the base that was
/// in force when it did.
#[test]
fn a_game_master_switches_their_own_book_on() {
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

    let on = switch_on(&mut conn, owner, world, book.id).unwrap();

    assert_eq!(on.compendium_id, book.id);
    assert_eq!(on.base_source_hash, book.source_hash);
    assert_eq!(on.base_parser_version, book.parser_version);
    assert_eq!(on.switched_on_by, Some(owner));
}

/// FR-033: one mechanism, and asking for a state you already have is not an
/// error. Ticking at world creation and ticking afterwards reach this same
/// call, so the second tick must be harmless rather than a failure a
/// creation form has to special-case.
#[test]
fn switching_on_twice_is_switching_on_once() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(&mut conn, owner, a_book("Monster Manual", SYSTEM), &[]).unwrap();

    let first = switch_on(&mut conn, owner, world, book.id).unwrap();
    let second = switch_on(&mut conn, owner, world, book.id).unwrap();

    assert_eq!(
        first.id, second.id,
        "a second tick must not be a second row"
    );
    assert_eq!(books_on(&mut conn, world).unwrap().len(), 1);
}

/// FR-011 and FR-070, at the level the storage claim is actually made: a
/// second world switching the same book on writes one row of a book list and
/// not one entry of content.
#[test]
fn a_second_world_stores_no_second_copy() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let first = a_world_running(&mut conn, owner, SYSTEM);
    let second = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin"), a_creature("Orc")],
    )
    .unwrap();

    switch_on(&mut conn, owner, first, book.id).unwrap();
    let after_one = entry_count(&mut conn, book.id);
    switch_on(&mut conn, owner, second, book.id).unwrap();

    assert_eq!(after_one, 2);
    assert_eq!(entry_count(&mut conn, book.id), after_one);
    // And both tables genuinely have it, so the stored count above is not
    // flat because the second switch quietly did nothing.
    assert_eq!(
        entries_served_by(&mut conn, first, book.id, None)
            .unwrap()
            .1
            .len(),
        2
    );
    assert_eq!(
        entries_served_by(&mut conn, second, book.id, None)
            .unwrap()
            .1
            .len(),
        2
    );
}

/// FR-014 and FR-010a, side one: a co-Game Master may arrange somebody
/// else's table and still may not put their own book on it. Trust to manage
/// the list is not a licence to stock it from another shelf.
#[test]
fn a_co_game_master_cannot_switch_their_own_book_on() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let co_gm = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    insert_test_world_member(&mut conn, world, co_gm, "GM");
    let theirs = store::import_book(&mut conn, co_gm, a_book("Their Book", SYSTEM), &[]).unwrap();

    let refused = switch_on(&mut conn, co_gm, world, theirs.id);

    assert!(
        matches!(refused, Err(BookListError::NotOnTheOwnersShelf)),
        "got {refused:?}"
    );
    assert!(books_on(&mut conn, world).unwrap().is_empty());
}

/// FR-014, side two — the one the requirement is actually about: a co-Game
/// Master cannot take the owner's book home to a world of their own. Being
/// able to use a book at somebody's table is not being able to inherit it.
#[test]
fn a_co_game_master_cannot_take_the_owners_book_home() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let co_gm = insert_test_user(&mut conn);
    let shared = a_world_running(&mut conn, owner, SYSTEM);
    insert_test_world_member(&mut conn, shared, co_gm, "GM");
    let theirs = a_world_running(&mut conn, co_gm, SYSTEM);
    let owners_book =
        store::import_book(&mut conn, owner, a_book("Monster Manual", SYSTEM), &[]).unwrap();
    switch_on(&mut conn, owner, shared, owners_book.id).unwrap();

    let refused = switch_on(&mut conn, co_gm, theirs, owners_book.id);

    assert!(
        matches!(refused, Err(BookListError::NotOnTheOwnersShelf)),
        "a book used at somebody's table must not be inheritable, got {refused:?}"
    );
    assert!(books_on(&mut conn, theirs).unwrap().is_empty());
}

/// The same rule, attempted around this module entirely. If it held only in
/// the Rust above, this insert would succeed — which is what makes the
/// trigger worth having rather than a comment saying the Rust checks it.
#[test]
fn the_database_refuses_a_book_from_another_shelf() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, stranger, SYSTEM);
    let not_theirs =
        store::import_book(&mut conn, owner, a_book("Monster Manual", SYSTEM), &[]).unwrap();

    let refused = diesel::sql_query(format!(
        "INSERT INTO world_books (id, world_id, compendium_id, base_source_hash, \
         base_parser_version, switched_on_by) VALUES ('{}', '{}', '{}', '{}', 'x', '{}')",
        Uuid::now_v7(),
        world,
        not_theirs.id,
        not_theirs.source_hash,
        stranger,
    ))
    .execute(&mut conn);

    assert!(
        refused.is_err(),
        "the database must refuse another shelf's book"
    );
    assert!(
        refused.unwrap_err().to_string().contains("FR-014"),
        "the refusal should say which rule it is"
    );
}

/// FR-041: a book read as one system cannot be switched on in a world running
/// another, and both ends are named so a person knows which is wrong.
#[test]
fn a_book_of_another_system_cannot_be_switched_on() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let elsewhere = store::import_book(
        &mut conn,
        owner,
        a_book("A Pathfinder Book", "other-system"),
        &[],
    )
    .unwrap();

    let refused = switch_on(&mut conn, owner, world, elsewhere.id);

    match refused {
        Err(BookListError::SystemMismatch {
            book_system,
            world_system,
        }) => {
            assert_eq!(book_system, "other-system");
            assert_eq!(world_system.as_deref(), Some(SYSTEM));
        }
        other => panic!("a mismatched system must be refused, got {other:?}"),
    }
    // And it is not offered in the first place (FR-030).
    assert!(offerable_to(&mut conn, owner, world).unwrap().is_empty());
}

/// FR-030: the shelf, narrowed to this world — and a book already on it is
/// not offered again.
#[test]
fn only_matching_books_not_already_on_are_offered() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let matching = store::import_book(&mut conn, owner, a_book("Fits", SYSTEM), &[]).unwrap();
    let other = store::import_book(
        &mut conn,
        owner,
        a_book("Does Not Fit", "other-system"),
        &[],
    )
    .unwrap();

    let offered: Vec<Uuid> = offerable_to(&mut conn, owner, world)
        .unwrap()
        .into_iter()
        .map(|book| book.id)
        .collect();
    assert_eq!(offered, vec![matching.id]);
    assert!(!offered.contains(&other.id));

    switch_on(&mut conn, owner, world, matching.id).unwrap();
    assert!(offerable_to(&mut conn, owner, world).unwrap().is_empty());
}

/// FR-042: a world that changes system says what no longer matches, and stops
/// serving it. Both halves, because either alone is the failure — dropping the
/// row loses the Game Master's decision, and keeping it served is the quiet
/// continuation the requirement forbids.
#[test]
fn a_world_that_changes_system_reports_and_stops_serving() {
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
    switch_on(&mut conn, owner, world, book.id).unwrap();
    assert!(entries_served_by(&mut conn, world, book.id, None).is_ok());

    diesel::update(worlds::table.filter(worlds::id.eq(world)))
        .set(worlds::game_system_id.eq(Some("other-system".to_string())))
        .execute(&mut conn)
        .unwrap();

    let listed = books_on(&mut conn, world).unwrap();
    assert_eq!(listed.len(), 1, "the book stays on the list, and says so");
    assert!(!listed[0].system_matches);
    assert!(matches!(
        entries_served_by(&mut conn, world, book.id, None),
        Err(BookListError::SystemMismatch { .. })
    ));
}

/// FR-013 and FR-032: what goes is named first, then goes — and what is left
/// behind in the world is nothing, while the base is untouched.
#[test]
fn switching_off_names_what_it_takes_then_takes_it() {
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
    switch_on(&mut conn, owner, world, book.id).unwrap();

    let report = switch_off_report(&mut conn, owner, world, book.id).unwrap();
    assert_eq!(report.book_title, "Monster Manual");
    assert_eq!(report.entry_count, 2);
    assert!(report.entry_names.contains(&"Goblin".to_string()));
    assert_eq!(
        books_on(&mut conn, world).unwrap().len(),
        1,
        "asking what a removal takes must take nothing"
    );

    switch_off(&mut conn, owner, world, book.id).unwrap();

    assert!(books_on(&mut conn, world).unwrap().is_empty());
    assert!(matches!(
        entries_served_by(&mut conn, world, book.id, None),
        Err(BookListError::NotOnTheList)
    ));
    // Nothing was copied in, so there is nothing left behind — and the shelf
    // still holds the book in full.
    assert_eq!(entry_count(&mut conn, book.id), 2);
}

/// The fetch is authorised by the **world**, not by the account, and that is
/// the whole difference between using a book and owning one: the co-Game
/// Master reads every entry at this table and is refused the same entries
/// through the library.
#[test]
fn a_co_game_master_reads_what_the_table_is_running_and_no_more() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let co_gm = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    insert_test_world_member(&mut conn, world, co_gm, "GM");
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();
    switch_on(&mut conn, owner, world, book.id).unwrap();

    let (title, entries) = entries_served_by(&mut conn, world, book.id, None).unwrap();
    assert_eq!(title, "Monster Manual");
    assert_eq!(entries.len(), 1);

    assert!(
        store::entries_for(&mut conn, co_gm, book.id, None).is_err(),
        "the library itself must still refuse everyone but the owner"
    );
    assert!(store::library_for(&mut conn, co_gm).unwrap().is_empty());
}

/// A player at the table is not trusted with its books. They may see the list
/// — every member may — and they have no route to change it.
#[test]
fn a_player_cannot_change_the_list() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    insert_test_world_member(&mut conn, world, player, "Player");
    let book = store::import_book(&mut conn, owner, a_book("Monster Manual", SYSTEM), &[]).unwrap();
    switch_on(&mut conn, owner, world, book.id).unwrap();

    assert!(matches!(
        switch_off(&mut conn, player, world, book.id),
        Err(BookListError::MayNotManageBooks)
    ));
    assert!(matches!(
        switch_off_report(&mut conn, player, world, book.id),
        Err(BookListError::MayNotManageBooks)
    ));
    assert!(matches!(
        offerable_to(&mut conn, player, world),
        Err(BookListError::MayNotManageBooks)
    ));
    assert_eq!(
        books_on(&mut conn, world).unwrap().len(),
        1,
        "and the list a player reads is the real one"
    );
}

/// Spec 050 decision 8 and FR-010a, one role at a time: the Owner, a Game
/// Master and a Trusted Player may each switch a book on and off, a Player
/// and a stranger may not, and whoever switches, the book that goes on is the
/// **owner's** — each manager also holds a book of their own with the same
/// title and system, and it is never the one offered or accepted.
#[test]
fn who_may_arrange_the_books_and_whose_books_they_are() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let owners_book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();

    let member = |conn: &mut PgConnection, role: &str| {
        let user = insert_test_user(conn);
        insert_test_world_member(conn, world, user, role);
        user
    };
    let game_master = member(&mut conn, "GM");
    let trusted = member(&mut conn, "TrustedPlayer");
    let player = member(&mut conn, "Player");
    let stranger = insert_test_user(&mut conn);

    for (who, caller, may) in [
        ("Owner", owner, true),
        ("Game Master", game_master, true),
        ("Trusted Player", trusted, true),
        ("Player", player, false),
        ("stranger", stranger, false),
    ] {
        // Their own copy of the same book, so a wrong shelf cannot pass by
        // coincidence of title or system.
        let their_own = (caller != owner).then(|| {
            store::import_book(&mut conn, caller, a_book("Monster Manual", SYSTEM), &[]).unwrap()
        });

        let offered = offerable_to(&mut conn, caller, world);
        let switched = switch_on(&mut conn, caller, world, owners_book.id);

        if !may {
            assert!(
                matches!(offered, Err(BookListError::MayNotManageBooks)),
                "{who}: {offered:?}"
            );
            assert!(
                matches!(switched, Err(BookListError::MayNotManageBooks)),
                "{who}: {switched:?}"
            );
            assert!(books_on(&mut conn, world).unwrap().is_empty(), "{who}");
            continue;
        }

        let offered: Vec<Uuid> = offered
            .unwrap_or_else(|e| panic!("{who}: {e}"))
            .into_iter()
            .map(|book| book.id)
            .collect();
        assert_eq!(
            offered,
            vec![owners_book.id],
            "{who} is offered the owner's shelf"
        );

        let on = switched.unwrap_or_else(|e| panic!("{who}: {e}"));
        assert_eq!(on.compendium_id, owners_book.id, "{who}");
        assert_eq!(
            on.switched_on_by,
            Some(caller),
            "FR-015 records who, for {who}"
        );

        if let Some(their_own) = their_own {
            assert!(
                matches!(
                    switch_on(&mut conn, caller, world, their_own.id),
                    Err(BookListError::NotOnTheOwnersShelf)
                ),
                "{who} must not stock the table from their own shelf"
            );
        }

        let report = switch_off_report(&mut conn, caller, world, owners_book.id)
            .unwrap_or_else(|e| panic!("{who}: {e}"));
        assert_eq!(report.entry_count, 1, "{who}");
        switch_off(&mut conn, caller, world, owners_book.id)
            .unwrap_or_else(|e| panic!("{who}: {e}"));
        assert!(books_on(&mut conn, world).unwrap().is_empty(), "{who}");
    }

    // And a refused caller cannot take off what somebody trusted put on.
    switch_on(&mut conn, trusted, world, owners_book.id).unwrap();
    for refused in [player, stranger] {
        assert!(matches!(
            switch_off_report(&mut conn, refused, world, owners_book.id),
            Err(BookListError::MayNotManageBooks)
        ));
        assert!(matches!(
            switch_off(&mut conn, refused, world, owners_book.id),
            Err(BookListError::MayNotManageBooks)
        ));
    }
    assert_eq!(books_on(&mut conn, world).unwrap().len(), 1);
}

/// The migration's half of ADR-099: the column accepts `TrustedPlayer` and
/// still refuses the spellings a person would plausibly type instead, so a
/// seed script cannot write a role that the code would then read as nobody.
#[test]
fn a_misspelt_trusted_player_is_refused_by_the_column() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner, SYSTEM);
    let someone = insert_test_user(&mut conn);

    let written = diesel::sql_query(format!(
        "INSERT INTO world_members (id, world_id, user_id, role) \
         VALUES ('{}', '{}', '{}', 'trustedplayer')",
        Uuid::now_v7(),
        world,
        someone,
    ))
    .execute(&mut conn);

    assert!(
        written.is_err(),
        "the column must refuse a spelling the code does not recognise"
    );
    assert!(matches!(
        offerable_to(&mut conn, someone, world),
        Err(BookListError::MayNotManageBooks)
    ));
}

/// 050 FR-060: removing a book from the library must name the worlds it is on
/// for, so this is the lookup that answer comes from.
#[test]
fn the_worlds_running_a_book_can_be_named() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let first = a_world_running(&mut conn, owner, SYSTEM);
    let second = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(&mut conn, owner, a_book("Monster Manual", SYSTEM), &[]).unwrap();
    switch_on(&mut conn, owner, first, book.id).unwrap();
    switch_on(&mut conn, owner, second, book.id).unwrap();

    let worlds_on: Vec<Uuid> = worlds_with_book(&mut conn, book.id)
        .unwrap()
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    assert_eq!(worlds_on.len(), 2);
    assert!(worlds_on.contains(&first) && worlds_on.contains(&second));
}

/// FR-061 and FR-028, from both directions: a compendium going takes the
/// links to it, a world going takes its own list, and neither reaches the
/// other's content.
#[test]
fn removing_either_end_takes_only_the_link() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let kept = a_world_running(&mut conn, owner, SYSTEM);
    let doomed = a_world_running(&mut conn, owner, SYSTEM);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", SYSTEM),
        &[a_creature("Goblin")],
    )
    .unwrap();
    switch_on(&mut conn, owner, kept, book.id).unwrap();
    switch_on(&mut conn, owner, doomed, book.id).unwrap();

    diesel::delete(worlds::table.filter(worlds::id.eq(doomed)))
        .execute(&mut conn)
        .unwrap();
    assert_eq!(worlds_with_book(&mut conn, book.id).unwrap().len(), 1);
    assert_eq!(
        entry_count(&mut conn, book.id),
        1,
        "a world going takes no content"
    );

    store::remove(&mut conn, owner, book.id).unwrap();
    assert!(books_on(&mut conn, kept).unwrap().is_empty());
    assert!(worlds_with_book(&mut conn, book.id).unwrap().is_empty());
}

/// Switching a book on does not tie a person to a world for ever. A Trusted
/// Player switches the owner's book on and changes an entry in it, then
/// deletes their account: the deletion succeeds, the book stays switched on,
/// the change stays made, and the records say only that somebody did it.
#[test]
fn a_person_who_switched_a_book_on_can_still_delete_their_account() {
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
    let trusted = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, world, trusted, "TrustedPlayer");

    let on = switch_on(&mut conn, trusted, world, book.id).unwrap();
    assert_eq!(on.switched_on_by, Some(trusted));
    crate::library::deltas::hide_entry(&mut conn, trusted, world, book.id, "creature", "Goblin")
        .unwrap();

    crate::users::delete_user_data_on(&mut conn, trusted)
        .expect("the account deletion must not be refused by a book they switched on");

    let still = books_on(&mut conn, world).unwrap();
    assert_eq!(still.len(), 1, "the book stays switched on");
    assert_eq!(still[0].row.switched_on_by, None);
    let held = crate::library::deltas::deltas_over(&mut conn, world, book.id, None).unwrap();
    assert_eq!(held.len(), 1, "and the change they made stays made");
    assert_eq!(held[0].changed_by, None);
}
