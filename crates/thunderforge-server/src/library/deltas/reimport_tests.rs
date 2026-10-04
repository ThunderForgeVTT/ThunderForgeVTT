//! A book re-read under worlds that have changed it (spec 049 T079 to T082,
//! spec 050 FR-006, FR-026, FR-027).
//!
//! Against a real database, because what these defend is what the rows say
//! afterwards: the base is a new version, the deltas are the same rows, and a
//! re-read that fails partway leaves every byte where it was.

use super::*;
use crate::compendium::store::{self, ImportError};
use crate::content::{Entry, NameState};
use crate::library::book_list::switch_on;
use crate::library::deltas::tests::{
    Table, a_creature, a_table, a_world_running, base_bytes, clear, fields, named, read,
};
use crate::library::deltas::{Content, DeltaForm, EntryState, add_entry, change_entry, hide_entry};
use crate::schema::compendiums;
use crate::test_support::test_app_state;

fn a_feat(name: &str, text: &str) -> Entry {
    Entry {
        kind: "feat".to_string(),
        name: name.to_string(),
        name_state: NameState::Clear,
        page: 165,
        values: Default::default(),
        text: Some(text.to_string()),
        suspect: false,
        extras: None,
    }
}

fn version_of(conn: &mut PgConnection, book: Uuid) -> i32 {
    compendiums::table
        .filter(compendiums::id.eq(book))
        .select(compendiums::base_version)
        .first(conn)
        .unwrap()
}

/// Every delta row over the book, whole, in a stable order.
fn delta_rows(conn: &mut PgConnection, book: Uuid) -> Vec<String> {
    world_entry_deltas::table
        .filter(world_entry_deltas::compendium_id.eq(book))
        .order(world_entry_deltas::id)
        .select(Delta::as_select())
        .load::<Delta>(conn)
        .unwrap()
        .into_iter()
        .map(|delta| format!("{delta:?}"))
        .collect()
}

fn reread(conn: &mut PgConnection, t: &Table, entries: &[Entry]) -> Result<(), ImportError> {
    store::replace_entries(conn, t.owner, t.book.id, "reader-test-2", entries)
}

/// **T079, FR-006 and FR-026.** Two worlds each change the book differently;
/// the owner re-reads it with the goblin's armour corrected and a hobgoblin
/// found. The base is version 2 and every entry is a new row, and every delta
/// is the same row still applying: the goblin keeps its changed hits over the
/// corrected armour, the orc stays hidden, the added hag and the rewritten
/// feat are still there. Nothing is reported, because nothing was stranded.
#[test]
fn deltas_in_two_worlds_survive_a_reimport() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let other = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, other, t.book.id).unwrap();

    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap();
    hide_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    add_entry(
        &mut conn,
        t.owner,
        other,
        t.book.id,
        "creature",
        "Mire Hag",
        Content::Prose("Lives in the fen.".to_string()),
    )
    .unwrap();
    change_entry(
        &mut conn,
        t.owner,
        other,
        t.book.id,
        "feat",
        "Alert",
        Content::Prose("You cannot be surprised.".to_string()),
    )
    .unwrap();

    assert_eq!(version_of(&mut conn, t.book.id), 1);
    let entry_ids_before: Vec<String> = base_bytes(&mut conn, t.book.id)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    let deltas_before = delta_rows(&mut conn, t.book.id);

    reread(
        &mut conn,
        &t,
        &[
            a_creature("Goblin", "16", 166),
            a_creature("Orc", "13", 246),
            a_creature("Hobgoblin", "18", 186),
            a_feat("Alert", "You gain a +5 bonus to initiative."),
        ],
    )
    .unwrap();

    assert_eq!(
        version_of(&mut conn, t.book.id),
        2,
        "a new version, not an edit"
    );
    let entry_ids_after: Vec<String> = base_bytes(&mut conn, t.book.id)
        .into_iter()
        .map(|(id, _, _)| id)
        .collect();
    assert!(
        entry_ids_after
            .iter()
            .all(|after| !entry_ids_before.contains(after)),
        "every entry of the new reading is a new row"
    );
    assert_eq!(
        delta_rows(&mut conn, t.book.id),
        deltas_before,
        "not one delta row was touched"
    );

    let first = read(&mut conn, t.world, t.book.id);
    let goblin = named(&first, "Goblin").unwrap();
    assert_eq!(goblin.state, EntryState::Changed);
    assert_eq!(goblin.field_values["hits"], serde_json::json!(clear("12")));
    assert_eq!(
        goblin.field_values["armour"],
        serde_json::json!(clear("16")),
        "the change lies over the corrected reading"
    );
    assert!(named(&first, "Orc").is_none(), "still hidden");
    assert!(
        named(&first, "Hobgoblin").is_some(),
        "the new entry is served"
    );
    assert!(first.unattached.is_empty());

    let second = read(&mut conn, other, t.book.id);
    assert_eq!(named(&second, "Mire Hag").unwrap().state, EntryState::Added);
    let alert = named(&second, "Alert").unwrap();
    assert_eq!(alert.state, EntryState::Changed);
    assert_eq!(
        alert.prose_text.as_deref(),
        Some("You cannot be surprised.")
    );
    assert!(second.unattached.is_empty());

    assert!(
        unattached_after_reimport(&mut conn, t.book.id)
            .unwrap()
            .is_empty()
    );
}

/// **T080, FR-027.** The re-read renames the goblin — which Phase 10's rule
/// makes a removal plus an addition — finds a second orc, and finds a real
/// hobgoblin where one world had added its own. Each stranded delta is named,
/// per world, with why; every one is still stored; and the worlds' own reads
/// say the same.
#[test]
fn a_delta_that_cannot_attach_is_named_and_kept() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let other = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, other, t.book.id).unwrap();

    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap();
    hide_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    change_entry(
        &mut conn,
        t.owner,
        other,
        t.book.id,
        "creature",
        "Orc",
        fields(&[("hits", clear("20"))]),
    )
    .unwrap();
    add_entry(
        &mut conn,
        t.owner,
        other,
        t.book.id,
        "creature",
        "Hobgoblin",
        fields(&[("armour", clear("17"))]),
    )
    .unwrap();
    let held_before = delta_rows(&mut conn, t.book.id);
    assert_eq!(held_before.len(), 4);

    reread(
        &mut conn,
        &t,
        &[
            a_creature("Goblin Warrior", "15", 166),
            a_creature("Orc", "13", 246),
            a_creature("Orc", "14", 247),
            a_creature("Hobgoblin", "18", 186),
            a_feat("Alert", "You gain a +5 bonus to initiative."),
        ],
    )
    .unwrap();

    let report = unattached_after_reimport(&mut conn, t.book.id).unwrap();
    let described: Vec<(Uuid, Vec<(String, DeltaForm, Unattached)>)> = report
        .iter()
        .map(|world| {
            (
                world.world_id,
                world
                    .deltas
                    .iter()
                    .map(|(delta, why)| (delta.name.clone(), delta.form, *why))
                    .collect(),
            )
        })
        .collect();
    let for_world = |world: Uuid| {
        described
            .iter()
            .find(|(id, _)| *id == world)
            .map(|(_, deltas)| deltas.clone())
            .unwrap_or_default()
    };

    assert_eq!(report.len(), 2, "both worlds are named: {described:?}");
    assert_eq!(
        for_world(t.world),
        vec![
            (
                "Goblin".to_string(),
                DeltaForm::Changed,
                Unattached::NoSuchEntry
            ),
            (
                "Orc".to_string(),
                DeltaForm::Hidden,
                Unattached::Ambiguous { count: 2 }
            ),
        ]
    );
    assert_eq!(
        for_world(other),
        vec![
            (
                "Hobgoblin".to_string(),
                DeltaForm::Added,
                Unattached::ShadowsTheBook
            ),
            (
                "Orc".to_string(),
                DeltaForm::Changed,
                Unattached::Ambiguous { count: 2 }
            ),
        ]
    );
    assert!(
        report.iter().all(|world| !world.world_name.is_empty()),
        "each world is named for a person"
    );

    assert_eq!(
        delta_rows(&mut conn, t.book.id),
        held_before,
        "reported, not discarded"
    );
    assert_eq!(read(&mut conn, t.world, t.book.id).unattached.len(), 2);
    assert_eq!(read(&mut conn, other, t.book.id).unattached.len(), 2);
}

/// **T081.** A re-read that fails partway — its second batch of rows breaks a
/// constraint after the first batch is already written — leaves the previous
/// base byte for byte, the previous version number, and every delta.
#[test]
fn a_reimport_that_fails_partway_changes_nothing() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap();

    let base_before = base_bytes(&mut conn, t.book.id);
    let deltas_before = delta_rows(&mut conn, t.book.id);
    let counts_before: serde_json::Value = compendiums::table
        .filter(compendiums::id.eq(t.book.id))
        .select(compendiums::entry_counts)
        .first(&mut conn)
        .unwrap();

    // Six hundred good creatures fill the first batch of five hundred and
    // start the second; the last entry is prose that also carries fields,
    // which the table refuses (`compendium_entries_prose_has_no_fields`).
    let mut broken: Vec<Entry> = (0..600)
        .map(|n| a_creature(&format!("Kobold {n:03}"), "12", 195))
        .collect();
    let mut both = a_feat("Tough", "Your hit point maximum increases.");
    both.values = [("hits".to_string(), clear("2"))].into_iter().collect();
    broken.push(both);

    let failed = reread(&mut conn, &t, &broken);
    assert!(
        matches!(failed, Err(ImportError::Database(_))),
        "the database refused it, inside the write: {failed:?}"
    );

    assert_eq!(base_bytes(&mut conn, t.book.id), base_before, "the base");
    assert_eq!(version_of(&mut conn, t.book.id), 1, "the version");
    assert_eq!(
        delta_rows(&mut conn, t.book.id),
        deltas_before,
        "the deltas"
    );
    let counts_after: serde_json::Value = compendiums::table
        .filter(compendiums::id.eq(t.book.id))
        .select(compendiums::entry_counts)
        .first(&mut conn)
        .unwrap();
    assert_eq!(counts_after, counts_before, "the shelf's counts");
    assert_eq!(
        named(&read(&mut conn, t.world, t.book.id), "Goblin")
            .unwrap()
            .state,
        EntryState::Changed,
        "and the world still reads its change"
    );
}
