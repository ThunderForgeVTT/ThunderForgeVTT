//! A collection's earlier versions, against a real database (spec 049 T096,
//! spec 050 FR-104, ADR-098).

use super::*;
use crate::compendium::collections::{create_collection, remove_entry, write_entry};
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState, ReadValue};
use crate::library::deltas::Content;
use crate::test_support::{insert_test_user, test_app_state};

const SYSTEM: &str = "test-system";

fn armour(value: &str) -> Content {
    Content::Fields(
        [("armour".to_string(), ReadValue::Clear(value.to_string()))]
            .into_iter()
            .collect(),
    )
}

fn names(entries: &[VersionEntry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.name.as_str()).collect()
}

fn version_of(conn: &mut PgConnection, id: Uuid) -> i32 {
    compendiums::table
        .filter(compendiums::id.eq(id))
        .select(compendiums::base_version)
        .first(conn)
        .unwrap()
}

/// **FR-104.** Every write and removal keeps the version it replaced, and a
/// restore is a new version, so it can be regretted too.
#[test]
fn every_change_keeps_the_version_it_replaced_and_a_restore_is_one_more() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let collection = create_collection(&mut conn, owner, "Fen Folk", SYSTEM).unwrap();

    write_entry(
        &mut conn,
        owner,
        collection.id,
        "creature",
        "Mire Hag",
        armour("17"),
    )
    .unwrap();
    let eel = write_entry(
        &mut conn,
        owner,
        collection.id,
        "creature",
        "Bog Eel",
        armour("12"),
    )
    .unwrap();
    remove_entry(&mut conn, owner, collection.id, eel.id).unwrap();
    assert_eq!(version_of(&mut conn, collection.id), 4);

    let history = history(&mut conn, owner, collection.id).unwrap();
    let described: Vec<(i32, &str)> = history
        .iter()
        .map(|past| (past.version, past.replaced_by.as_str()))
        .collect();
    assert_eq!(
        described,
        vec![
            (3, "Took out creature \"Bog Eel\""),
            (2, "Wrote creature \"Bog Eel\""),
            (1, "Wrote creature \"Mire Hag\""),
        ]
    );
    assert_eq!(history[0].entry_total, 2);

    assert!(
        read_at(&mut conn, owner, collection.id, At::Version(1))
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        names(&read_at(&mut conn, owner, collection.id, At::Version(3)).unwrap()),
        vec!["Bog Eel", "Mire Hag"]
    );
    assert_eq!(
        names(&read_at(&mut conn, owner, collection.id, At::Current).unwrap()),
        vec!["Mire Hag"]
    );

    let restored = restore(&mut conn, owner, collection.id, 3).unwrap();
    assert_eq!(restored.base_version, 5, "a restore moves on, never back");
    assert_eq!(restored.entry_counts, serde_json::json!({ "creature": 2 }));
    let current = read_at(&mut conn, owner, collection.id, At::Current).unwrap();
    assert_eq!(names(&current), vec!["Bog Eel", "Mire Hag"]);
    assert_eq!(
        current[0].field_values,
        read_at(&mut conn, owner, collection.id, At::Version(3)).unwrap()[0].field_values
    );

    // The restore is regrettable: what it replaced is version 4, kept.
    let latest = &history_of(&mut conn, owner, collection.id)[0];
    assert_eq!(
        (latest.version, latest.replaced_by.as_str()),
        (4, "Went back to version 3")
    );
    restore(&mut conn, owner, collection.id, 4).unwrap();
    assert_eq!(
        names(&read_at(&mut conn, owner, collection.id, At::Current).unwrap()),
        vec!["Mire Hag"]
    );

    assert!(matches!(
        restore(&mut conn, owner, collection.id, 99),
        Err(CollectionError::NoSuchVersion { version: 99 })
    ));
}

fn history_of(conn: &mut PgConnection, owner: Uuid, id: Uuid) -> Vec<PastVersion> {
    history(conn, owner, id).unwrap()
}

/// **ADR-098 condition 1.** Nobody but the owner lists, reads or restores an
/// earlier version, and a refused restore changes nothing.
#[test]
fn only_the_owner_reaches_an_earlier_version() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let collection = create_collection(&mut conn, owner, "Fen Folk", SYSTEM).unwrap();
    write_entry(
        &mut conn,
        owner,
        collection.id,
        "creature",
        "Mire Hag",
        armour("17"),
    )
    .unwrap();

    assert!(matches!(
        history(&mut conn, stranger, collection.id),
        Err(CollectionError::NotYours(_))
    ));
    for at in [At::Current, At::Version(1)] {
        assert!(matches!(
            read_at(&mut conn, stranger, collection.id, at),
            Err(CollectionError::NotYours(_))
        ));
    }
    assert!(matches!(
        restore(&mut conn, stranger, collection.id, 1),
        Err(CollectionError::NotYours(_))
    ));
    assert_eq!(version_of(&mut conn, collection.id), 2);
    assert_eq!(history_of(&mut conn, owner, collection.id).len(), 1);
}

/// **FR-101.** An imported book has no earlier versions, and the database
/// refuses one however it is asked — which is what stops a sync back into a
/// book from any route, since a sync back writes its version first.
#[test]
fn the_database_refuses_a_version_of_an_imported_book() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = store::import_book(
        &mut conn,
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
            values: Default::default(),
            text: None,
            suspect: false,
            extras: None,
        }],
    )
    .unwrap();

    let refused = record(&mut conn, book.id, "Synced from anywhere").unwrap_err();
    assert!(
        refused.to_string().contains("FR-101"),
        "refused by the trigger, not by chance: {refused}"
    );

    let raw = diesel::sql_query(format!(
        "INSERT INTO shelf_collection_versions (id, compendium_id, version, book_title, entries, entry_counts, replaced_by) \
         VALUES ('{}', '{}', 1, 'x', '[]', '{{}}', 'x')",
        Uuid::now_v7(),
        book.id
    ))
    .execute(&mut conn);
    assert!(raw.is_err(), "raw SQL is refused as well");

    assert!(history_of(&mut conn, owner, book.id).is_empty());
    assert!(matches!(
        restore(&mut conn, owner, book.id, 1),
        Err(CollectionError::NoSuchVersion { version: 1 })
    ));
}

/// **ADR-098 condition 3, pinned.** An earlier version's entries are read in
/// one place, [`read_at`], so a moderation check put there withholds content
/// from every version at once. This fails the day a second read appears —
/// through Diesel or through SQL — so that read is looked at rather than
/// slipping past the condition.
#[test]
fn an_earlier_version_is_read_in_one_function() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stack = vec![root.clone()];
    let mut readers = Vec::new();
    while let Some(dir) = stack.pop() {
        for item in std::fs::read_dir(&dir).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let file = path.file_name().unwrap().to_string_lossy().to_string();
            if !file.ends_with(".rs") || file.ends_with("tests.rs") || file == "schema.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let shown = path.strip_prefix(&root).unwrap().display().to_string();
            if source.contains("shelf_collection_versions::entries") {
                readers.push(format!("{shown} (entries)"));
            }
            if source
                .to_lowercase()
                .contains("from shelf_collection_versions")
            {
                readers.push(format!("{shown} (SQL)"));
            }
        }
    }
    readers.sort();
    // `record` writes the column; `read_at` is the only read of it.
    assert_eq!(
        readers,
        vec!["compendium/versions.rs (entries)".to_string()]
    );

    let versions = std::fs::read_to_string(root.join("compendium/versions.rs")).unwrap();
    let uses: Vec<usize> = versions
        .match_indices("shelf_collection_versions::entries")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(
        uses.len(),
        2,
        "one write in `record`, one read in `read_at`"
    );
    let read_at_begins = versions.find("pub fn read_at(").unwrap();
    let read_at_ends = read_at_begins + versions[read_at_begins..].find("\n}\n").unwrap();
    let record_begins = versions.find("pub(crate) fn record(").unwrap();
    assert!(
        uses[0] > record_begins && uses[0] < read_at_begins,
        "the write is in `record`"
    );
    assert!(
        (read_at_begins..read_at_ends).contains(&uses[1]),
        "the read is in `read_at`"
    );
}
