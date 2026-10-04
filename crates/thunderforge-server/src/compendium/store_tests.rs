//! What the store must be true about, against a real database.
//!
//! Three of these prove properties the schema enforces rather than the Rust
//! does — origin's immutability, prose with no mechanical fields, a page
//! number that is one-based. They are written as attempts to break the rule
//! from outside this module, because that is the only way to find out whether
//! the rule is in the database or merely in the discipline of the code that
//! happens to be calling it today.

use super::*;
use crate::content::{NameState, ReadValue};
use crate::test_support::{insert_test_user, test_app_state};

fn a_book(title: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        // A distinct hash per book, since one account may not hold the same
        // file twice.
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: "test-system".to_string(),
        parser_version: "reader-test".to_string(),
        page_count: 320,
        silent_page_count: 0,
    }
}

/// An anchored entry with one field read, one doubted, and one looked for and
/// not found — the three states, in the one entry.
fn a_creature() -> Entry {
    Entry {
        kind: "creature".to_string(),
        name: "Goblin".to_string(),
        name_state: NameState::Clear,
        page: 166,
        values: [
            ("armour".to_string(), ReadValue::Clear("15".to_string())),
            (
                "hits".to_string(),
                ReadValue::Uncertain("7 (2d6)".to_string()),
            ),
            ("speed".to_string(), ReadValue::Unread),
        ]
        .into_iter()
        .collect(),
        text: None,
        suspect: false,
        extras: None,
    }
}

fn a_feat() -> Entry {
    Entry {
        kind: "feat".to_string(),
        name: "Alert".to_string(),
        name_state: NameState::Uncertain,
        page: 165,
        values: Default::default(),
        text: Some("You gain a +5 bonus to initiative.".to_string()),
        suspect: true,
        extras: None,
    }
}

/// FR-051: nobody is asked, and nothing in the signature could have carried
/// an answer. The write path states the origin itself.
#[test]
fn an_import_is_uploaded_without_anybody_being_asked() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);

    let book = import_book(&mut conn, owner, a_book("Monster Manual"), &[a_creature()]).unwrap();

    assert_eq!(book.origin, ContentOrigin::Uploaded);
    assert!(!book.origin.may_be_shared());
}

/// FR-057, and the reason the migration carries a trigger rather than trusting
/// this module's restraint: the database itself refuses the flip, so going
/// around the store does not work either.
#[test]
fn origin_cannot_be_updated_even_by_raw_sql() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = import_book(&mut conn, owner, a_book("Player's Handbook"), &[]).unwrap();

    let flipped = diesel::sql_query(format!(
        "UPDATE compendiums SET origin = 'Authored' WHERE id = '{}'",
        book.id
    ))
    .execute(&mut conn);

    assert!(
        flipped.is_err(),
        "the database must refuse to turn uploaded content into authored content"
    );
    assert_eq!(
        load(&mut conn, owner, book.id).unwrap().origin,
        ContentOrigin::Uploaded,
        "and the row must be unchanged after the refusal"
    );
}

/// An update that does not touch origin still works — the trigger refuses a
/// change to that column, not every write to the table. Worth asserting,
/// because a trigger that refused everything would pass the test above while
/// making the shelf read-only.
#[test]
fn a_book_can_still_be_renamed() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = import_book(&mut conn, owner, a_book("Untitled scan"), &[]).unwrap();

    rename(&mut conn, owner, book.id, "Tome of Beasts").unwrap();

    assert_eq!(
        load(&mut conn, owner, book.id).unwrap().book_title,
        "Tome of Beasts"
    );
}

/// FR-002, all the way into storage: a field that was looked for and not found
/// carries **no value**. Read back as raw JSON rather than through the Rust
/// type, because the Rust type cannot express the failure being guarded
/// against — the question is whether the stored document invented one.
#[test]
fn an_unread_field_is_stored_with_no_value_at_all() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = import_book(&mut conn, owner, a_book("Monster Manual"), &[a_creature()]).unwrap();

    let stored = entries_for(&mut conn, owner, book.id, None).unwrap();
    let values = &stored[0].field_values;

    assert_eq!(values["armour"]["state"], "clear");
    assert_eq!(values["armour"]["value"], "15");
    assert_eq!(values["hits"]["state"], "uncertain");
    // The text is kept exactly as read, never coerced into a number.
    assert_eq!(values["hits"]["value"], "7 (2d6)");

    assert_eq!(values["speed"]["state"], "unread");
    assert!(
        values["speed"].get("value").is_none(),
        "an unread field must carry no value: not a null, not an empty string, not a zero"
    );
}

/// FR-001b: a prose entry has nowhere to put a mechanical field, and the
/// constraint is in the table rather than in the importer's good manners.
#[test]
fn a_prose_entry_may_not_carry_mechanical_fields() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = import_book(&mut conn, owner, a_book("Player's Handbook"), &[a_feat()]).unwrap();

    let stored = entries_for(&mut conn, owner, book.id, None).unwrap();
    assert_eq!(
        stored[0].prose_text.as_deref(),
        Some("You gain a +5 bonus to initiative.")
    );
    assert_eq!(stored[0].field_values, serde_json::json!({}));
    // FR-004: the reader's distrust reaches the shelf, not only the review.
    assert!(stored[0].suspect);
    assert!(stored[0].name_uncertain);

    let smuggled = diesel::sql_query(format!(
        "INSERT INTO compendium_entries (id, compendium_id, kind, name, page, field_values, prose_text) \
         VALUES ('{}', '{}', 'feat', 'Alert', 165, '{{\"armour\": {{\"state\": \"clear\", \"value\": \"15\"}}}}'::jsonb, 'prose')",
        Uuid::now_v7(),
        book.id
    ))
    .execute(&mut conn);

    assert!(
        smuggled.is_err(),
        "prose with mechanical fields must be refused by the table itself"
    );
}

/// FR-032: an import applies completely or not at all. Induced by a page
/// number the table refuses — the entry is written inside the same
/// transaction as the compendium, so the whole book fails to land.
#[test]
fn a_failure_partway_leaves_no_compendium_at_all() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);

    let mut impossible = a_creature();
    impossible.page = 0;
    let book = a_book("Half a book");
    let hash = book.source_hash.clone();

    let failed = import_book(&mut conn, owner, book, &[a_creature(), impossible]);

    assert!(failed.is_err());
    assert!(
        find_by_source_hash(&mut conn, owner, &hash)
            .unwrap()
            .is_none(),
        "a failed import must leave the shelf exactly as it was"
    );
    assert!(library_for(&mut conn, owner).unwrap().is_empty());
}

/// FR-041: the counts are counted from what is being written, so the library
/// cannot describe a book as holding something it does not.
#[test]
fn counts_per_kind_come_from_the_entries_themselves() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);

    let book = import_book(
        &mut conn,
        owner,
        a_book("Mixed"),
        &[a_creature(), a_creature(), a_feat()],
    )
    .unwrap();

    assert_eq!(
        book.entry_counts,
        serde_json::json!({"creature": 2, "feat": 1})
    );
    assert_eq!(
        entries_for(&mut conn, owner, book.id, Some("feat"))
            .unwrap()
            .len(),
        1
    );
}

/// FR-047, against **the account's** library: the same account may not hold
/// the same file twice, and a second account may hold it freely. The second
/// half is decision 1 — two Game Masters with the same book get two
/// compendiums, deliberately, because the alternative is one store of parsed
/// commercial books served across accounts.
#[test]
fn a_file_is_unique_to_an_account_and_not_across_accounts() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let somebody_else = insert_test_user(&mut conn);

    let book = a_book("Dungeon Master's Guide");
    let hash = book.source_hash.clone();
    import_book(&mut conn, owner, book.clone(), &[]).unwrap();

    assert!(
        import_book(&mut conn, owner, book.clone(), &[]).is_err(),
        "one account may not hold the same file twice"
    );
    assert!(
        import_book(&mut conn, somebody_else, book, &[]).is_ok(),
        "another account's identical file is its own compendium"
    );

    assert!(
        find_by_source_hash(&mut conn, owner, &hash)
            .unwrap()
            .is_some()
    );
    assert!(
        find_by_source_hash(&mut conn, owner, &format!("{:0>64}", 1))
            .unwrap()
            .is_none(),
        "a file this account has not read in is not a match"
    );
}

/// FR-055a: a shelf is one account's. Every read is gated, and a stranger
/// gets the same refusal whichever door they try.
#[test]
fn another_account_can_read_neither_the_book_nor_its_entries() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let book = import_book(&mut conn, owner, a_book("Mine"), &[a_creature()]).unwrap();

    assert!(load(&mut conn, stranger, book.id).is_err());
    assert!(entries_for(&mut conn, stranger, book.id, None).is_err());
    assert!(rename(&mut conn, stranger, book.id, "Yours").is_err());
    assert!(remove(&mut conn, stranger, book.id).is_err());
    assert!(
        library_for(&mut conn, stranger).unwrap().is_empty(),
        "a library lists only its own account's books"
    );

    // And the owner's book is untouched by any of those attempts.
    assert_eq!(load(&mut conn, owner, book.id).unwrap().book_title, "Mine");
}

/// FR-044: removal takes that import's contribution and nothing else.
#[test]
fn removal_takes_one_book_and_leaves_the_other_intact() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let first = import_book(&mut conn, owner, a_book("First"), &[a_creature()]).unwrap();
    let second = import_book(&mut conn, owner, a_book("Second"), &[a_feat()]).unwrap();

    remove(&mut conn, owner, first.id).unwrap();

    assert!(load(&mut conn, owner, first.id).is_err());
    assert_eq!(
        entries_for(&mut conn, owner, second.id, None)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(library_for(&mut conn, owner).unwrap().len(), 1);
}

/// FR-047's overwrite: a re-read replaces what the bucket holds and leaves
/// the bucket — and its origin — where it was.
#[test]
fn re_reading_replaces_the_contents_and_not_the_origin() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let book = import_book(&mut conn, owner, a_book("Corrected"), &[a_creature()]).unwrap();

    replace_entries(
        &mut conn,
        owner,
        book.id,
        "reader-test-2",
        &[a_feat(), a_feat()],
    )
    .unwrap();

    let after = load(&mut conn, owner, book.id).unwrap();
    assert_eq!(after.origin, ContentOrigin::Uploaded);
    assert_eq!(after.parser_version, "reader-test-2");
    assert_eq!(after.entry_counts, serde_json::json!({"feat": 2}));

    let entries = entries_for(&mut conn, owner, book.id, None).unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.kind == "feat"));
}

/// FR-005: a book that was part scans says so on the shelf, not only in the
/// review window that is long since closed.
#[test]
fn silent_pages_are_remembered() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);

    let mut book = a_book("Half scanned");
    book.page_count = 300;
    book.silent_page_count = 100;
    let stored = import_book(&mut conn, owner, book, &[]).unwrap();

    assert_eq!(stored.page_count, 300);
    assert_eq!(stored.silent_page_count, 100);

    // And a book cannot claim to have been silent on pages it does not have.
    let mut impossible = a_book("Impossible");
    impossible.page_count = 10;
    impossible.silent_page_count = 11;
    assert!(import_book(&mut conn, owner, impossible, &[]).is_err());
}
