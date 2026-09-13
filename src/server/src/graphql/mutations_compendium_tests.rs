//! The four checks the server makes on arrival, each proved by a payload that
//! fails it, plus what a removal and a re-import do (spec 049 T041, T047,
//! T048).
//!
//! Every test here stands its own system up in a temporary directory and
//! writes the manifest by hand. That is not only to avoid naming a bundled
//! system in shared code — it is what makes "re-read from the manifest"
//! testable at all: the payload and the manifest can be made to disagree,
//! which is exactly the case FR-015 is about.

use std::collections::BTreeMap;

use super::*;
use crate::content::ReadValue;
use crate::graphql::queries::compendium::GraphQLContentOrigin;
use crate::test_support::{insert_test_user, test_app_state};

/// A system that declares one anchored kind, written to disk the way a pack
/// writes one.
struct DeclaredSystem {
    root: std::path::PathBuf,
    id: String,
}

impl DeclaredSystem {
    fn with_kinds(kinds: &[&str]) -> Self {
        let id = format!("test-system-{}", Uuid::now_v7().simple());
        let root = std::env::temp_dir().join(format!("tf-systems-{}", Uuid::now_v7().simple()));
        let dir = root.join(&id);
        std::fs::create_dir_all(&dir).expect("failed to make a temporary system directory");

        let patterns: Vec<serde_json::Value> = kinds
            .iter()
            .map(|kind| {
                serde_json::json!({
                    "kind": kind,
                    "shape": "anchored",
                    "anchor": "Armour Class",
                    "name": { "position": "before", "withinLines": 6, "prefer": "largest" },
                    "fields": [{ "key": "armourClass", "label": "Armour Class", "as": "integer" }],
                })
            })
            .collect();

        std::fs::write(
            dir.join("system.json"),
            serde_json::json!({ "id": id, "contentPatterns": patterns }).to_string(),
        )
        .expect("failed to write a temporary system manifest");

        Self { root, id }
    }

    fn dir(&self) -> &str {
        self.root.to_str().expect("a temporary path that is UTF-8")
    }
}

impl Drop for DeclaredSystem {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).ok();
    }
}

fn an_entry(kind: &str, name: &str) -> ImportedEntryInput {
    let mut values = BTreeMap::new();
    values.insert(
        "armourClass".to_string(),
        ReadValue::Clear("15".to_string()),
    );
    ImportedEntryInput {
        kind: kind.to_string(),
        name: name.to_string(),
        name_uncertain: false,
        page: 12,
        values: Some(Json(values)),
        text: None,
        suspect: false,
        extras: None,
    }
}

/// How many of one kind a book says it holds.
fn counted(book: &GraphQLCompendium, kind: &str) -> i32 {
    book.entry_counts
        .iter()
        .find(|count| count.kind == kind)
        .map(|count| count.count)
        .unwrap_or_default()
}

fn a_hash() -> String {
    format!("{:0>64}", Uuid::now_v7().simple())
}

fn an_import(
    system: &DeclaredSystem,
    entries: Vec<ImportedEntryInput>,
) -> CreateCompendiumFromImportInput {
    CreateCompendiumFromImportInput {
        book_title: "A Book Somebody Read".to_string(),
        source_hash: a_hash(),
        system_id: system.id.clone(),
        parser_version: "test".to_string(),
        page_count: 200,
        silent_page_count: 3,
        replaces_compendium_id: None,
        entries,
    }
}

/// The ordinary case, so the refusals below mean something.
#[tokio::test]
async fn a_reviewed_book_becomes_a_compendium_the_caller_owns() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let book = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(
            &system,
            vec![
                an_entry("creature", "Adult Red Dragon"),
                an_entry("creature", "Goblin"),
            ],
        ),
    )
    .await
    .expect("a payload that passes all four checks is imported");

    assert_eq!(counted(&book, "creature"), 2);
    assert_eq!(book.entry_total, 2);
    // FR-051: the origin nothing could send is the one that was written.
    assert_eq!(book.origin, GraphQLContentOrigin::Uploaded);
}

/// Check two: a payload naming a system that declares nothing is refused, and
/// refused **because the manifest says so**, not because the payload did.
#[tokio::test]
async fn a_payload_that_lies_about_its_system_is_refused() {
    let state = test_app_state();
    let declared = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let mut lying = an_import(&declared, vec![an_entry("creature", "Goblin")]);
    lying.system_id = format!("not-a-system-{}", Uuid::now_v7().simple());

    let refused = create_compendium_from_import_impl(&state, declared.dir(), owner, lying).await;

    let message = refused
        .expect_err("a system with no manifest cannot be read into")
        .message;
    assert!(
        message.contains("does not say what its content looks like"),
        "the refusal must say why: {message}"
    );
}

/// Check four: a kind the manifest does not name is a payload this flow did
/// not produce, however plausible it looks.
#[tokio::test]
async fn an_undeclared_kind_is_refused() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let refused = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(
            &system,
            vec![
                an_entry("creature", "Goblin"),
                an_entry("spell", "Fireball"),
            ],
        ),
    )
    .await;

    let message = refused.expect_err("an undeclared kind is refused").message;
    assert!(
        message.contains("spell"),
        "the refusal must name the kind it did not recognise: {message}"
    );
}

/// Check three: the browser refuses first, and the server refuses again, with
/// the bound named (FR-035).
#[tokio::test]
async fn a_payload_over_the_bound_is_refused_with_the_bound_named() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let too_many = (0..=MAX_ENTRIES_PER_IMPORT)
        .map(|n| an_entry("creature", &format!("Goblin {n}")))
        .collect();

    let refused = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, too_many),
    )
    .await;

    let message = refused
        .expect_err("one past the bound is past the bound")
        .message;
    assert!(
        message.contains(&MAX_ENTRIES_PER_IMPORT.to_string()),
        "the bound must be named in the refusal: {message}"
    );
}

/// Check one: an overwrite aimed at somebody else's shelf is refused, and
/// refused by the ownership helper rather than by an inline comparison.
#[tokio::test]
async fn an_overwrite_from_the_wrong_account_is_refused() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let (owner, stranger) = {
        let mut conn = state.db_pool.get().unwrap();
        (insert_test_user(&mut conn), insert_test_user(&mut conn))
    };

    let theirs = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the owner's own import stands");

    let mut over_theirs = an_import(&system, vec![an_entry("creature", "Something Else")]);
    over_theirs.replaces_compendium_id = Some(theirs.id);
    over_theirs.source_hash = theirs.source_hash.clone();

    let refused =
        create_compendium_from_import_impl(&state, system.dir(), stranger, over_theirs).await;

    assert!(
        refused.is_err(),
        "one account must not be able to write over another account's book"
    );

    // And it is still the owner's book, unchanged.
    let still_theirs = compendium_for_file_hash_impl(&state, owner, theirs.source_hash)
        .await
        .expect("the owner can look for their own book")
        .expect("it is still there");
    assert_eq!(still_theirs.entry_total, 1);
}

/// FR-047 and spec 047 FR-070 to FR-072: the same file is recognised before
/// anything is sent, and against the **account's** library.
#[tokio::test]
async fn the_same_file_is_recognised_and_a_near_identical_one_is_not() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let (owner, somebody_else) = {
        let mut conn = state.db_pool.get().unwrap();
        (insert_test_user(&mut conn), insert_test_user(&mut conn))
    };

    let book = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the first import stands");

    assert!(
        compendium_for_file_hash_impl(&state, owner, book.source_hash.clone())
            .await
            .unwrap()
            .is_some(),
        "the file this account has already read in is recognised by its hash"
    );

    // Spec 047 FR-075: the same work re-saved is a different file. One
    // character of difference in the hash is not a match, and claiming it
    // were would overwrite a book with a different one.
    let near_identical = {
        let mut hash = book.source_hash.clone();
        hash.replace_range(0..1, if hash.starts_with('a') { "b" } else { "a" });
        hash
    };
    assert!(
        compendium_for_file_hash_impl(&state, owner, near_identical)
            .await
            .unwrap()
            .is_none(),
        "a near-identical file must not be claimed as a match"
    );

    // FR-055a: another account's library is not searched, so the same file in
    // two libraries is two books and neither account learns of the other.
    assert!(
        compendium_for_file_hash_impl(&state, somebody_else, book.source_hash)
            .await
            .unwrap()
            .is_none(),
        "the hash check is against this account's library and no other"
    );
}

/// FR-047's overwrite: the same file, read again, replaces what the book
/// holds without becoming a second book.
#[tokio::test]
async fn a_re_import_of_the_same_file_overwrites_in_place() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let first = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the first import stands");

    let mut again = an_import(
        &system,
        vec![
            an_entry("creature", "Goblin"),
            an_entry("creature", "Adult Red Dragon"),
        ],
    );
    again.source_hash = first.source_hash.clone();
    again.replaces_compendium_id = Some(first.id);
    again.parser_version = "test-improved".to_string();

    let replaced = create_compendium_from_import_impl(&state, system.dir(), owner, again)
        .await
        .expect("re-reading the same file replaces what the book holds");

    assert_eq!(replaced.id, first.id, "the shelf entry is the same book");
    assert_eq!(
        counted(&replaced, "creature"),
        2,
        "the base beneath the book was replaced by the fresh read"
    );
    assert_eq!(replaced.parser_version, "test-improved");
    // FR-057: nothing about a re-read can change where the content came from.
    assert_eq!(replaced.origin, GraphQLContentOrigin::Uploaded);
}

/// The second import of the same file with no overwrite named is refused, and
/// the refusal says what to do instead.
#[tokio::test]
async fn importing_the_same_file_twice_is_refused_rather_than_duplicated() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let first = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the first import stands");

    let mut again = an_import(&system, vec![an_entry("creature", "Goblin")]);
    again.source_hash = first.source_hash;

    let message = create_compendium_from_import_impl(&state, system.dir(), owner, again)
        .await
        .expect_err("the same file is not imported twice")
        .message;
    assert!(
        message.contains("already imported"),
        "the refusal must say the account already has this book: {message}"
    );
}

/// A re-read of a *different* file may not be pushed over an existing book:
/// the shelf would go on naming the first file's hash (spec 047 FR-075).
#[tokio::test]
async fn an_overwrite_with_a_different_file_is_refused() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let first = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the first import stands");

    // A fresh hash: a different printing, or the same book re-saved.
    let mut different = an_import(&system, vec![an_entry("creature", "Goblin")]);
    different.replaces_compendium_id = Some(first.id);

    let message = create_compendium_from_import_impl(&state, system.dir(), owner, different)
        .await
        .expect_err("a different file is not an overwrite")
        .message;
    assert!(
        message.contains("not the file"),
        "the refusal must say the file is not the one the book was read from: {message}"
    );
}

/// FR-045: the report names what would go, and changes nothing.
#[tokio::test]
async fn asking_what_a_removal_would_take_changes_nothing() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let book = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(
            &system,
            vec![
                an_entry("creature", "Goblin"),
                an_entry("creature", "Adult Red Dragon"),
            ],
        ),
    )
    .await
    .expect("the import stands");

    let report = remove_compendium_impl(&state, owner, book.id, false)
        .await
        .expect("the owner may ask");

    assert_eq!(report.entry_count, 2);
    assert!(!report.removed, "asking is not removing");
    assert!(
        compendium_for_file_hash_impl(&state, owner, book.source_hash)
            .await
            .unwrap()
            .is_some(),
        "the book is still on the shelf after the report"
    );
}

/// FR-044: confirming takes that import's contribution, and nothing else.
#[tokio::test]
async fn confirming_removes_that_import_and_leaves_the_other_alone() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let owner = insert_test_user(&mut state.db_pool.get().unwrap());

    let going = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the first import stands");
    let staying = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Adult Red Dragon")]),
    )
    .await
    .expect("the second import stands");

    let confirmed = remove_compendium_impl(&state, owner, going.id, true)
        .await
        .expect("the owner may remove their own book");
    assert!(confirmed.removed);

    assert!(
        compendium_for_file_hash_impl(&state, owner, going.source_hash)
            .await
            .unwrap()
            .is_none(),
        "the removed book is gone"
    );
    let survivor = compendium_for_file_hash_impl(&state, owner, staying.source_hash)
        .await
        .unwrap()
        .expect("the other book is untouched");
    assert_eq!(survivor.entry_total, 1);
}

/// Another account cannot remove, and cannot learn that the book exists by
/// being refused differently.
#[tokio::test]
async fn a_stranger_cannot_remove_or_report_on_somebody_elses_book() {
    let state = test_app_state();
    let system = DeclaredSystem::with_kinds(&["creature"]);
    let (owner, stranger) = {
        let mut conn = state.db_pool.get().unwrap();
        (insert_test_user(&mut conn), insert_test_user(&mut conn))
    };

    let book = create_compendium_from_import_impl(
        &state,
        system.dir(),
        owner,
        an_import(&system, vec![an_entry("creature", "Goblin")]),
    )
    .await
    .expect("the import stands");

    let reported = remove_compendium_impl(&state, stranger, book.id, false).await;
    let removed = remove_compendium_impl(&state, stranger, book.id, true).await;
    let missing = remove_compendium_impl(&state, stranger, Uuid::now_v7(), false).await;

    assert!(reported.is_err() && removed.is_err());
    assert_eq!(
        reported.unwrap_err().message,
        missing.unwrap_err().message,
        "a book that is not yours and one that does not exist must refuse identically"
    );
    assert!(
        compendium_for_file_hash_impl(&state, owner, book.source_hash)
            .await
            .unwrap()
            .is_some(),
        "the refused removal removed nothing"
    );
}

/// FR-002 across the wire: a field the reader looked for and did not find has
/// nowhere to carry a value, so no payload can invent one.
#[test]
fn an_unread_field_carrying_a_value_does_not_deserialise() {
    let unread: Result<ReadValue, _> = serde_json::from_value(serde_json::json!({
        "state": "unread",
        "value": "15",
    }));
    assert!(
        unread.is_err(),
        "a payload claiming a field was not found and supplying its value is not a reading"
    );
}

/// A page number is one-based because the point of keeping one is looking it
/// up in the physical book.
#[test]
fn a_page_that_is_not_a_page_is_refused() {
    let mut entry = an_entry("creature", "Goblin");
    entry.page = 0;
    assert!(entry_from(entry).is_err());

    let mut negative = an_entry("creature", "Goblin");
    negative.page = -3;
    assert!(entry_from(negative).is_err());
}
