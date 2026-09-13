//! A world's changes over its books, first as a pure rule and then against a
//! real database (spec 050 T071 to T075).
//!
//! The pure tests pin what a world reads. The database tests attack the same
//! rules from outside this module — raw SQL against the table, a second world,
//! the base's own rows — because a rule that holds only while this module is
//! the caller is a rule until somebody writes a different caller.

use super::*;
use crate::compendium::store::{self, NewBook};
use crate::content::{Entry, NameState};
use crate::library::book_list::{switch_off, switch_off_report, switch_on};
use crate::schema::worlds;
use crate::test_support::{
    insert_test_user, insert_test_world, insert_test_world_member, test_app_state,
};

const SYSTEM: &str = "test-system";

fn a_book(title: &str) -> NewBook {
    NewBook {
        book_title: title.to_string(),
        source_hash: format!("{:0>64}", Uuid::now_v7().simple()),
        system_id: SYSTEM.to_string(),
        parser_version: "reader-test".to_string(),
        page_count: 320,
        silent_page_count: 0,
    }
}

fn clear(value: &str) -> ReadValue {
    ReadValue::Clear(value.to_string())
}

fn a_creature(name: &str, armour: &str, page: u32) -> Entry {
    Entry {
        kind: "creature".to_string(),
        name: name.to_string(),
        name_state: NameState::Clear,
        page,
        values: [
            ("armour".to_string(), clear(armour)),
            ("hits".to_string(), clear("7")),
            ("speed".to_string(), ReadValue::Unread),
        ]
        .into_iter()
        .collect(),
        text: None,
        suspect: false,
        extras: None,
    }
}

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

fn fields(pairs: &[(&str, ReadValue)]) -> Content {
    Content::Fields(
        pairs
            .iter()
            .map(|(field, value)| (field.to_string(), value.clone()))
            .collect(),
    )
}

struct Table {
    owner: Uuid,
    world: Uuid,
    book: store::Compendium,
}

fn a_world_running(conn: &mut PgConnection, owner: Uuid) -> Uuid {
    let world_id = insert_test_world(conn, owner);
    diesel::update(worlds::table.filter(worlds::id.eq(world_id)))
        .set(worlds::game_system_id.eq(Some(SYSTEM.to_string())))
        .execute(conn)
        .expect("failed to set the world's system");
    world_id
}

/// An owner, a world running a book with a goblin, an orc and a feat.
fn a_table(conn: &mut PgConnection) -> Table {
    let owner = insert_test_user(conn);
    let world = a_world_running(conn, owner);
    let book = store::import_book(
        conn,
        owner,
        a_book("Monster Manual"),
        &[
            a_creature("Goblin", "15", 166),
            a_creature("Orc", "13", 246),
            a_feat("Alert", "You gain a +5 bonus to initiative."),
        ],
    )
    .unwrap();
    switch_on(conn, owner, world, book.id).unwrap();
    Table { owner, world, book }
}

/// Every stored byte of a book's entries, in a stable order: what "the base
/// is untouched" means, checked rather than assumed.
fn base_bytes(conn: &mut PgConnection, compendium_id: Uuid) -> Vec<(String, String, String)> {
    compendium_entries::table
        .filter(compendium_entries::compendium_id.eq(compendium_id))
        .order((compendium_entries::kind, compendium_entries::name))
        .select((
            compendium_entries::id,
            compendium_entries::name,
            compendium_entries::field_values,
            compendium_entries::prose_text,
        ))
        .load::<(Uuid, String, serde_json::Value, Option<String>)>(conn)
        .unwrap()
        .into_iter()
        .map(|(id, name, values, prose)| {
            (
                format!("{id}/{name}"),
                values.to_string(),
                prose.unwrap_or_default(),
            )
        })
        .collect()
}

fn read(conn: &mut PgConnection, world: Uuid, book: Uuid) -> Resolution {
    world_reads(conn, world, book, None, false).unwrap().1
}

fn named<'a>(resolution: &'a Resolution, name: &str) -> Option<&'a WorldEntry> {
    resolution.entries.iter().find(|entry| entry.name == name)
}

/// FR-023: the row holds the field that differs and not the entry. A field
/// set back to what the book says leaves the delta, and a change that leaves
/// nothing different leaves no delta at all.
#[test]
fn a_change_stores_only_what_differs() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);

    let held = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12")), ("armour", clear("15"))]),
    )
    .unwrap()
    .expect("hits differ from the book");

    assert_eq!(held.form, DeltaForm::Changed);
    assert_eq!(
        held.field_values,
        Some(serde_json::json!({"hits": {"state": "clear", "value": "12"}})),
        "armour matched the book and must not be stored"
    );
    assert!(held.prose_text.is_none());

    let back = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("7"))]),
    )
    .unwrap();
    assert!(back.is_none(), "nothing differs, so nothing is held");
    assert!(
        deltas_over(&mut conn, t.world, t.book.id, None)
            .unwrap()
            .is_empty()
    );
}

/// FR-020 and 050 SC-005: an edit, a hide and an addition in one world reach
/// neither the other world running the same book nor the book itself, whose
/// every stored byte is compared before and after.
#[test]
fn a_change_in_one_world_reaches_no_other_world_and_no_base() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let other = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, other, t.book.id).unwrap();
    let before = base_bytes(&mut conn, t.book.id);

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
    change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "feat",
        "Alert",
        Content::Prose("You gain a +10 bonus to initiative.".to_string()),
    )
    .unwrap();
    hide_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("40"))]),
    )
    .unwrap();

    let here = read(&mut conn, t.world, t.book.id);
    assert_eq!(
        named(&here, "Goblin").unwrap().field_values["hits"]["value"],
        "12"
    );
    assert_eq!(
        named(&here, "Alert").unwrap().prose_text.as_deref(),
        Some("You gain a +10 bonus to initiative.")
    );
    assert!(named(&here, "Orc").is_none());
    assert_eq!(named(&here, "Mire Hag").unwrap().state, EntryState::Added);

    let there = read(&mut conn, other, t.book.id);
    assert_eq!(
        named(&there, "Goblin").unwrap().field_values["hits"]["value"],
        "7"
    );
    assert_eq!(
        named(&there, "Goblin").unwrap().state,
        EntryState::Inherited
    );
    assert!(named(&there, "Orc").is_some(), "hidden here, present there");
    assert!(
        named(&there, "Mire Hag").is_none(),
        "added here, absent there"
    );

    assert_eq!(base_bytes(&mut conn, t.book.id), before);
}

/// **FR-025a at write time, with two same-named entries in a real book.** A
/// change and a hide over the shared identity are refused, name the count,
/// and write nothing; an addition may not take the identity either; and a
/// read serves both twins untouched.
#[test]
fn two_same_named_entries_refuse_every_delta_over_them() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let owner = insert_test_user(&mut conn);
    let world = a_world_running(&mut conn, owner);
    let book = store::import_book(
        &mut conn,
        owner,
        a_book("Limitless Monsters"),
        &[
            a_creature("Goblin", "15", 166),
            a_creature("Goblin", "17", 167),
        ],
    )
    .unwrap();
    switch_on(&mut conn, owner, world, book.id).unwrap();
    let before = base_bytes(&mut conn, book.id);

    let changed = change_entry(
        &mut conn,
        owner,
        world,
        book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("99"))]),
    );
    assert!(
        matches!(changed, Err(DeltaError::Ambiguous { count: 2, .. })),
        "{changed:?}"
    );
    let refusal = changed.unwrap_err().to_string();
    assert!(
        refusal.contains("2 creature entries named \"Goblin\""),
        "{refusal}"
    );
    assert!(refusal.contains("nothing was changed"), "{refusal}");

    let hidden = hide_entry(&mut conn, owner, world, book.id, "creature", "Goblin");
    assert!(
        matches!(hidden, Err(DeltaError::Ambiguous { count: 2, .. })),
        "{hidden:?}"
    );

    let added = add_entry(
        &mut conn,
        owner,
        world,
        book.id,
        "creature",
        "Goblin",
        fields(&[]),
    );
    assert!(
        matches!(added, Err(DeltaError::AlreadyInTheBook { .. })),
        "{added:?}"
    );

    assert!(
        deltas_over(&mut conn, world, book.id, None)
            .unwrap()
            .is_empty()
    );
    let read = read(&mut conn, world, book.id);
    assert_eq!(read.entries.len(), 2);
    assert!(read.entries.iter().all(|entry| entry.ambiguous));
    assert!(
        read.entries
            .iter()
            .all(|entry| entry.state == EntryState::Inherited)
    );
    assert_eq!(base_bytes(&mut conn, book.id), before);
}

/// FR-025a at read time: a change written while the identity was unique, and
/// a re-read of the book that then finds a second goblin. The change attaches
/// to neither and is reported, rather than applied to whichever row the
/// query returned first.
#[test]
fn a_delta_whose_identity_gains_a_twin_stops_attaching_and_is_reported() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let held = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("99"))]),
    )
    .unwrap()
    .unwrap();

    store::replace_entries(
        &mut conn,
        t.owner,
        t.book.id,
        "reader-test-2",
        &[
            a_creature("Goblin", "15", 166),
            a_creature("Goblin", "17", 300),
        ],
    )
    .unwrap();

    let read = read(&mut conn, t.world, t.book.id);
    assert!(
        read.entries
            .iter()
            .all(|entry| entry.field_values["hits"]["value"] == "7"),
        "{:?}",
        read.entries
    );
    assert_eq!(read.unattached.len(), 1);
    assert_eq!(read.unattached[0].0.id, held.id);
    assert_eq!(read.unattached[0].1, Unattached::Ambiguous { count: 2 });
    assert_eq!(
        deltas_over(&mut conn, t.world, t.book.id, None)
            .unwrap()
            .len(),
        1,
        "reported, and still held (FR-027)"
    );
}

/// **T073, FR-052 and FR-052a.** On one screen of one uploaded book: a changed
/// entry and a hidden entry are uploaded and may not be shared; an added entry
/// is authored and may. Asked through what the world reads, and through
/// [`origin_of_delta_entry`], which is what the collection invariant calls.
#[test]
fn a_changed_entry_is_uploaded_and_an_added_one_is_authored() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    assert_eq!(t.book.origin, ContentOrigin::Uploaded);

    let changed = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap()
    .unwrap();
    let hidden = hide_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    let added = add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[("hits", clear("40"))]),
    )
    .unwrap();

    assert_eq!(changed.origin, ContentOrigin::Uploaded);
    assert_eq!(hidden.origin, ContentOrigin::Uploaded);
    assert_eq!(added.origin, ContentOrigin::Authored);

    for (delta, origin) in [
        (&changed, ContentOrigin::Uploaded),
        (&hidden, ContentOrigin::Uploaded),
        (&added, ContentOrigin::Authored),
    ] {
        assert_eq!(
            origin_of_delta_entry(&mut conn, delta.id).unwrap(),
            Some(origin),
            "{:?}",
            delta.form
        );
    }
    assert_eq!(
        origin_of_delta_entry(&mut conn, Uuid::now_v7()).unwrap(),
        None,
        "an id nobody wrote is not evidence of authorship"
    );

    let (_, shown) = world_reads(&mut conn, t.world, t.book.id, None, true).unwrap();
    let goblin = named(&shown, "Goblin").unwrap();
    let hag = named(&shown, "Mire Hag").unwrap();
    assert!(!goblin.origin.may_be_shared());
    assert!(!named(&shown, "Orc").unwrap().origin.may_be_shared());
    assert!(hag.origin.may_be_shared());
    assert_eq!(
        hag.id, added.id,
        "an addition is addressed by its delta's id"
    );
}

/// FR-052 made unrepresentable: going around this module with SQL, a change
/// cannot be written as authored, an addition cannot be written as uploaded,
/// and a change cannot be turned into an addition in place — which is the
/// edit that would make an uploaded sword shareable.
#[test]
fn the_database_refuses_an_origin_the_form_does_not_carry() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);

    let raw = |conn: &mut PgConnection, form: &str, origin: &str, name: &str, fields: &str| {
        diesel::sql_query(format!(
            "INSERT INTO world_entry_deltas (id, world_id, compendium_id, kind, name, form, origin, field_values) \
             VALUES ('{}', '{}', '{}', 'creature', '{name}', '{form}', '{origin}', {fields})",
            Uuid::now_v7(),
            t.world,
            t.book.id,
        ))
        .execute(conn)
    };

    let as_authored = raw(
        &mut conn,
        "Changed",
        "Authored",
        "Goblin",
        "'{\"hits\":{\"state\":\"unread\"}}'",
    );
    assert!(
        as_authored.is_err(),
        "a change to an uploaded entry is uploaded"
    );
    let as_uploaded = raw(&mut conn, "Added", "Uploaded", "Mire Hag", "'{}'");
    assert!(as_uploaded.is_err(), "an addition is authored");

    let changed = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    )
    .unwrap()
    .unwrap();
    let flipped = diesel::sql_query(format!(
        "UPDATE world_entry_deltas SET form = 'Added', origin = 'Authored' WHERE id = '{}'",
        changed.id
    ))
    .execute(&mut conn);
    let message = flipped.expect_err("the flip must be refused").to_string();
    assert!(message.contains("FR-052"), "{message}");
    let form_only = diesel::sql_query(format!(
        "UPDATE world_entry_deltas SET form = 'Added', prose_text = NULL WHERE id = '{}'",
        changed.id
    ))
    .execute(&mut conn);
    assert!(
        form_only.is_err(),
        "an addition over an uploaded book is authored, not uploaded"
    );

    let renamed = diesel::sql_query(format!(
        "UPDATE world_entry_deltas SET name = 'Orc' WHERE id = '{}'",
        changed.id
    ))
    .execute(&mut conn);
    assert!(renamed.is_err(), "a rename is a hide and an addition");
}

/// FR-023's shape made structural: a hidden entry holds no content, a change
/// holds something and not nothing, and a delta over a book this world has
/// not switched on cannot exist.
#[test]
fn the_database_refuses_deltas_of_the_wrong_shape() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let unlisted = store::import_book(&mut conn, t.owner, a_book("Volo's"), &[]).unwrap();

    let attempt = |conn: &mut PgConnection,
                   compendium: Uuid,
                   form: &str,
                   fields: &str,
                   prose: &str| {
        diesel::sql_query(format!(
            "INSERT INTO world_entry_deltas (id, world_id, compendium_id, kind, name, form, origin, field_values, prose_text) \
             VALUES ('{}', '{}', '{compendium}', 'creature', 'Goblin', '{form}', 'Uploaded', {fields}, {prose})",
            Uuid::now_v7(),
            t.world,
        ))
        .execute(conn)
    };

    for (why, compendium, form, fields, prose) in [
        (
            "hidden with content",
            t.book.id,
            "Hidden",
            "'{\"a\":1}'",
            "NULL",
        ),
        ("hidden with prose", t.book.id, "Hidden", "NULL", "'text'"),
        ("an empty change", t.book.id, "Changed", "'{}'", "NULL"),
        ("a change of nothing", t.book.id, "Changed", "NULL", "NULL"),
        (
            "a change of both",
            t.book.id,
            "Changed",
            "'{\"a\":1}'",
            "'text'",
        ),
        (
            "fields that are not an object",
            t.book.id,
            "Changed",
            "'[1]'",
            "NULL",
        ),
        (
            "a book not on the list",
            unlisted.id,
            "Hidden",
            "NULL",
            "NULL",
        ),
    ] {
        assert!(
            attempt(&mut conn, compendium, form, fields, prose).is_err(),
            "{why} must be refused"
        );
    }
}

/// FR-024: a person sees what the book says beside what the world says, puts
/// it back, and the world reads the book again. Restoring an addition takes
/// it out. Restoring what was never changed is refused rather than a silent
/// success.
#[test]
fn a_changed_entry_shows_what_it_was_and_can_be_put_back() {
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
    hide_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "feat",
        "Table Rule",
        Content::Prose("Crits explode.".to_string()),
    )
    .unwrap();

    let goblin = world_entry(&mut conn, t.world, t.book.id, "creature", "Goblin")
        .unwrap()
        .unwrap();
    assert_eq!(goblin.field_values["hits"]["value"], "12");
    assert_eq!(
        goblin.before.unwrap().field_values["hits"]["value"],
        "7",
        "what it was before is the book's reading"
    );

    for (kind, name) in [
        ("creature", "Goblin"),
        ("creature", "Orc"),
        ("feat", "Table Rule"),
    ] {
        restore_entry(&mut conn, t.owner, t.world, t.book.id, kind, name).unwrap();
    }

    let read = read(&mut conn, t.world, t.book.id);
    let goblin = named(&read, "Goblin").unwrap();
    assert_eq!(goblin.state, EntryState::Inherited);
    assert_eq!(goblin.field_values["hits"]["value"], "7");
    assert!(named(&read, "Orc").is_some());
    assert!(named(&read, "Table Rule").is_none());

    let again = restore_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Goblin");
    assert!(
        matches!(again, Err(DeltaError::NothingToRestore { .. })),
        "{again:?}"
    );
}

/// FR-028: a world that goes takes its deltas, and nothing of the base or of
/// another world's deltas over the same book.
#[test]
fn removing_a_world_removes_its_deltas_and_nothing_else() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    let other = a_world_running(&mut conn, t.owner);
    switch_on(&mut conn, t.owner, other, t.book.id).unwrap();

    for world in [t.world, other] {
        change_entry(
            &mut conn,
            t.owner,
            world,
            t.book.id,
            "creature",
            "Goblin",
            fields(&[("hits", clear("12"))]),
        )
        .unwrap();
        add_entry(
            &mut conn,
            t.owner,
            world,
            t.book.id,
            "creature",
            "Mire Hag",
            fields(&[]),
        )
        .unwrap();
    }
    let before = base_bytes(&mut conn, t.book.id);

    diesel::delete(worlds::table.filter(worlds::id.eq(t.world)))
        .execute(&mut conn)
        .unwrap();

    let gone: i64 = world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(t.world))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(gone, 0);
    assert_eq!(
        deltas_over(&mut conn, other, t.book.id, None)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(base_bytes(&mut conn, t.book.id), before);
    let there = read(&mut conn, other, t.book.id);
    assert_eq!(
        named(&there, "Goblin").unwrap().field_values["hits"]["value"],
        "12"
    );
}

/// FR-020a, ADR-099: the Owner, a Game Master and a Trusted Player may change
/// what a world inherited; a Player and a stranger may not, by any of the four
/// writes.
#[test]
fn who_may_change_what_a_world_inherited() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);

    let member = |conn: &mut PgConnection, role: &str| {
        let user = insert_test_user(conn);
        insert_test_world_member(conn, t.world, user, role);
        user
    };
    let game_master = member(&mut conn, "GM");
    let trusted = member(&mut conn, "TrustedPlayer");
    let player = member(&mut conn, "Player");
    let stranger = insert_test_user(&mut conn);

    for (who, caller, may) in [
        ("Owner", t.owner, true),
        ("Game Master", game_master, true),
        ("Trusted Player", trusted, true),
        ("Player", player, false),
        ("stranger", stranger, false),
    ] {
        fn refused<T>(result: &Result<T, DeltaError>) -> bool {
            matches!(
                result,
                Err(DeltaError::Book(BookListError::MayNotManageBooks))
            )
        }
        let name = format!("Added by {who}");

        let changed = change_entry(
            &mut conn,
            caller,
            t.world,
            t.book.id,
            "creature",
            "Goblin",
            fields(&[("hits", clear("12"))]),
        );
        let hidden = hide_entry(&mut conn, caller, t.world, t.book.id, "creature", "Orc");
        let added = add_entry(
            &mut conn,
            caller,
            t.world,
            t.book.id,
            "creature",
            &name,
            fields(&[]),
        );

        if may {
            assert!(changed.is_ok(), "{who}: {changed:?}");
            assert!(hidden.is_ok(), "{who}: {hidden:?}");
            assert!(added.is_ok(), "{who}: {added:?}");
            let restored = restore_entry(&mut conn, caller, t.world, t.book.id, "creature", "Orc");
            assert!(restored.is_ok(), "{who}: {restored:?}");
            restore_entry(&mut conn, caller, t.world, t.book.id, "creature", "Goblin").unwrap();
        } else {
            assert!(refused(&changed), "{who}: {changed:?}");
            assert!(refused(&hidden), "{who}: {hidden:?}");
            assert!(refused(&added), "{who}: {added:?}");
            let restored =
                restore_entry(&mut conn, caller, t.world, t.book.id, "creature", "Goblin");
            assert!(refused(&restored), "{who}");
        }
    }

    let held = deltas_over(&mut conn, t.world, t.book.id, None).unwrap();
    let names: Vec<_> = held.iter().map(|delta| delta.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "Added by Game Master",
            "Added by Owner",
            "Added by Trusted Player"
        ],
        "only what the three trusted roles added remains"
    );
    assert!(held.iter().all(|delta| delta.changed_by.is_some()));
}

/// 050 FR-013 and 049 FR-046, the extension point Phase 9 left: switching a
/// book off names every delta that goes with it before anything goes, and
/// then they go.
#[test]
fn switching_a_book_off_names_its_deltas_first_and_then_takes_them() {
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
    hide_entry(&mut conn, t.owner, t.world, t.book.id, "creature", "Orc").unwrap();
    add_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Mire Hag",
        fields(&[]),
    )
    .unwrap();

    let report = switch_off_report(&mut conn, t.owner, t.world, t.book.id).unwrap();
    assert_eq!(
        report.deltas,
        vec![
            "changed: creature \"Goblin\"".to_string(),
            "added: creature \"Mire Hag\"".to_string(),
            "hidden: creature \"Orc\"".to_string(),
        ]
    );
    assert_eq!(
        deltas_over(&mut conn, t.world, t.book.id, None)
            .unwrap()
            .len(),
        3,
        "asking takes nothing"
    );

    switch_off(&mut conn, t.owner, t.world, t.book.id).unwrap();
    let left: i64 = world_entry_deltas::table
        .filter(world_entry_deltas::world_id.eq(t.world))
        .count()
        .get_result(&mut conn)
        .unwrap();
    assert_eq!(left, 0);
}

/// FR-042: a world that has moved to another system is not reading the book,
/// so it may not change it either.
#[test]
fn a_book_the_world_no_longer_matches_cannot_be_changed() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);
    diesel::update(worlds::table.filter(worlds::id.eq(t.world)))
        .set(worlds::game_system_id.eq(Some("another-system".to_string())))
        .execute(&mut conn)
        .unwrap();

    let changed = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        fields(&[("hits", clear("12"))]),
    );
    assert!(
        matches!(
            changed,
            Err(DeltaError::Book(BookListError::SystemMismatch { .. }))
        ),
        "{changed:?}"
    );
}

/// 049 FR-001b carried into the delta: prose has no fields to change, and an
/// anchored entry has no prose.
#[test]
fn a_change_keeps_prose_and_fields_apart() {
    let state = test_app_state();
    let mut conn = state.db_pool.get().unwrap();
    let t = a_table(&mut conn);

    let on_prose = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "feat",
        "Alert",
        fields(&[("hits", clear("1"))]),
    );
    assert!(
        matches!(on_prose, Err(DeltaError::ProseHasNoFields { .. })),
        "{on_prose:?}"
    );
    let on_fields = change_entry(
        &mut conn,
        t.owner,
        t.world,
        t.book.id,
        "creature",
        "Goblin",
        Content::Prose("A goblin.".to_string()),
    );
    assert!(
        matches!(on_fields, Err(DeltaError::FieldsHaveNoProse { .. })),
        "{on_fields:?}"
    );
}
