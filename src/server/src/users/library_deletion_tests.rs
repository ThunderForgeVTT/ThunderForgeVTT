//! Deleting an account takes its whole library, and nothing of it survives
//! anywhere (spec 049 T089, T090; spec 050 FR-062 to FR-064).
//!
//! Checked by reading the tables afterwards for anything that still names the
//! deleted books, rather than by trusting the cascades the migrations
//! declare: a cascade that was never there is invisible until something
//! looks.

use diesel::prelude::*;
use uuid::Uuid;

use crate::compendium::collections;
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState, ReadValue};
use crate::library::book_list::switch_on;
use crate::library::deltas::{self, Content};
use crate::schema::{compendium_entries, compendiums, world_books, world_entry_deltas, worlds};
use crate::test_support::{
    insert_test_actor, insert_test_scene, insert_test_user, insert_test_world,
    insert_test_world_member, test_app_state,
};

const SYSTEM: &str = "test-system";

fn a_book(conn: &mut PgConnection, owner: Uuid, title: &str) -> store::Compendium {
    store::import_book(
        conn,
        owner,
        NewBook {
            book_title: title.to_string(),
            source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
            system_id: SYSTEM.to_string(),
            parser_version: "reader-test".to_string(),
            page_count: 10,
            silent_page_count: 0,
        },
        &[Entry {
            kind: "creature".to_string(),
            name: "Goblin".to_string(),
            name_state: NameState::Clear,
            page: 3,
            values: [("hits".to_string(), ReadValue::Clear("7".to_string()))]
                .into_iter()
                .collect(),
            text: None,
            suspect: false,
            extras: None,
        }],
    )
    .unwrap()
}

fn a_world_on_the_system(conn: &mut PgConnection, owner: Uuid) -> Uuid {
    let world = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world)))
        .set(worlds::game_system_id.eq(Some(SYSTEM.to_string())))
        .execute(conn)
        .unwrap();
    world
}

/// How many rows anywhere still name one of `books`.
fn rows_naming(conn: &mut PgConnection, books: &[Uuid], titles: &[&str]) -> [i64; 4] {
    [
        compendiums::table
            .filter(compendiums::id.eq_any(books))
            .count()
            .get_result(conn)
            .unwrap(),
        compendium_entries::table
            .filter(compendium_entries::compendium_id.eq_any(books))
            .count()
            .get_result(conn)
            .unwrap(),
        world_books::table
            .filter(world_books::compendium_id.eq_any(books))
            .count()
            .get_result(conn)
            .unwrap(),
        world_entry_deltas::table
            .filter(
                world_entry_deltas::compendium_id
                    .eq_any(books.iter().map(|id| Some(*id)).collect::<Vec<_>>())
                    .or(world_entry_deltas::written_beside_title.eq_any(titles)),
            )
            .count()
            .get_result(conn)
            .unwrap(),
    ]
}

/// **T089, FR-062, FR-064.** An account with a book no world ever used, a
/// collection, and a book switched on in its world with a change and an
/// addition over it. After the account is deleted, not one row anywhere names
/// any of them — no base, no entry, no link, no delta, no kept addition — and
/// another account's book is exactly where it was.
#[test]
fn deleting_an_account_leaves_nothing_of_its_library() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let bystander = insert_test_user(&mut conn);

    // Titles unique to this run: the test database is shared, and another
    // test's kept addition beside its own "Monster Manual" is not this one's.
    let run = Uuid::now_v7().simple().to_string();
    let titles = [
        format!("Book Nobody Opened {run}"),
        format!("Monster Manual {run}"),
        format!("Fen Folk {run}"),
    ];
    let titles: Vec<&str> = titles.iter().map(String::as_str).collect();
    let unused = a_book(&mut conn, owner, titles[0]);
    let used = a_book(&mut conn, owner, titles[1]);
    let collection = collections::create_collection(&mut conn, owner, titles[2], SYSTEM).unwrap();
    collections::write_entry(
        &mut conn,
        owner,
        collection.id,
        "creature",
        "Mire Hag",
        Content::Prose("Lives in the fen.".to_string()),
    )
    .unwrap();
    let theirs = a_book(&mut conn, bystander, "Their Own Book");

    let world = a_world_on_the_system(&mut conn, owner);
    switch_on(&mut conn, owner, world, used.id).unwrap();
    switch_on(&mut conn, owner, world, collection.id).unwrap();
    deltas::change_entry(
        &mut conn,
        owner,
        world,
        used.id,
        "creature",
        "Goblin",
        Content::Fields(
            [("hits".to_string(), ReadValue::Clear("12".to_string()))]
                .into_iter()
                .collect(),
        ),
    )
    .unwrap();
    deltas::add_entry(
        &mut conn,
        owner,
        world,
        used.id,
        "creature",
        "Bog Wight",
        Content::Prose("It was a traveller once.".to_string()),
    )
    .unwrap();

    let books = [unused.id, used.id, collection.id];
    assert_eq!(rows_naming(&mut conn, &books, &titles), [3, 3, 2, 2]);

    crate::users::delete_user_data_on(&mut conn, owner).unwrap();

    assert_eq!(
        rows_naming(&mut conn, &books, &titles),
        [0, 0, 0, 0],
        "bases, entries, book-list links and deltas: nothing names the deleted library"
    );
    assert_eq!(
        store::entries_for(&mut conn, bystander, theirs.id, None)
            .unwrap()
            .len(),
        1,
        "another account's book is untouched"
    );
}

/// **T090, FR-063.** The world goes with its owner, and the player's character
/// is copied to the player first, as it always was — but nothing of the book
/// the world was running comes with it. The player's shelf is empty, none of
/// their worlds lists the book, and none holds a change over it.
#[test]
fn a_rescued_character_brings_no_book_with_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let player = insert_test_user(&mut conn);

    let title = format!("Monster Manual {}", Uuid::now_v7().simple());
    let book = a_book(&mut conn, owner, &title);
    let campaign = a_world_on_the_system(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, campaign, owner);
    insert_test_world_member(&mut conn, campaign, player, "Player");
    switch_on(&mut conn, owner, campaign, book.id).unwrap();
    insert_test_actor(&mut conn, campaign, scene, player);

    crate::users::delete_user_data_on(&mut conn, owner).unwrap();

    let players_worlds: Vec<Uuid> = worlds::table
        .filter(worlds::created_by.eq(player))
        .select(worlds::id)
        .load(&mut conn)
        .unwrap();
    assert!(
        !players_worlds.is_empty(),
        "the character was rescued into a world of the player's"
    );
    assert!(
        store::library_for(&mut conn, player).unwrap().is_empty(),
        "no book moved to the player's shelf"
    );
    let linked: i64 = world_books::table
        .filter(world_books::world_id.eq_any(&players_worlds))
        .count()
        .get_result(&mut conn)
        .unwrap();
    let changed: i64 = world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq_any(&players_worlds))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!((linked, changed), (0, 0), "and no world of theirs reads it");
    assert_eq!(
        rows_naming(&mut conn, &[book.id], &[title.as_str()]),
        [0, 0, 0, 0]
    );
}
