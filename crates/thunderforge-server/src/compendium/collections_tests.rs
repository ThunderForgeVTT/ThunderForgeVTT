//! Collections on the shelf, against a real database (spec 049 T083 to T087,
//! spec 050 FR-007 to FR-009c).
//!
//! Most of what these defend is sameness — a collection on the shelf, in a
//! book list and under a world's changes, exactly as a book is — and the rest
//! is the handful of places origin makes them differ. Both halves are checked
//! through the functions every route uses, and the differences once more
//! against the table itself.

use super::*;
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState, ReadValue};
use crate::library::book_list::{BookListError, offerable_to, switch_on};
use crate::library::deltas::{self, EntryState};
use crate::schema::worlds;
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};

const SYSTEM: &str = "test-system";

fn clear(value: &str) -> ReadValue {
    ReadValue::Clear(value.to_string())
}

fn fields(pairs: &[(&str, &str)]) -> Content {
    Content::Fields(
        pairs
            .iter()
            .map(|(field, value)| (field.to_string(), clear(value)))
            .collect(),
    )
}

fn a_world_on(conn: &mut PgConnection, owner: Uuid, system: &str) -> Uuid {
    let world = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world)))
        .set(worlds::game_system_id.eq(Some(system.to_string())))
        .execute(conn)
        .unwrap();
    world
}

fn a_book(conn: &mut PgConnection, owner: Uuid) -> Compendium {
    store::import_book(
        conn,
        owner,
        NewBook {
            book_title: "Monster Manual".to_string(),
            source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
            system_id: SYSTEM.to_string(),
            parser_version: "reader-test".to_string(),
            page_count: 320,
            silent_page_count: 0,
        },
        &[Entry {
            kind: "creature".to_string(),
            name: "Goblin".to_string(),
            name_state: NameState::Clear,
            page: 166,
            values: [("armour".to_string(), clear("15"))].into_iter().collect(),
            text: None,
            suspect: false,
            extras: None,
        }],
    )
    .unwrap()
}

/// A collection with a creature and a prose feat in it.
fn a_collection(conn: &mut PgConnection, owner: Uuid) -> Compendium {
    let collection = create_collection(conn, owner, "Fen Folk", SYSTEM).unwrap();
    write_entry(
        conn,
        owner,
        collection.id,
        "creature",
        "Mire Hag",
        fields(&[("armour", "17"), ("hits", "52")]),
    )
    .unwrap();
    write_entry(
        conn,
        owner,
        collection.id,
        "feat",
        "Bog-Born",
        Content::Prose("You do not sink.".to_string()),
    )
    .unwrap();
    store::load(conn, owner, collection.id).unwrap()
}

/// **T083, FR-007.** A collection sits on the shelf beside a book: listed by
/// the same call, authored, with no file hash, and at a version that moved on
/// with each entry written.
#[test]
fn a_collection_is_on_the_same_shelf_as_a_book() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = a_book(&mut conn, owner);
    let collection = a_collection(&mut conn, owner);

    let shelf: Vec<Uuid> = store::library_for(&mut conn, owner)
        .unwrap()
        .iter()
        .map(|held| held.id)
        .collect();
    assert!(shelf.contains(&book.id) && shelf.contains(&collection.id));

    assert_eq!(collection.origin, ContentOrigin::Authored);
    assert_eq!(collection.source_hash, None);
    assert_eq!(
        collection.base_version, 3,
        "created at 1, two entries written"
    );
    assert_eq!(
        collection.entry_counts,
        serde_json::json!({ "creature": 1, "feat": 1 })
    );
    let entries = store::entries_for(&mut conn, owner, collection.id, None).unwrap();
    assert!(
        entries.iter().all(|entry| entry.page.is_none()),
        "an entry written in a collection is on no page"
    );
}

/// What a collection refuses to be written: a second entry of one kind and
/// name, an unnamed one, and anything into an imported book or somebody
/// else's collection. Removing an entry is a version too.
#[test]
fn writing_a_collection_refuses_what_it_should() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let book = a_book(&mut conn, owner);
    let collection = a_collection(&mut conn, owner);

    let twin = write_entry(
        &mut conn,
        owner,
        collection.id,
        "creature",
        "Mire Hag",
        fields(&[]),
    );
    assert!(
        matches!(twin, Err(CollectionError::AlreadyHas { .. })),
        "{twin:?}"
    );
    let unnamed = write_entry(&mut conn, owner, collection.id, " ", "x", fields(&[]));
    assert!(
        matches!(unnamed, Err(CollectionError::Unnamed)),
        "{unnamed:?}"
    );
    let into_a_book = write_entry(&mut conn, owner, book.id, "creature", "Orc", fields(&[]));
    assert!(
        matches!(into_a_book, Err(CollectionError::ImportedBook { .. })),
        "{into_a_book:?}"
    );
    let theirs = write_entry(
        &mut conn,
        stranger,
        collection.id,
        "creature",
        "Imp",
        fields(&[]),
    );
    assert!(
        matches!(theirs, Err(CollectionError::NotYours(_))),
        "{theirs:?}"
    );
    assert!(matches!(
        create_collection(&mut conn, owner, "   ", SYSTEM),
        Err(CollectionError::Untitled)
    ));

    let hag = store::entries_for(&mut conn, owner, collection.id, Some("creature"))
        .unwrap()
        .remove(0);
    remove_entry(&mut conn, owner, collection.id, hag.id).unwrap();
    let after = store::load(&mut conn, owner, collection.id).unwrap();
    assert_eq!(after.base_version, 4);
    assert_eq!(after.entry_counts, serde_json::json!({ "feat": 1 }));
    assert!(matches!(
        remove_entry(&mut conn, owner, collection.id, hag.id),
        Err(CollectionError::NoSuchEntry)
    ));
}

/// **T083, T084, FR-008, FR-009.** In a world a collection is a book: offered
/// only to a world on its system and refused by any other, switched on the
/// same way, read the same way, and changed by a world the same way — with
/// the one difference origin makes, that a world's change over an authored
/// entry is authored too.
#[test]
fn in_a_world_a_collection_behaves_as_a_book_does() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let collection = a_collection(&mut conn, owner);
    let on_its_system = a_world_on(&mut conn, owner, SYSTEM);
    let on_another = a_world_on(&mut conn, owner, "another-system");

    let offered = |conn: &mut PgConnection, world| -> Vec<Uuid> {
        offerable_to(conn, owner, world)
            .unwrap()
            .iter()
            .map(|book| book.id)
            .collect()
    };
    assert!(offered(&mut conn, on_its_system).contains(&collection.id));
    assert!(!offered(&mut conn, on_another).contains(&collection.id));
    assert!(matches!(
        switch_on(&mut conn, owner, on_another, collection.id),
        Err(BookListError::SystemMismatch { .. })
    ));

    switch_on(&mut conn, owner, on_its_system, collection.id).unwrap();
    let (title, read) =
        deltas::world_reads(&mut conn, on_its_system, collection.id, None, false).unwrap();
    assert_eq!(title, "Fen Folk");
    assert_eq!(read.entries.len(), 2);
    assert!(read.entries.iter().all(|entry| entry.page.is_none()
        && entry.origin == ContentOrigin::Authored
        && entry.state == EntryState::Inherited));

    let changed = deltas::change_entry(
        &mut conn,
        owner,
        on_its_system,
        collection.id,
        "creature",
        "Mire Hag",
        Content::Fields([("hits".to_string(), clear("60"))].into_iter().collect()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(changed.origin, ContentOrigin::Authored);
    let (_, read) =
        deltas::world_reads(&mut conn, on_its_system, collection.id, None, false).unwrap();
    let hag = read
        .entries
        .iter()
        .find(|entry| entry.name == "Mire Hag")
        .unwrap();
    assert_eq!(hag.state, EntryState::Changed);
    assert_eq!(hag.field_values["hits"], serde_json::json!(clear("60")));
    assert!(hag.origin.may_be_shared());
}

/// **T085, T086, FR-009a, FR-009b.** The owner's collection downloads with
/// every entry and says nothing was left out; an imported book is refused
/// before any of it is read; somebody else's collection is refused.
#[test]
fn a_collection_downloads_and_a_book_does_not() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let book = a_book(&mut conn, owner);
    let collection = a_collection(&mut conn, owner);

    let file = download(&mut conn, owner, collection.id).unwrap();
    assert_eq!(file.format, FILE_FORMAT);
    assert_eq!(file.title, "Fen Folk");
    assert_eq!(file.system_id, SYSTEM);
    assert_eq!(file.version, 3);
    assert!(file.excluded.is_empty());
    assert_eq!(
        file.entries,
        vec![
            FileEntry {
                kind: "creature".to_string(),
                name: "Mire Hag".to_string(),
                field_values: serde_json::json!({
                    "armour": clear("17"),
                    "hits": clear("52"),
                }),
                prose_text: None,
            },
            FileEntry {
                kind: "feat".to_string(),
                name: "Bog-Born".to_string(),
                field_values: serde_json::json!({}),
                prose_text: Some("You do not sink.".to_string()),
            },
        ]
    );
    let json = serde_json::to_value(&file).unwrap();
    assert_eq!(json["excluded"], serde_json::json!([]), "said, not implied");
    assert_eq!(json["entries"][1]["proseText"], "You do not sink.");

    assert!(matches!(
        download(&mut conn, owner, book.id),
        Err(CollectionError::NotDownloadable { .. })
    ));
    assert!(matches!(
        download(&mut conn, stranger, collection.id),
        Err(CollectionError::NotYours(_))
    ));
}

/// **T087, FR-009c.** Anything that is not authored is left out of the file
/// and named with why — never silently dropped. No entry of a collection can
/// be uploaded today, so the rule is held on the function that decides it.
#[test]
fn a_download_names_what_it_left_out() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let collection = a_collection(&mut conn, owner);
    let mut entries: Vec<(StoredEntry, ContentOrigin)> =
        store::entries_for(&mut conn, owner, collection.id, None)
            .unwrap()
            .into_iter()
            .map(|entry| (entry, ContentOrigin::Authored))
            .collect();
    entries[0].1 = ContentOrigin::Uploaded;
    let left_out = entries[0].0.name.clone();

    let file = compose_file(&collection, entries);
    assert_eq!(file.entries.len(), 1);
    assert!(file.entries.iter().all(|entry| entry.name != left_out));
    let [excluded] = file.excluded.as_slice() else {
        panic!("one exclusion expected: {:?}", file.excluded);
    };
    assert_eq!(excluded.name, left_out);
    assert!(excluded.reason.contains("read in"));
}

/// The database holds what origin decides, without this module: a hash on a
/// collection, a book with no hash, a page-less entry in a book and a paged
/// entry in a collection are each refused.
#[test]
fn the_database_ties_hash_and_page_to_origin() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = a_book(&mut conn, owner);
    let collection = a_collection(&mut conn, owner);

    let run = |conn: &mut PgConnection, sql: String| diesel::sql_query(sql).execute(conn);

    assert!(
        run(
            &mut conn,
            format!(
                "UPDATE compendiums SET source_hash = 'abc' WHERE id = '{}'",
                collection.id
            )
        )
        .is_err(),
        "a collection read from no file cannot claim one"
    );
    assert!(
        run(
            &mut conn,
            format!(
                "UPDATE compendiums SET source_hash = NULL WHERE id = '{}'",
                book.id
            )
        )
        .is_err(),
        "a book cannot lose the hash of its file"
    );
    let entry = |compendium: Uuid, page: &str| {
        format!(
            "INSERT INTO compendium_entries (id, compendium_id, kind, name, page) \
             VALUES ('{}', '{compendium}', 'creature', 'Imp', {page})",
            Uuid::now_v7()
        )
    };
    assert!(
        run(&mut conn, entry(book.id, "NULL")).is_err(),
        "a book entry names its page"
    );
    assert!(
        run(&mut conn, entry(collection.id, "12")).is_err(),
        "a collection entry is on no page"
    );
}
