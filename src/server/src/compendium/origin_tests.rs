//! The invariant, attempted from outside every route (spec 049 FR-054a,
//! FR-056, ADR-097).
//!
//! Written as raw SQL on purpose, as `store_tests` writes its attempts on
//! origin's immutability: the claim is that a collection cannot contain
//! uploaded content **by any route**, and the only way to find out whether a
//! rule lives in the database or merely in the discipline of today's callers
//! is to go around the callers. These run as the database superuser the test
//! suite connects as, which is also the answer to "can an operator with psql
//! do it?" — no.

use super::*;
use crate::collections::MEMBER_TYPES;
use crate::compendium::store::{NewBook, import_book};
use crate::content::{Entry, NameState};
use crate::test_support::{
    insert_test_ability, insert_test_actor, insert_test_item, insert_test_lore_entry,
    insert_test_scene, insert_test_user, insert_test_world, test_app_state,
};

fn a_book(title: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: "test-system".to_string(),
        parser_version: "reader-test".to_string(),
        page_count: 1,
        silent_page_count: 0,
    }
}

fn prose(name: &str, text: &str) -> Entry {
    Entry {
        kind: "prose".to_string(),
        name: name.to_string(),
        name_state: NameState::Clear,
        page: 1,
        values: Default::default(),
        text: Some(text.to_string()),
        suspect: false,
        extras: None,
    }
}

/// One book on `owner`'s shelf, and the id of the single entry in it.
fn an_uploaded_entry(conn: &mut PgConnection, owner: Uuid, entry: Entry) -> (Uuid, Uuid) {
    let book = import_book(conn, owner, a_book("Monster Manual"), &[entry]).expect("import");
    let entry_id = crate::schema::compendium_entries::table
        .filter(crate::schema::compendium_entries::compendium_id.eq(book.id))
        .select(crate::schema::compendium_entries::id)
        .first::<Uuid>(conn)
        .expect("the entry was stored");
    (book.id, entry_id)
}

fn a_collection(conn: &mut PgConnection, world: Uuid, owner: Uuid) -> Uuid {
    let id = Uuid::now_v7();
    diesel::sql_query(format!(
        "INSERT INTO world_collections (id, world_id, name, created_by, updated_by) \
         VALUES ('{id}', '{world}', 'A haunted manor', '{owner}', '{owner}')"
    ))
    .execute(conn)
    .expect("collection");
    id
}

/// The raw write a route would make, with nothing in front of it.
fn put_in_collection(
    conn: &mut PgConnection,
    collection: Uuid,
    member_type: &str,
    member_id: Uuid,
    added_by: Uuid,
) -> QueryResult<usize> {
    diesel::sql_query(format!(
        "INSERT INTO world_collection_members \
             (id, collection_id, member_type, member_id, added_by) \
         VALUES ('{}', '{collection}', '{member_type}', '{member_id}', '{added_by}')",
        Uuid::now_v7()
    ))
    .execute(conn)
}

fn members_of(conn: &mut PgConnection, collection: Uuid) -> i64 {
    crate::schema::world_collection_members::table
        .filter(crate::schema::world_collection_members::collection_id.eq(collection))
        .count()
        .get_result(conn)
        .expect("count")
}

/// FR-054a: the write itself is refused — not a route, not a resolver, the
/// INSERT — and it is refused as an origin refusal a route can phrase.
#[test]
fn a_collection_cannot_be_given_uploaded_content_even_by_raw_sql() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let collection = a_collection(&mut conn, world, owner);
    let (_, entry) = an_uploaded_entry(&mut conn, owner, prose("Goblin", "Small and cruel."));

    let refused = put_in_collection(
        &mut conn,
        collection,
        content_type::COMPENDIUM_ENTRY,
        entry,
        owner,
    )
    .expect_err("the database must refuse uploaded content into a collection");

    assert_eq!(
        refusal_from_database(&refused),
        Some(LeaveRefusal::Uploaded),
        "the refusal must be recognisable as origin, got: {refused}"
    );
    assert_eq!(members_of(&mut conn, collection), 0);
}

/// The rule restricts an origin, not a subject. The same collection, the same
/// raw write, and authored content goes straight in.
#[test]
fn authored_content_enters_a_collection_through_the_same_write() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let collection = a_collection(&mut conn, world, owner);
    let item = insert_test_item(&mut conn, world, owner);

    put_in_collection(&mut conn, collection, "item", item, owner)
        .expect("authored content must be accepted");
    assert_eq!(members_of(&mut conn, collection), 1);
}

/// The other way in: an existing, legitimate membership row rewritten to point
/// at uploaded content. A guard on INSERT alone would be a guard with a door
/// beside it.
#[test]
fn a_membership_cannot_be_repointed_at_uploaded_content() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let collection = a_collection(&mut conn, world, owner);
    let item = insert_test_item(&mut conn, world, owner);
    put_in_collection(&mut conn, collection, "item", item, owner).expect("authored");
    let (_, entry) = an_uploaded_entry(&mut conn, owner, prose("Goblin", "Small and cruel."));

    let refused = diesel::sql_query(format!(
        "UPDATE world_collection_members \
            SET member_type = 'compendium_entry', member_id = '{entry}' \
          WHERE collection_id = '{collection}'"
    ))
    .execute(&mut conn)
    .expect_err("repointing a member at uploaded content must be refused");
    assert_eq!(
        refusal_from_database(&refused),
        Some(LeaveRefusal::Uploaded)
    );
}

/// An origin nobody has established is refused, not waved through: a typo in
/// a member type, a type added next year without an arm, and an id that
/// resolves to nothing all fail closed.
#[test]
fn content_of_unknown_origin_is_refused_rather_than_presumed_authored() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let collection = a_collection(&mut conn, world, owner);
    let item = insert_test_item(&mut conn, world, owner);

    for (member_type, member_id) in [("spaceship", item), ("item", Uuid::now_v7())] {
        let refused = put_in_collection(&mut conn, collection, member_type, member_id, owner)
            .expect_err("an unestablished origin must be refused");
        assert_eq!(
            refusal_from_database(&refused),
            Some(LeaveRefusal::NotEstablished),
            "{member_type}: {refused}"
        );
    }
    assert_eq!(members_of(&mut conn, collection), 0);
}

/// Spec 026 keeps a membership row after its artifact is deleted, so the
/// collection stays openable. The guard must not turn that into a collection
/// that can no longer be reordered — it asks about *what* a row carries, and
/// a reorder does not change that.
#[test]
fn a_collection_whose_member_was_deleted_can_still_be_reordered() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let collection = a_collection(&mut conn, world, owner);
    let item = insert_test_item(&mut conn, world, owner);
    put_in_collection(&mut conn, collection, "item", item, owner).expect("authored");

    diesel::sql_query(format!("DELETE FROM world_items WHERE id = '{item}'"))
        .execute(&mut conn)
        .expect("delete the artifact");

    diesel::sql_query(format!(
        "UPDATE world_collection_members SET sort_order = 7 WHERE collection_id = '{collection}'"
    ))
    .execute(&mut conn)
    .expect("a reorder must not be refused for content that is merely gone");
}

/// The lookup answers per entry and for every member type a collection may
/// hold. Walks `MEMBER_TYPES` rather than a hand-written list, so a sixth type
/// added there without an arm in the SQL function fails here instead of being
/// refused in production with a message nobody can explain.
#[test]
fn every_kind_of_content_answers_for_its_own_origin() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let scene = insert_test_scene(&mut conn, world, owner);

    let authored = [
        ("actor", insert_test_actor(&mut conn, world, scene, owner)),
        ("item", insert_test_item(&mut conn, world, owner)),
        ("ability", insert_test_ability(&mut conn, world, owner)),
        ("lore", insert_test_lore_entry(&mut conn, world, owner)),
        ("scene", scene),
    ];
    for member_type in MEMBER_TYPES {
        let (_, id) = authored
            .iter()
            .find(|(kind, _)| kind == member_type)
            .unwrap_or_else(|| panic!("no fixture for member type {member_type}"));
        assert_eq!(
            origin_of(&mut conn, member_type, *id).unwrap(),
            Some(ContentOrigin::Authored),
            "{member_type}"
        );
    }

    let (book, entry) = an_uploaded_entry(&mut conn, owner, prose("Goblin", "Small and cruel."));
    assert_eq!(
        origin_of(&mut conn, content_type::COMPENDIUM_ENTRY, entry).unwrap(),
        Some(ContentOrigin::Uploaded)
    );
    assert_eq!(
        origin_of(&mut conn, content_type::COMPENDIUM, book).unwrap(),
        Some(ContentOrigin::Uploaded)
    );

    assert_eq!(origin_of(&mut conn, "item", Uuid::now_v7()).unwrap(), None);
    assert_eq!(origin_of(&mut conn, "spaceship", entry).unwrap(), None);
}

/// FR-056, T054: the licence of the uploaded document makes no difference.
///
/// The book here says, in every place a licence could be said — its title and
/// its own text — that it is openly licensed. It is still uploaded, and still
/// cannot enter a collection. There is nothing to configure to make it
/// otherwise: `NewBook` has no licence field and the guard reads none, which
/// is the point of deciding by origin (049 decision 4). The known cost this
/// accepts is recorded in FR-056a, and the refusal names the two routes left.
#[test]
fn an_openly_licensed_book_is_still_uploaded_and_still_cannot_leave() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = insert_test_world(&mut conn, owner);
    let collection = a_collection(&mut conn, world, owner);

    let book = import_book(
        &mut conn,
        owner,
        a_book("System Reference Document 5.1 (CC-BY-4.0)"),
        &[prose(
            "Legal Information",
            "This work is licensed under the Creative Commons Attribution 4.0 \
             International License. You are free to share and adapt it.",
        )],
    )
    .expect("import");
    assert_eq!(book.origin, ContentOrigin::Uploaded);

    let entry = crate::schema::compendium_entries::table
        .filter(crate::schema::compendium_entries::compendium_id.eq(book.id))
        .select(crate::schema::compendium_entries::id)
        .first::<Uuid>(&mut conn)
        .expect("entry");

    let refused = put_in_collection(
        &mut conn,
        collection,
        content_type::COMPENDIUM_ENTRY,
        entry,
        owner,
    )
    .expect_err("a permissive licence must not open the door");
    let refusal = refusal_from_database(&refused).expect("an origin refusal");
    assert_eq!(refusal, LeaveRefusal::Uploaded);
    assert!(refusal.message().contains("author it"));
    assert!(refusal.message().contains("system pack"));
    assert!(
        !refusal.message().to_lowercase().contains("licen"),
        "the refusal must not suggest a licence was weighed, got: {}",
        refusal.message()
    );
}

/// Every table a route writes to carry content outward has the guard on it.
///
/// Asked of the catalogue rather than by attempting an uploaded share link,
/// because those three tables' foreign keys already make uploaded content
/// unnameable in them today — the guard there is for the day a world artifact
/// can be uploaded, and this is what fails if somebody drops it before then.
#[test]
fn every_table_that_carries_content_outward_is_guarded() {
    #[derive(QueryableByName)]
    struct Guarded {
        #[diesel(sql_type = diesel::sql_types::Text)]
        table_name: String,
    }

    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let guarded: Vec<String> = diesel::sql_query(
        "SELECT c.relname::TEXT AS table_name \
           FROM pg_trigger t \
           JOIN pg_class c ON c.oid = t.tgrelid \
           JOIN pg_proc p ON p.oid = t.tgfoid \
          WHERE p.proname = 'refuse_content_that_may_not_leave' \
            AND NOT t.tgisinternal \
            AND t.tgenabled <> 'D' \
          ORDER BY 1",
    )
    .load::<Guarded>(&mut conn)
    .expect("catalogue")
    .into_iter()
    .map(|row| row.table_name)
    .collect();

    assert_eq!(
        guarded,
        [
            "world_ability_shares",
            "world_actor_shares",
            "world_collection_members",
            "world_item_shares",
        ]
    );
}
