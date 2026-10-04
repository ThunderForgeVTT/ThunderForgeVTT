//! What happens to a world's deltas when what they hang from goes: the book is
//! switched off, switched back on, removed from the shelf, or the world itself
//! goes — and whether a delta can leave the account by way of a collection.
//!
//! Spec 050 decision 5 is the rule most of these defend: changes and hides go
//! with the book, **additions stay**. Phase 11 first shipped the opposite, a
//! cascade that took a Game Master's own writing with the book, so the tests
//! here are written against the database rather than against this module's
//! discipline.

use super::tests::{Table, a_table, clear, fields, named, read};
use super::*;
use crate::library::book_list::{switch_off, switch_off_report, switch_on};
use crate::schema::worlds;
use crate::test_support::test_app_state;

/// The goblin changed, the orc hidden, a hag added — one of each form.
fn one_of_each(conn: &mut PgConnection, t: &Table) -> (Delta, Delta, Delta) {
    let changed = change_entry(
        conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap()
    .unwrap();
    let hidden = hide_entry(conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    let added = add_entry(
        conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        Content::Prose("Lives in the fen and bargains in teeth.".to_string()),
    )
    .unwrap();
    (changed, hidden, added)
}

fn held_ids(conn: &mut PgConnection, world: Uuid) -> Vec<Uuid> {
    world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(world))
        .order(world_entry_deltas::id)
        .select(world_entry_deltas::id)
        .load(conn)
        .unwrap()
}

/// **Decision 5.** The report says, before anything happens, that the change
/// and the hide will go and the addition will stay. After switching off, the
/// change and the hide are gone and the addition is still there: the same row,
/// still authored, still shareable, listed as this world's own, and removable
/// without switching the book back on.
#[test]
fn switching_a_book_off_takes_changes_and_hides_and_keeps_additions() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let (_, _, added) = one_of_each(&mut conn, &t);

    let report = switch_off_report(&mut conn, t.owner, t.world, t.book.id).unwrap();
    assert_eq!(
        report.deltas,
        vec![
            "changed: creature \"Goblin\"".to_string(),
            "hidden: creature \"Orc\"".to_string(),
        ],
        "what goes is named"
    );
    assert_eq!(
        report.additions_kept,
        vec!["added: creature \"Mire Hag\"".to_string()],
        "and what stays is named as staying"
    );
    assert_eq!(
        held_ids(&mut conn, t.world).len(),
        3,
        "asking takes nothing"
    );

    switch_off(&mut conn, t.owner, t.world, t.book.id).unwrap();

    assert_eq!(
        held_ids(&mut conn, t.world),
        vec![added.id],
        "the change and the hide went with the book; the addition did not"
    );
    assert_eq!(
        origin_of_delta_entry(&mut conn, added.id).unwrap(),
        Some(ContentOrigin::Authored)
    );

    let kept = additions_without_their_book(&mut conn, t.world).unwrap();
    let [(title, hag)] = kept.as_slice() else {
        panic!("one kept addition expected: {kept:?}");
    };
    assert_eq!(
        title, "Monster Manual",
        "it says which book it was written beside"
    );
    assert_eq!(hag.name, "Mire Hag");
    assert_eq!(hag.state, EntryState::Added);
    assert_eq!(hag.origin, ContentOrigin::Authored);
    assert!(hag.origin.may_be_shared());
    assert_eq!(
        hag.prose_text.as_deref(),
        Some("Lives in the fen and bargains in teeth.")
    );

    // A switched-off book is not partly on: its page is not served, and a
    // new addition beside it is refused until it is switched on again.
    assert!(matches!(
        world_reads(&mut conn, t.world, t.book.id, None, false),
        Err(DeltaError::Book(BookListError::NotOnTheList))
    ));
    let beside_nothing = add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Bog Wight",
        fields(&[]),
    );
    assert!(
        matches!(
            beside_nothing,
            Err(DeltaError::Book(BookListError::NotOnTheList))
        ),
        "{beside_nothing:?}"
    );

    // The world's own writing stays until somebody removes it, and removing
    // it does not need the book back.
    restore_entry(
        &mut conn, t.owner, t.world, t.book.id, "creature", "Mire Hag",
    )
    .unwrap();
    assert!(held_ids(&mut conn, t.world).is_empty());
    assert!(
        additions_without_their_book(&mut conn, t.world)
            .unwrap()
            .is_empty()
    );
}

/// Switching the book back on brings the kept addition back onto its page as
/// the same row — not a second copy, and not a conflict: the identity (world,
/// book, kind, name) was never released, so there is nothing to reconcile.
/// Adding the same kind and name again is refused as already added.
#[test]
fn switching_the_book_back_on_rejoins_the_addition_without_a_copy() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let (_, _, added) = one_of_each(&mut conn, &t);
    switch_off(&mut conn, t.owner, t.world, t.book.id).unwrap();

    switch_on(&mut conn, t.owner, t.world, t.book.id).unwrap();

    assert_eq!(
        held_ids(&mut conn, t.world),
        vec![added.id],
        "one row, the same row"
    );
    assert!(
        additions_without_their_book(&mut conn, t.world)
            .unwrap()
            .is_empty(),
        "back on its book's page, so no longer listed on its own"
    );
    let page = read(&mut conn, t.world, t.book.id);
    let hags: Vec<_> = page
        .entries
        .iter()
        .filter(|entry| entry.name == "Mire Hag")
        .collect();
    assert_eq!(hags.len(), 1);
    assert_eq!(hags[0].id, added.id);
    assert_eq!(hags[0].state, EntryState::Added);
    assert!(page.unattached.is_empty());
    // The change and the hide did not come back: they went with the book.
    assert_eq!(named(&page, "Goblin").unwrap().state, EntryState::Inherited);
    assert!(named(&page, "Orc").is_some());

    let again = add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[]),
    );
    assert!(
        matches!(again, Err(DeltaError::AlreadyAdded { .. })),
        "{again:?}"
    );
}

/// The database, not this module, is what keeps a change or hide off a book
/// the world is not running while an addition outlives it. Tried around the
/// module with SQL: the link deleted directly takes the change and the hide
/// and leaves the addition, and a hide over an unlisted book cannot be
/// written.
#[test]
fn the_database_ties_changes_and_hides_to_the_list_and_not_additions() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    one_of_each(&mut conn, &t);

    // Deleting the link directly, as a repair script would.
    diesel::sql_query(format!(
        "DELETE FROM world_books WHERE world_id = '{}' AND compendium_id = '{}'",
        t.world, t.book.id
    ))
    .execute(&mut conn)
    .unwrap();
    let forms: Vec<DeltaForm> = world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(t.world))
        .select(world_entry_deltas::form)
        .load(&mut conn)
        .unwrap();
    assert_eq!(forms, vec![DeltaForm::Added]);

    let hide_unlisted = diesel::sql_query(format!(
        "INSERT INTO world_entry_deltas (id, world_id, compendium_id, kind, name, form, origin) \
         VALUES ('{}', '{}', '{}', 'creature', 'Goblin', 'Hidden', 'Uploaded')",
        Uuid::now_v7(),
        t.world,
        t.book.id,
    ))
    .execute(&mut conn);
    assert!(hide_unlisted.is_err(), "a hide over a book not on the list");
}

/// FR-028 still holds for additions whose book is off: a world that goes takes
/// them, because an addition is the world's and has nowhere else to live.
#[test]
fn a_kept_addition_goes_with_its_world() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();

    let t = a_table(&mut conn);
    one_of_each(&mut conn, &t);
    switch_off(&mut conn, t.owner, t.world, t.book.id).unwrap();
    diesel::delete(worlds::table.filter(worlds::id.eq(t.world)))
        .execute(&mut conn)
        .unwrap();
    assert!(held_ids(&mut conn, t.world).is_empty());
}

/// **Deltas under the origin invariant** (spec 049 FR-054a, ADR-097). No route
/// offers a delta as a collection member today — `world_entry_delta` is not a
/// member type spec 026 knows — so this goes straight at the table: a changed
/// uploaded entry is refused at the door, and a world-only addition is let
/// through. The invariant holds before any route learns to ask it.
#[test]
fn a_collection_refuses_a_changed_entry_and_accepts_an_addition() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let (changed, hidden, added) = one_of_each(&mut conn, &t);

    let collection = Uuid::now_v7();
    diesel::sql_query(format!(
        "INSERT INTO world_collections (id, world_id, name, created_by, updated_by) \
         VALUES ('{collection}', '{}', 'Fen Folk', '{}', '{}')",
        t.world, t.owner, t.owner
    ))
    .execute(&mut conn)
    .unwrap();

    let as_member = |conn: &mut PgConnection, delta: Uuid| {
        diesel::sql_query(format!(
            "INSERT INTO world_collection_members (id, collection_id, member_type, member_id, added_by) \
             VALUES ('{}', '{collection}', 'world_entry_delta', '{delta}', '{}')",
            Uuid::now_v7(),
            t.owner
        ))
        .execute(conn)
    };

    for (form, id) in [("changed", changed.id), ("hidden", hidden.id)] {
        let refused = as_member(&mut conn, id)
            .expect_err("an uploaded entry, however edited, may not join a collection");
        assert!(
            refused.to_string().contains("FR-054a"),
            "{form}: refused by the invariant, not by accident: {refused}"
        );
    }
    as_member(&mut conn, added.id).expect("a world-only addition is authored and may");

    // And the function every sharing rule asks, answering per entry.
    #[derive(QueryableByName)]
    struct Answer {
        #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Text>)]
        origin: Option<String>,
    }
    let ask = |conn: &mut PgConnection, id: Uuid| {
        diesel::sql_query(format!(
            "SELECT content_origin('world_entry_delta', '{id}')::text AS origin"
        ))
        .get_result::<Answer>(conn)
        .unwrap()
        .origin
    };
    assert_eq!(ask(&mut conn, changed.id).as_deref(), Some("Uploaded"));
    assert_eq!(ask(&mut conn, added.id).as_deref(), Some("Authored"));
    assert_eq!(ask(&mut conn, Uuid::now_v7()), None);
}
