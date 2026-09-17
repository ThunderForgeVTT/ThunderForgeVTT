//! A world's changes synced back to their collection, against a real database
//! (spec 049 T096 to T099, spec 050 FR-100 to FR-105).

use super::*;
use crate::compendium::collections::{create_collection, write_entry};
use crate::compendium::store;
use crate::compendium::versions::{self, At};
use crate::library::book_list::switch_on;
use crate::library::deltas::tests::{
    SYSTEM, Table, a_table, a_world_running, clear, fields, named, read,
};
use crate::library::deltas::{Content, add_entry, change_entry, hide_entry};
use crate::test_support::{insert_test_user, insert_test_world_member, test_app_state};

/// An owner, a world running a collection with a hag, an eel and a feat.
fn a_collection_table(conn: &mut PgConnection) -> Table {
    let owner = insert_test_user(conn);
    let world = a_world_running(conn, owner);
    let collection = create_collection(conn, owner, "Fen Folk", SYSTEM).unwrap();
    for (name, value) in [("Mire Hag", "17"), ("Bog Eel", "12")] {
        write_entry(
            conn,
            owner,
            collection.id,
            "creature",
            name,
            fields(&[("armour", clear(value))]),
        )
        .unwrap();
    }
    write_entry(
        conn,
        owner,
        collection.id,
        "feat",
        "Bog-Born",
        Content::Prose("You do not sink.".to_string()),
    )
    .unwrap();
    switch_on(conn, owner, world, collection.id).unwrap();
    let book = store::load(conn, owner, collection.id).unwrap();
    Table { owner, world, book }
}

fn held(conn: &mut PgConnection, world: Uuid, book: Uuid) -> Vec<String> {
    deltas_over(conn, world, book, None)
        .unwrap()
        .into_iter()
        .map(|delta| delta.name)
        .collect()
}

/// **T096, T099, FR-100, FR-104, FR-105, US6 scenarios 1 to 6.** A change, a
/// hide and an addition go back to the collection as one new version. The
/// syncing world no longer holds them and reads them as the collection; a
/// world started afterwards inherits them; another world's delta over the
/// hidden entry is named as stranded, not dropped; and the version before the
/// sync can be put back.
#[test]
fn a_sync_back_lands_as_a_version_and_leaves_the_world() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_collection_table(&mut conn);
    let neighbour = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, neighbour, t.book.id).unwrap();

    change_entry(
        &mut conn,
        t.owner,
        neighbour,
        t.book.id,
        "creature",
        "Bog Eel",
        fields(&[("armour", clear("14"))]),
    )
    .unwrap();
    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("60"))]),
    )
    .unwrap();
    hide_entry(
        &mut conn, t.owner, t.world, t.book.id, "creature", "Bog Eel",
    )
    .unwrap();
    add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Will-o-Wisp",
        fields(&[("armour", clear("19"))]),
    )
    .unwrap();

    let plan = plan_sync_back(&mut conn, t.owner, t.world, t.book.id).unwrap();
    assert_eq!(plan.base_version, t.book.base_version);
    let mut described: Vec<(&str, &str)> = plan
        .changes
        .iter()
        .map(|change| {
            let word = match change {
                ShelfChange::Rewritten { .. } => "rewritten",
                ShelfChange::TakenOut { .. } => "taken out",
                ShelfChange::Added { .. } => "added",
            };
            (change.identity().1, word)
        })
        .collect();
    described.sort();
    assert_eq!(
        described,
        vec![
            ("Bog Eel", "taken out"),
            ("Mire Hag", "rewritten"),
            ("Will-o-Wisp", "added")
        ]
    );
    assert!(plan.staying.is_empty());

    let landed = sync_back(&mut conn, t.owner, t.world, t.book.id, &plan.stamp).unwrap();
    assert_eq!(landed.base_version, t.book.base_version + 1);

    // FR-105: the world holds none of it, and reads it as the collection.
    assert!(held(&mut conn, t.world, t.book.id).is_empty());
    let here = read(&mut conn, t.world, t.book.id);
    let hag = named(&here, "Mire Hag").unwrap();
    assert_eq!(hag.state, EntryState::Inherited);
    assert_eq!(hag.field_values["hits"]["value"], serde_json::json!("60"));
    assert_eq!(hag.field_values["armour"]["value"], serde_json::json!("17"));
    assert_eq!(
        named(&here, "Will-o-Wisp").unwrap().state,
        EntryState::Inherited
    );
    assert!(named(&here, "Bog Eel").is_none());

    // A world started afterwards inherits the synced version.
    let later = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, later, t.book.id).unwrap();
    let there = read(&mut conn, later, t.book.id);
    assert_eq!(
        named(&there, "Mire Hag").unwrap().field_values["hits"]["value"],
        serde_json::json!("60")
    );
    assert!(named(&there, "Will-o-Wisp").is_some());

    // The neighbour's change over the eel is still stored, and named.
    assert_eq!(held(&mut conn, neighbour, t.book.id), vec!["Bog Eel"]);
    assert_eq!(landed.stranded.len(), 1);
    assert_eq!(landed.stranded[0].world_id, neighbour);
    assert_eq!(landed.stranded[0].deltas[0].1, Unattached::NoSuchEntry);

    // FR-104: the version before the sync is kept and can be put back.
    let history = versions::history(&mut conn, t.owner, t.book.id).unwrap();
    assert!(history[0].replaced_by.starts_with("Synced from "));
    assert_eq!(history[0].version, t.book.base_version);
    versions::restore(&mut conn, t.owner, t.book.id, t.book.base_version).unwrap();
    let back = versions::read_at(&mut conn, t.owner, t.book.id, At::Current).unwrap();
    let names: Vec<&str> = back.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["Bog Eel", "Mire Hag", "Bog-Born"]);
    assert!(
        read(&mut conn, neighbour, t.book.id).unattached.is_empty(),
        "the eel is back"
    );
}

/// **FR-103.** What lands is what was shown: a stamp from before a further
/// change is refused and changes nothing, and nothing to sync is said so.
#[test]
fn a_sync_back_refuses_a_plan_that_moved_on() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_collection_table(&mut conn);

    let nothing = plan_sync_back(&mut conn, t.owner, t.world, t.book.id).unwrap();
    assert!(nothing.changes.is_empty());
    assert!(matches!(
        sync_back(&mut conn, t.owner, t.world, t.book.id, &nothing.stamp),
        Err(DeltaError::NothingToSync)
    ));

    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("60"))]),
    )
    .unwrap();
    let shown = plan_sync_back(&mut conn, t.owner, t.world, t.book.id).unwrap();
    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("61"))]),
    )
    .unwrap();
    assert!(matches!(
        sync_back(&mut conn, t.owner, t.world, t.book.id, &shown.stamp),
        Err(DeltaError::SyncPlanChanged)
    ));

    // The shelf moving on stales it too.
    let fresh = plan_sync_back(&mut conn, t.owner, t.world, t.book.id).unwrap();
    write_entry(
        &mut conn,
        t.owner,
        t.book.id,
        "creature",
        "Newt",
        fields(&[("armour", clear("10"))]),
    )
    .unwrap();
    assert!(matches!(
        sync_back(&mut conn, t.owner, t.world, t.book.id, &fresh.stamp),
        Err(DeltaError::SyncPlanChanged)
    ));
    assert_eq!(held(&mut conn, t.world, t.book.id), vec!["Mire Hag"]);
    assert!(
        versions::history(&mut conn, t.owner, t.book.id)
            .unwrap()
            .iter()
            .all(|past| !past.replaced_by.starts_with("Synced"))
    );
}

/// **T097, FR-101, FR-102.** No sync back into an imported book, for anyone,
/// at any volume, with the reason given — and the world's changes stay.
#[test]
fn an_imported_book_never_syncs_back() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let game_master = insert_test_user(&mut conn);
    insert_test_world_member(&mut conn, t.world, game_master, "GM");

    for name in ["Goblin", "Orc"] {
        change_entry(
            &mut conn,
            t.owner,
            t.world,
            t.book.id,
            "creature",
            name,
            fields(&[("hits", clear("99"))]),
        )
        .unwrap();
    }

    for caller in [t.owner, game_master] {
        let refused = plan_sync_back(&mut conn, caller, t.world, t.book.id).unwrap_err();
        assert!(
            matches!(&refused, DeltaError::BookNeverSyncsBack { title } if title == "Monster Manual"),
            "{refused}"
        );
        assert!(refused.to_string().contains("stay in this world"));
        assert!(matches!(
            sync_back(&mut conn, caller, t.world, t.book.id, "any stamp"),
            Err(DeltaError::BookNeverSyncsBack { .. })
        ));
    }
    assert_eq!(held(&mut conn, t.world, t.book.id), vec!["Goblin", "Orc"]);
    assert!(
        versions::history(&mut conn, t.owner, t.book.id)
            .unwrap()
            .is_empty()
    );
}

/// Only the world's owner syncs to the shelf; the table's other managers are
/// told who can, and a player is refused as for any change.
#[test]
fn only_the_world_owner_syncs_back() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_collection_table(&mut conn);
    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("60"))]),
    )
    .unwrap();

    let member = |conn: &mut PgConnection, role: &str| {
        let user = insert_test_user(conn);
        insert_test_world_member(conn, t.world, user, role);
        user
    };
    let game_master = member(&mut conn, "GM");
    let trusted = member(&mut conn, "TrustedPlayer");
    let player = member(&mut conn, "Player");
    let stamp = plan_sync_back(&mut conn, t.owner, t.world, t.book.id)
        .unwrap()
        .stamp;

    for caller in [game_master, trusted] {
        assert!(matches!(
            sync_back(&mut conn, caller, t.world, t.book.id, &stamp),
            Err(DeltaError::OnlyTheOwnerSyncs)
        ));
    }
    assert!(matches!(
        plan_sync_back(&mut conn, player, t.world, t.book.id),
        Err(DeltaError::Book(BookListError::MayNotManageBooks))
    ));
    assert_eq!(held(&mut conn, t.world, t.book.id), vec!["Mire Hag"]);
}

/// FR-027 still holds inside the world: an addition the collection has since
/// gained an entry for does not attach, is not synced, and stays.
#[test]
fn what_does_not_attach_stays_in_the_world() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_collection_table(&mut conn);
    add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Newt",
        fields(&[("armour", clear("9"))]),
    )
    .unwrap();
    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("60"))]),
    )
    .unwrap();
    write_entry(
        &mut conn,
        t.owner,
        t.book.id,
        "creature",
        "Newt",
        fields(&[("armour", clear("10"))]),
    )
    .unwrap();

    let plan = plan_sync_back(&mut conn, t.owner, t.world, t.book.id).unwrap();
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.staying.len(), 1);
    assert_eq!(plan.staying[0].1, Unattached::ShadowsTheBook);

    let landed = sync_back(&mut conn, t.owner, t.world, t.book.id, &plan.stamp).unwrap();
    assert_eq!(held(&mut conn, t.world, t.book.id), vec!["Newt"]);
    assert_eq!(landed.stranded.len(), 1, "still reported as the world's");
}
