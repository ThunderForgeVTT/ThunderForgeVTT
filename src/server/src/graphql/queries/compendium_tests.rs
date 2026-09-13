//! What the library read surface must be true about, driven through the real
//! schema against a real database (spec 049 T050).
//!
//! These execute GraphQL documents rather than calling the resolvers as
//! functions, because the properties being proved are properties of what a
//! client can ask for: that one account's document returns another account's
//! book is not a claim about a function's return type.

use super::*;
use crate::auth_middleware::AuthenticatedUser;
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState, ReadValue};
use crate::test_support::{insert_test_user, insert_test_world, test_app_state};
use async_graphql::Request;

fn schema(state: crate::state::AppState) -> crate::graphql::AppSchema {
    async_graphql::Schema::build(
        crate::graphql::QueryRoot::default(),
        crate::graphql::MutationRoot::default(),
        crate::graphql::SubscriptionRoot,
    )
    .data(state)
    .finish()
}

fn as_user(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        user_id,
        session_id: Uuid::now_v7(),
        expires_at: chrono::Utc::now().naive_utc() + chrono::Duration::hours(1),
        is_admin: false,
        role: "User".to_string(),
        disabled: false,
    }
}

/// An admin, for the one question worth asking of one: an operator is another
/// account, and FR-055a makes no exception for them.
fn as_admin(user_id: Uuid) -> AuthenticatedUser {
    AuthenticatedUser {
        is_admin: true,
        role: "Admin".to_string(),
        ..as_user(user_id)
    }
}

fn a_book(title: &str, hash: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        source_hash: hash.to_string(),
        system_id: "test-system".to_string(),
        parser_version: "reader-test".to_string(),
        page_count: 320,
        silent_page_count: 4,
    }
}

/// A hash that is the right shape for a SHA-256 and belongs to this test
/// alone. Two ids, because one is half the hex a real hash has.
fn a_hash() -> String {
    format!("{}{}", Uuid::now_v7().simple(), Uuid::now_v7().simple())
}

fn creature(name: &str, page: u32) -> Entry {
    Entry {
        kind: "creature".to_string(),
        name: name.to_string(),
        name_state: NameState::Clear,
        page,
        values: [
            ("armour".to_string(), ReadValue::Clear("15".to_string())),
            ("speed".to_string(), ReadValue::Unread),
        ]
        .into_iter()
        .collect(),
        text: None,
        suspect: false,
        extras: None,
    }
}

fn feat(name: &str, page: u32) -> Entry {
    Entry {
        kind: "feat".to_string(),
        name: name.to_string(),
        name_state: NameState::Uncertain,
        page,
        values: Default::default(),
        text: Some("You gain a +5 bonus to initiative.".to_string()),
        suspect: true,
        extras: None,
    }
}

async fn ask(
    state: &crate::state::AppState,
    user: AuthenticatedUser,
    document: &str,
) -> serde_json::Value {
    let response = schema(state.clone())
        .execute(Request::new(document).data(user))
        .await;
    assert!(response.errors.is_empty(), "{:?}", response.errors);
    response.data.into_json().expect("json")
}

const LIBRARY: &str = "{ myLibrary { id bookTitle systemId origin pageCount silentPageCount \
                        sourceHash importedAt entryTotal entryCounts { kind count } } }";

/// 049 US3 acceptance 1, and 050 US1 acceptance 1: two books are two
/// compendiums, each with its own name, date and counts — not one
/// undifferentiated pile.
#[tokio::test]
async fn two_books_are_two_compendiums_with_their_own_counts() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let owner = insert_test_user(&mut conn);
    store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", &a_hash()),
        &[creature("Goblin", 166), creature("Owlbear", 249)],
    )
    .expect("first import");
    store::import_book(
        &mut conn,
        owner,
        a_book("Player's Handbook", &a_hash()),
        &[feat("Alert", 165)],
    )
    .expect("second import");
    drop(conn);

    let data = ask(&state, as_user(owner), LIBRARY).await;
    let shelf = data["myLibrary"].as_array().expect("shelf");
    assert_eq!(shelf.len(), 2, "two imports, two books on the shelf");

    // Newest first, so the book just read is the one at the top.
    assert_eq!(shelf[0]["bookTitle"], "Player's Handbook");
    assert_eq!(shelf[1]["bookTitle"], "Monster Manual");

    let manual = &shelf[1];
    assert_eq!(manual["entryTotal"], 2);
    assert_eq!(
        manual["entryCounts"],
        serde_json::json!([{ "kind": "creature", "count": 2 }]),
        "each book's counts are its own",
    );
    assert_eq!(
        shelf[0]["entryCounts"],
        serde_json::json!([{ "kind": "feat", "count": 1 }]),
    );

    // FR-051: origin is what the server wrote, and every book here is one a
    // person uploaded.
    assert!(shelf.iter().all(|book| book["origin"] == "UPLOADED"));
    // FR-005: what the shelf must still be able to say months later.
    assert_eq!(manual["silentPageCount"], 4);
    assert!(manual["importedAt"].as_str().expect("a date").contains('T'));
}

/// The guard T032 asks for, stated where it can actually be enforced: a
/// person reaches their own shelf and nobody else's, by any of the three
/// reads, and an operator is not an exception (FR-055a).
#[tokio::test]
async fn another_account_reaches_none_of_it() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let operator = insert_test_user(&mut conn);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Somebody Else's Monster Manual", &a_hash()),
        &[creature("Goblin", 166)],
    )
    .expect("import");
    drop(conn);

    let document = format!(
        "{{ myLibrary {{ id }} \
           compendium(id: \"{id}\") {{ id bookTitle }} \
           compendiumEntries(compendiumId: \"{id}\") {{ total entries {{ name }} }} }}",
        id = book.id,
    );

    for caller in [as_user(stranger), as_admin(operator)] {
        let who = if caller.is_admin {
            "an operator"
        } else {
            "a stranger"
        };
        let data = ask(&state, caller, &document).await;
        assert_eq!(
            data["myLibrary"].as_array().expect("shelf").len(),
            0,
            "{who} has an empty shelf of their own",
        );
        assert_eq!(
            data["compendium"],
            serde_json::Value::Null,
            "{who} must not be able to open another account's book",
        );
        assert_eq!(
            data["compendiumEntries"]["total"], 0,
            "{who} must not be able to read another account's entries",
        );
    }

    // And the same document, asked by the person it belongs to, answers — so
    // the refusals above are about who is asking and not about a broken query.
    let mine = ask(&state, as_user(owner), &document).await;
    assert_eq!(
        mine["compendium"]["bookTitle"],
        "Somebody Else's Monster Manual"
    );
    assert_eq!(mine["compendiumEntries"]["total"], 1);
}

/// A book that does not exist and a book that is not yours must be
/// indistinguishable, or an id space can be walked for what other people hold.
#[tokio::test]
async fn a_missing_book_and_somebody_elses_answer_identically() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let owner = insert_test_user(&mut conn);
    let stranger = insert_test_user(&mut conn);
    let book =
        store::import_book(&mut conn, owner, a_book("Xanathar's", &a_hash()), &[]).expect("import");
    drop(conn);

    let answer_for = |id: Uuid| {
        let state = state.clone();
        async move {
            ask(
                &state,
                as_user(stranger),
                &format!("{{ compendium(id: \"{id}\") {{ id }} }}"),
            )
            .await
        }
    };

    assert_eq!(
        answer_for(book.id).await,
        answer_for(Uuid::now_v7()).await,
        "somebody else's book and no book at all must read the same",
    );
}

/// FR-042 and FR-043: browse one book by kind, and every entry names the book
/// it came from and the page it was found on.
#[tokio::test]
async fn entries_browse_by_kind_and_name_their_book_and_page() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let owner = insert_test_user(&mut conn);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", &a_hash()),
        &[creature("Goblin", 166), feat("Alert", 165)],
    )
    .expect("import");
    drop(conn);

    let data = ask(
        &state,
        as_user(owner),
        &format!(
            "{{ compendiumEntries(compendiumId: \"{}\", kind: \"creature\") \
               {{ total entries {{ name kind page bookTitle compendiumId fieldValues \
                  nameUncertain suspect }} }} }}",
            book.id
        ),
    )
    .await;

    let page = &data["compendiumEntries"];
    assert_eq!(page["total"], 1, "the kind filter excludes the feat");
    let entry = &page["entries"][0];
    assert_eq!(entry["name"], "Goblin");
    assert_eq!(entry["page"], 166, "the page it was found on (FR-043)");
    assert_eq!(
        entry["bookTitle"], "Monster Manual",
        "and the book it came from (FR-043)",
    );
    assert_eq!(entry["compendiumId"], book.id.to_string());

    // FR-002: a field looked for and not found carries no value, and crosses
    // the wire still carrying none.
    assert_eq!(entry["fieldValues"]["armour"]["value"], "15");
    assert_eq!(entry["fieldValues"]["speed"]["state"], "unread");
    assert!(entry["fieldValues"]["speed"].get("value").is_none());
}

/// A book holds thousands, so a client is handed a page at a time — and
/// walking every page yields every entry exactly once.
#[tokio::test]
async fn entries_paginate_without_repeating_or_losing_one() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let owner = insert_test_user(&mut conn);
    // Two entries share a name deliberately: name alone is not a total order,
    // and a page boundary landing on a tie is where a cursor goes wrong.
    let mut entries: Vec<Entry> = (0..25)
        .map(|n| creature(&format!("Goblin {n:02}"), 1))
        .collect();
    entries.push(creature("Goblin 07", 2));
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Monster Manual", &a_hash()),
        &entries,
    )
    .expect("import");
    drop(conn);

    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    let mut pages = 0;
    loop {
        let after = cursor
            .as_ref()
            .map(|c| format!(", after: \"{c}\""))
            .unwrap_or_default();
        let data = ask(
            &state,
            as_user(owner),
            &format!(
                "{{ compendiumEntries(compendiumId: \"{}\", first: 10{after}) \
                   {{ total nextCursor entries {{ id name }} }} }}",
                book.id
            ),
        )
        .await;
        let page = &data["compendiumEntries"];
        assert_eq!(page["total"], 26);
        for entry in page["entries"].as_array().expect("entries") {
            seen.push(entry["id"].as_str().expect("id").to_string());
        }
        pages += 1;
        assert!(pages <= 5, "paging must terminate");
        match page["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_string()),
            None => break,
        }
    }

    assert_eq!(pages, 3, "26 entries at 10 a page is three pages");
    assert_eq!(seen.len(), 26, "every entry was handed out");
    let mut unique = seen.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), 26, "and none of them twice");
}

/// The point of hashing against the account rather than a world: the Game
/// Master who read a book while running one world is told they already have
/// it when they go to read it again for another (FR-047, 050 FR-004).
///
/// The two worlds are in this test because the requirement is about them, and
/// what it proves is that neither of them appears in the answer: the shelf
/// the duplicate is recognised against is reached without naming a world at
/// all, so there is no world-scoped read for a second world to miss.
#[tokio::test]
async fn a_re_import_is_recognised_from_a_different_world() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().expect("conn");
    let master = insert_test_user(&mut conn);
    let first_world = insert_test_world(&mut conn, master);
    let second_world = insert_test_world(&mut conn, master);
    assert_ne!(first_world, second_world);

    let hash = a_hash();
    store::import_book(
        &mut conn,
        master,
        a_book("Monster Manual", &hash),
        &[creature("Goblin", 166)],
    )
    .expect("read while running the first world");
    drop(conn);

    // Sitting in the second world, about to read the same file in again.
    let data = ask(&state, as_user(master), LIBRARY).await;
    let shelf = data["myLibrary"].as_array().expect("shelf");
    let already_held: Vec<&serde_json::Value> = shelf
        .iter()
        .filter(|book| book["sourceHash"] == serde_json::json!(hash))
        .collect();

    assert_eq!(
        already_held.len(),
        1,
        "the file is recognised from the shelf, whichever world the Game Master is in",
    );
    assert_eq!(already_held[0]["bookTitle"], "Monster Manual");

    // Spec 047 FR-075: a near-identical file — the same work re-saved — is a
    // different file and must not be claimed as a match, or a book would be
    // overwritten by a different one.
    let nearly = format!("{}f", &hash[..hash.len() - 1]);
    assert_ne!(nearly, hash);
    assert!(
        !shelf
            .iter()
            .any(|book| book["sourceHash"] == serde_json::json!(nearly)),
        "a different file is not this file",
    );
}
