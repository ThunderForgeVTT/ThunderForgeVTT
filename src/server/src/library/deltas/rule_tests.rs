//! What a world reads, as a rule and nothing else (spec 050 T072, T073).
//!
//! [`resolve`] is a function of two lists, so every edge of it is pinned here
//! without a database — above all FR-025a, where two entries share a kind and
//! a name and a delta must attach to neither. The same rules are attacked
//! against a real database in `tests.rs`.

use super::*;

fn stored(kind: &str, name: &str, fields: serde_json::Value) -> StoredEntry {
    StoredEntry {
        id: Uuid::now_v7(),
        compendium_id: Uuid::nil(),
        kind: kind.to_string(),
        name: name.to_string(),
        name_uncertain: false,
        page: 12,
        field_values: fields,
        prose_text: None,
        suspect: false,
        extras: None,
        created_at: chrono::Utc::now().naive_utc(),
    }
}

fn delta(form: DeltaForm, kind: &str, name: &str, fields: Option<serde_json::Value>) -> Delta {
    let now = chrono::Utc::now().naive_utc();
    Delta {
        id: Uuid::now_v7(),
        world_id: Uuid::nil(),
        compendium_id: Uuid::nil(),
        kind: kind.to_string(),
        name: name.to_string(),
        form,
        origin: form.origin_over(ContentOrigin::Uploaded),
        field_values: fields,
        prose_text: None,
        changed_by: None,
        created_at: now,
        updated_at: now,
    }
}

/// FR-022 and FR-023: a change replaces the fields it names, leaves the rest
/// as the book has them, and the book's own reading stays visible as
/// "before" (FR-024).
#[test]
fn a_change_is_laid_over_the_book_and_the_book_shows_through() {
    let base = vec![stored(
        "creature",
        "Goblin",
        serde_json::json!({"armour": {"state": "clear", "value": "15"}, "hits": {"state": "clear", "value": "7"}}),
    )];
    let change = delta(
        DeltaForm::Changed,
        "creature",
        "Goblin",
        Some(serde_json::json!({"hits": {"state": "clear", "value": "12"}})),
    );

    let read = resolve(ContentOrigin::Uploaded, base, vec![change.clone()], false);

    assert!(read.unattached.is_empty());
    let [goblin] = read.entries.as_slice() else {
        panic!("one entry expected: {:?}", read.entries);
    };
    assert_eq!(goblin.state, EntryState::Changed);
    assert_eq!(goblin.delta_id, Some(change.id));
    assert_eq!(goblin.field_values["hits"]["value"], "12");
    assert_eq!(goblin.field_values["armour"]["value"], "15");
    assert_eq!(
        goblin.before.as_ref().unwrap().field_values["hits"]["value"],
        "7"
    );
}

/// **FR-025a, the case this phase exists to get right.** Two entries in one
/// book share a kind and a name. A delta written over that identity attaches
/// to **neither** — not the first, not the second, not both — both are
/// served exactly as the book has them, and the delta is reported with how
/// many entries made it ambiguous.
#[test]
fn a_delta_over_two_same_named_entries_attaches_to_neither_and_is_reported() {
    let first = stored(
        "creature",
        "Goblin",
        serde_json::json!({"hits": {"state": "clear", "value": "7"}}),
    );
    let second = stored(
        "creature",
        "Goblin",
        serde_json::json!({"hits": {"state": "clear", "value": "11"}}),
    );
    let bystander = stored("creature", "Orc", serde_json::json!({}));
    let change = delta(
        DeltaForm::Changed,
        "creature",
        "Goblin",
        Some(serde_json::json!({"hits": {"state": "clear", "value": "99"}})),
    );

    let read = resolve(
        ContentOrigin::Uploaded,
        vec![first.clone(), second.clone(), bystander],
        vec![change.clone()],
        false,
    );

    let goblins: Vec<_> = read
        .entries
        .iter()
        .filter(|entry| entry.name == "Goblin")
        .collect();
    assert_eq!(goblins.len(), 2, "both goblins are still served");
    for goblin in &goblins {
        assert_eq!(goblin.state, EntryState::Inherited, "{goblin:?}");
        assert!(goblin.ambiguous);
        assert!(goblin.delta_id.is_none());
        assert_ne!(goblin.field_values["hits"]["value"], "99");
    }
    let hits: Vec<_> = goblins
        .iter()
        .map(|goblin| goblin.field_values["hits"]["value"].as_str().unwrap())
        .collect();
    assert!(hits.contains(&"7") && hits.contains(&"11"), "{hits:?}");

    assert_eq!(read.unattached.len(), 1, "reported once, not once per twin");
    assert_eq!(read.unattached[0].0.id, change.id);
    assert_eq!(read.unattached[0].1, Unattached::Ambiguous { count: 2 });

    let orc = read
        .entries
        .iter()
        .find(|entry| entry.name == "Orc")
        .unwrap();
    assert!(!orc.ambiguous, "ambiguity is per identity, not per book");
}

/// A hidden entry is absent from what the world reads, and present — marked —
/// for the person who asks to see what is hidden so they can restore it.
#[test]
fn a_hidden_entry_is_left_out_unless_asked_for() {
    let base = vec![stored("creature", "Goblin", serde_json::json!({}))];
    let hide = delta(DeltaForm::Hidden, "creature", "Goblin", None);

    let read = resolve(
        ContentOrigin::Uploaded,
        base.clone(),
        vec![hide.clone()],
        false,
    );
    assert!(read.entries.is_empty());

    let shown = resolve(ContentOrigin::Uploaded, base, vec![hide], true);
    assert_eq!(shown.entries[0].state, EntryState::Hidden);
    assert_eq!(shown.entries[0].origin, ContentOrigin::Uploaded);
}

/// FR-052 and FR-052a in the rule itself: over an uploaded book, a changed and
/// a hidden entry are uploaded and an added one is authored. And over an
/// authored collection (050 FR-008), a change is authored — the rule is "carry
/// the book's origin", not "always uploaded".
#[test]
fn origin_follows_the_form_not_the_book() {
    assert_eq!(
        DeltaForm::Changed.origin_over(ContentOrigin::Uploaded),
        ContentOrigin::Uploaded
    );
    assert_eq!(
        DeltaForm::Hidden.origin_over(ContentOrigin::Uploaded),
        ContentOrigin::Uploaded
    );
    assert_eq!(
        DeltaForm::Added.origin_over(ContentOrigin::Uploaded),
        ContentOrigin::Authored
    );
    assert_eq!(
        DeltaForm::Changed.origin_over(ContentOrigin::Authored),
        ContentOrigin::Authored
    );
}

/// FR-027's shape, already: a change whose entry is gone, and an addition the
/// book now has an entry for, are reported by name and not dropped.
#[test]
fn deltas_that_name_nothing_or_collide_are_reported() {
    let base = vec![stored("creature", "Goblin", serde_json::json!({}))];
    let orphan = delta(
        DeltaForm::Changed,
        "creature",
        "Hobgoblin",
        Some(serde_json::json!({"hits": {"state": "unread"}})),
    );
    let collision = delta(
        DeltaForm::Added,
        "creature",
        "Goblin",
        Some(serde_json::json!({})),
    );
    let addition = delta(
        DeltaForm::Added,
        "creature",
        "Grick",
        Some(serde_json::json!({})),
    );

    let read = resolve(
        ContentOrigin::Uploaded,
        base,
        vec![orphan.clone(), collision.clone(), addition.clone()],
        false,
    );

    let names: Vec<_> = read
        .entries
        .iter()
        .map(|entry| (entry.name.as_str(), entry.state, entry.origin))
        .collect();
    assert_eq!(
        names,
        vec![
            ("Goblin", EntryState::Inherited, ContentOrigin::Uploaded),
            ("Grick", EntryState::Added, ContentOrigin::Authored),
        ]
    );
    let reasons: Vec<_> = read
        .unattached
        .iter()
        .map(|(delta, why)| (delta.name.as_str(), *why))
        .collect();
    assert_eq!(
        reasons,
        vec![
            ("Goblin", Unattached::ShadowsTheBook),
            ("Hobgoblin", Unattached::NoSuchEntry),
        ]
    );
}
