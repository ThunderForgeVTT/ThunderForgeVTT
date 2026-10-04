//! A book leaving the shelf, and what a table keeps (spec 050 decision 5,
//! FR-060, FR-061 as amended on 2026-09-13).
//!
//! Removal is the stronger act than switching off: the book itself goes, for
//! every world, with no way to switch it back on. Changes and hides go with it
//! because they mean nothing without the entries they modify. Additions stay,
//! detached, labelled with the title of the book they were written beside —
//! and nothing, including a re-import of the same file, can attach them to a
//! book again.

use super::tests::{Table, a_book, a_creature, a_table, a_world_running, clear, fields, read};
use super::*;
use crate::compendium::store;
use crate::library::book_list::{switch_off, switch_on};
use crate::schema::worlds;
use crate::test_support::test_app_state;

fn one_of_each(conn: &mut PgConnection, t: &Table, world: Uuid) -> Delta {
    change_entry(
        conn,
        t.owner,
        world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap();
    hide_entry(conn, t.owner, world, t.book.id, "creature", "Orc").unwrap();
    add_entry(
        conn,
        t.owner,
        world,
        t.book.id,
        "creature",
        "Mire Hag",
        Content::Prose("Lives in the fen and bargains in teeth.".to_string()),
    )
    .unwrap()
}

fn held(conn: &mut PgConnection, world: Uuid) -> Vec<Delta> {
    world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(world))
        .order(world_entry_deltas::id)
        .select(Delta::as_select())
        .load(conn)
        .unwrap()
}

/// **The owner's decision.** Two worlds hold deltas over one book — one still
/// running it, one that switched it off and kept its addition. The removal
/// report names, per world, what goes and what stays, including the world the
/// book list alone would not name. After removal the changes and hides are
/// gone; each addition is still in its world, detached, still authored, still
/// labelled with the book's title, still listed as the table's own writing,
/// still admissible to a collection, and still removable.
#[test]
fn removing_a_book_takes_changes_and_hides_and_keeps_additions() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let running = t.world;
    let hag_running = one_of_each(&mut conn, &t, running);

    let switched_off = a_world_running(&mut conn, t.owner);
    diesel::update(worlds::table.filter(worlds::id.eq(switched_off)))
        .set(worlds::name.eq("Zz The Quiet Table"))
        .execute(&mut conn)
        .unwrap();
    switch_on(&mut conn, t.owner, switched_off, t.book.id).unwrap();
    let hag_quiet = one_of_each(&mut conn, &t, switched_off);
    switch_off(&mut conn, t.owner, switched_off, t.book.id).unwrap();

    let report = removal_consequences(&mut conn, t.book.id).unwrap();
    let quiet = report
        .iter()
        .find(|each| each.world_id == switched_off)
        .expect("a world with the book off but an addition kept is still named");
    assert!(quiet.lost.is_empty(), "its changes already went: {quiet:?}");
    assert_eq!(quiet.kept, vec!["added: creature \"Mire Hag\"".to_string()]);
    let busy = report.iter().find(|each| each.world_id == running).unwrap();
    assert_eq!(
        busy.lost,
        vec![
            "changed: creature \"Goblin\"".to_string(),
            "hidden: creature \"Orc\"".to_string(),
        ]
    );
    assert_eq!(busy.kept, vec!["added: creature \"Mire Hag\"".to_string()]);
    assert_eq!(
        held(&mut conn, running).len(),
        3,
        "the report takes nothing"
    );

    store::remove(&mut conn, t.owner, t.book.id).unwrap();

    for (world, hag) in [(running, &hag_running), (switched_off, &hag_quiet)] {
        let left = held(&mut conn, world);
        let [kept] = left.as_slice() else {
            panic!("only the addition stays: {left:?}");
        };
        assert_eq!(kept.id, hag.id, "the same row");
        assert_eq!(kept.form, DeltaForm::Added);
        assert_eq!(kept.compendium_id, None);
        assert_eq!(kept.written_beside_title.as_deref(), Some("Monster Manual"));
        assert_eq!(kept.origin, ContentOrigin::Authored);
        assert_eq!(
            origin_of_delta_entry(&mut conn, kept.id).unwrap(),
            Some(ContentOrigin::Authored)
        );

        let listed = additions_without_their_book(&mut conn, world).unwrap();
        let [(title, entry)] = listed.as_slice() else {
            panic!("listed as the table's own: {listed:?}");
        };
        assert_eq!(title, "Monster Manual");
        assert_eq!(entry.id, hag.id);
        assert_eq!(entry.compendium_id, None);
        assert_eq!(entry.state, EntryState::Added);
        assert!(entry.origin.may_be_shared());
        assert_eq!(
            entry.prose_text.as_deref(),
            Some("Lives in the fen and bargains in teeth.")
        );
    }

    // Shareable, by the invariant every sharing route writes through.
    let collection = Uuid::now_v7();
    diesel::sql_query(format!(
        "INSERT INTO world_collections (id, world_id, name, created_by, updated_by) \
         VALUES ('{collection}', '{running}', 'Fen Folk', '{0}', '{0}')",
        t.owner
    ))
    .execute(&mut conn)
    .unwrap();
    diesel::sql_query(format!(
        "INSERT INTO world_collection_members (id, collection_id, member_type, member_id, added_by) \
         VALUES ('{}', '{collection}', 'world_entry_delta', '{}', '{}')",
        Uuid::now_v7(),
        hag_running.id,
        t.owner
    ))
    .execute(&mut conn)
    .expect("a kept addition is authored and may be shared");

    // And removable by the one address it still has.
    remove_kept_addition(&mut conn, t.owner, switched_off, hag_quiet.id).unwrap();
    assert!(held(&mut conn, switched_off).is_empty());
}

/// The database, not the library's removal, is what keeps the addition: a
/// raw DELETE of the compendium row detaches it just the same. And the
/// detachment runs one way only — no UPDATE can give a kept addition a book
/// again, or change the title it remembers.
#[test]
fn a_raw_delete_keeps_the_addition_and_nothing_brings_the_book_back() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let hag = one_of_each(&mut conn, &t, t.world);

    diesel::sql_query(format!(
        "DELETE FROM compendiums WHERE id = '{}'",
        t.book.id
    ))
    .execute(&mut conn)
    .expect("a raw removal must not be refused by a table's writing");

    let left = held(&mut conn, t.world);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].id, hag.id);
    assert_eq!(
        left[0].written_beside_title.as_deref(),
        Some("Monster Manual")
    );

    let another = store::import_book(&mut conn, t.owner, a_book("Monster Manual"), &[]).unwrap();
    for (why, set) in [
        (
            "rejoin a book",
            format!(
                "compendium_id = '{}', written_beside_title = NULL",
                another.id
            ),
        ),
        (
            "rejoin a book and keep a title",
            format!("compendium_id = '{}'", another.id),
        ),
        (
            "rename the book it remembers",
            "written_beside_title = 'Volo''s Guide'".to_string(),
        ),
        ("become a change", "form = 'Changed'".to_string()),
    ] {
        let tried = diesel::sql_query(format!(
            "UPDATE world_entry_deltas SET {set} WHERE id = '{}'",
            hag.id
        ))
        .execute(&mut conn);
        assert!(tried.is_err(), "a kept addition must not {why}");
    }

    // A change or hide cannot be written without a book, by any route.
    let bookless_hide = diesel::sql_query(format!(
        "INSERT INTO world_entry_deltas (id, world_id, compendium_id, kind, name, form, origin, written_beside_title) \
         VALUES ('{}', '{}', NULL, 'creature', 'Orc', 'Hidden', 'Uploaded', 'Monster Manual')",
        Uuid::now_v7(),
        t.world
    ))
    .execute(&mut conn);
    assert!(bookless_hide.is_err());
}

/// **Identity after removal.** The same book is read in again — a new
/// compendium with a new id, this time with a Mire Hag of its own in it — and
/// switched on. The kept addition does not rejoin the new book, does not
/// shadow or get shadowed by the book's own Mire Hag, and is not duplicated:
/// the book's entry is on the book's page, the table's own is under its own
/// heading with the old book's title, and a second addition of the same kind
/// and name beside the new book is refused with a reason naming the first.
#[test]
fn a_re_import_neither_absorbs_nor_duplicates_a_kept_addition() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let hag = one_of_each(&mut conn, &t, t.world);
    store::remove(&mut conn, t.owner, t.book.id).unwrap();

    let again = store::import_book(
        &mut conn,
        t.owner,
        a_book("Monster Manual"),
        &[
            a_creature("Goblin", "15", 166),
            a_creature("Mire Hag", "12", 210),
        ],
    )
    .unwrap();
    switch_on(&mut conn, t.owner, t.world, again.id).unwrap();

    let page = read(&mut conn, t.world, again.id);
    let hags: Vec<_> = page
        .entries
        .iter()
        .filter(|entry| entry.name == "Mire Hag")
        .collect();
    assert_eq!(hags.len(), 1, "the book's own, once");
    assert_eq!(hags[0].state, EntryState::Inherited);
    assert_ne!(hags[0].id, hag.id);
    assert!(page.unattached.is_empty(), "nothing silently collides");

    let kept = additions_without_their_book(&mut conn, t.world).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(
        kept[0].1.id, hag.id,
        "the table's own, still under its own heading"
    );
    assert_eq!(
        kept[0].1.compendium_id, None,
        "and not absorbed into the new book"
    );

    // A new book without its own Mire Hag: an addition of that name beside it
    // would be a second entry the table cannot tell from the first.
    let third = store::import_book(&mut conn, t.owner, a_book("Fen Bestiary"), &[]).unwrap();
    switch_on(&mut conn, t.owner, t.world, third.id).unwrap();
    let twin = add_entry(
        &mut conn,
        t.owner,
        t.world,
        third.id,
        "creature",
        "Mire Hag",
        fields(&[]),
    );
    let Err(DeltaError::AlreadyWrittenHere { book_title, .. }) = &twin else {
        panic!("a second Mire Hag must be refused: {twin:?}");
    };
    assert_eq!(book_title, "Monster Manual");
    assert!(twin.unwrap_err().to_string().contains("Monster Manual"));
    assert_eq!(held(&mut conn, t.world).len(), 1);
}

/// Account deletion is unchanged: the owner's worlds go, and every delta in
/// them goes with the world — additions included, kept or not.
#[test]
fn deleting_the_owners_account_still_takes_their_worlds_additions() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    one_of_each(&mut conn, &t, t.world);
    let other = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, other, t.book.id).unwrap();
    add_entry(
        &mut conn,
        t.owner,
        other,
        t.book.id,
        "creature",
        "Bog Wight",
        fields(&[]),
    )
    .unwrap();
    switch_off(&mut conn, t.owner, other, t.book.id).unwrap();

    crate::users::delete_user_data_on(&mut conn, t.owner).unwrap();

    assert!(held(&mut conn, t.world).is_empty());
    assert!(held(&mut conn, other).is_empty());
    let books: i64 = crate::schema::compendiums::table
        .filter(crate::schema::compendiums::id.eq(t.book.id))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(books, 0);
}
