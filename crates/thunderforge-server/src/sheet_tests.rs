use super::*;

fn declaration(json: serde_json::Value) -> SheetDeclaration {
    let manifest = serde_json::json!({ "sheet": [json] });
    declarations_from_manifest(&manifest)
        .into_iter()
        .next()
        .expect("one declaration")
}

#[test]
fn a_text_field_a_player_wrote_in_is_published() {
    let d = declaration(serde_json::json!({
        "id": "highConcept", "label": "High Concept", "kind": "text", "slot": "traitData"
    }));
    let values = values_from(
        &d,
        &serde_json::json!({ "highConcept": "Disgraced Knight" }),
    );

    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].value,
        DeclaredValueKind::Text("Disgraced Knight".to_string())
    );
}

/// A field nobody filled in is not a field whose value is nothing — the same
/// rule the attribute and resource readers already apply.
#[test]
fn an_unwritten_text_field_publishes_nothing() {
    let d = declaration(serde_json::json!({ "id": "notes", "kind": "text" }));
    assert!(values_from(&d, &serde_json::json!({})).is_empty());
    assert!(values_from(&d, &serde_json::json!({ "notes": "" })).is_empty());
}

/// A track is the exception, and deliberately: the boxes exist whether or not
/// any are filled, so an empty stress track is the truth rather than an
/// absence.
#[test]
fn a_track_with_nothing_ticked_is_still_a_track() {
    let d = declaration(serde_json::json!({
        "id": "stress", "label": "Stress", "kind": "track", "of": 8, "slot": "resourceData"
    }));
    let values = values_from(&d, &serde_json::json!({}));

    assert_eq!(values.len(), 1);
    assert_eq!(
        values[0].value,
        DeclaredValueKind::Track { filled: 0, of: 8 }
    );
}

#[test]
fn a_track_cannot_report_more_marks_filled_than_it_has() {
    let d = declaration(serde_json::json!({ "id": "stress", "kind": "track", "of": 4 }));
    let values = values_from(&d, &serde_json::json!({ "stress": 99 }));
    assert_eq!(
        values[0].value,
        DeclaredValueKind::Track { filled: 4, of: 4 },
        "stored data outside the declared range is clamped, not believed"
    );
}

#[test]
fn an_unharmed_character_is_on_no_rung_of_the_ladder() {
    let d = declaration(serde_json::json!({
        "id": "damage", "kind": "state",
        "options": ["impaired", "debilitated", "dead"]
    }));
    let values = values_from(&d, &serde_json::json!({}));

    match &values[0].value {
        DeclaredValueKind::State { current, options } => {
            assert!(current.is_none(), "no rung is a real answer");
            assert_eq!(options.len(), 3, "and the ladder still travels");
        }
        other => panic!("expected a state, got {other:?}"),
    }
}

/// FR-032. Fate's twenty-six and Cypher's seven are blanks the *player* names;
/// a format modelling only fixed lists turns them into twenty-six wrong
/// labels.
#[test]
fn a_player_named_slot_takes_its_label_from_the_player() {
    let d = declaration(serde_json::json!({
        "id": "skill", "label": "Skills", "kind": "slots", "count": 26,
        "slot": "proficiencyData", "source": "skills"
    }));
    let values = values_from(
        &d,
        &serde_json::json!({ "skills": [
            { "name": "Burglary", "value": 3 },
            { "name": "Notice", "value": 2 }
        ]}),
    );

    assert_eq!(values.len(), 2, "two named, not twenty-six blanks");
    assert_eq!(values[0].label, "Burglary");
    assert_eq!(values[0].id, "skill1");
    assert_eq!(values[0].value.as_integer(), Some(3));
    assert_eq!(values[1].label, "Notice");
}

#[test]
fn unnamed_slots_are_not_published_as_empty_rows() {
    let d = declaration(serde_json::json!({
        "id": "skill", "kind": "slots", "count": 26, "source": "skills"
    }));
    let values = values_from(
        &d,
        &serde_json::json!({ "skills": [{ "name": "Fight", "value": 2 }, { "name": "" }] }),
    );
    assert_eq!(
        values.len(),
        1,
        "an unfilled sheet is not twenty-six blanks"
    );
}

#[test]
fn a_slot_declaration_never_publishes_more_than_it_declares() {
    let d = declaration(serde_json::json!({
        "id": "skill", "kind": "slots", "count": 2, "source": "skills"
    }));
    let values = values_from(
        &d,
        &serde_json::json!({ "skills": [{"name":"A"},{"name":"B"},{"name":"C"}] }),
    );
    assert_eq!(values.len(), 2);
}

/// FR-033: the parts of one thing say so.
#[test]
fn a_grouped_declaration_carries_its_group_onto_every_value() {
    let manifest = serde_json::json!({ "sheet": [
        { "id": "might", "kind": "number", "group": "might", "slot": "abilityData" },
        { "id": "mightPool", "kind": "number", "group": "might", "slot": "resourceData" }
    ]});
    let declared = declarations_from_manifest(&manifest);
    assert_eq!(declared.len(), 2);
    assert!(declared.iter().all(|d| d.group.as_deref() == Some("might")));

    let values = values_from(&declared[0], &serde_json::json!({ "might": 10 }));
    assert_eq!(values[0].group.as_deref(), Some("might"));
}

/// A kind this build does not know is skipped rather than guessed at. FR-035's
/// "show it as text anyway" is about a value that arrived, not a declaration
/// nothing can read.
#[test]
fn a_kind_this_build_does_not_know_is_skipped_rather_than_invented() {
    let manifest = serde_json::json!({ "sheet": [
        { "id": "clock", "kind": "segmented-clock", "segments": 6 },
        { "id": "notes", "kind": "text" }
    ]});
    let declared = declarations_from_manifest(&manifest);
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0].id, "notes");
}

/// A ruleset whose sheet is scores and pools has nothing else to say.
#[test]
fn a_system_declaring_no_sheet_block_yields_none() {
    assert!(declarations_from_manifest(&serde_json::json!({})).is_empty());
}

#[test]
fn declaration_order_is_the_manifests_own() {
    let manifest = serde_json::json!({ "sheet": [
        { "id": "c", "kind": "text" },
        { "id": "a", "kind": "text" },
        { "id": "b", "kind": "text" }
    ]});
    let ids: Vec<String> = declarations_from_manifest(&manifest)
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert_eq!(
        ids,
        vec!["c", "a", "b"],
        "the book's order, not the alphabet"
    );
}

/// The declarations a manifest makes must actually be read, or SC-012 passes
/// on a system that publishes nothing.
///
/// This caught a real thing: Fate's resources were declared and a test
/// asserting "Fate publishes no pools" kept passing, which meant either the
/// test or the parse was wrong. It was the parse — worth pinning so the next
/// silent drop is loud.
#[test]
fn the_two_newly_declared_manifests_are_actually_parsed() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/systems");
    let dir = dir.to_string_lossy();

    for (system, expected_sheet, expected_resources) in
        [("fate_core", 10usize, 2usize), ("cypher_system", 17, 3)]
    {
        let sheet = declarations_for_system(&dir, system);
        assert_eq!(
            sheet.len(),
            expected_sheet,
            "{system} declares {expected_sheet} sheet entries; {} were read",
            sheet.len()
        );

        let resources = crate::status_display::declarations_for_system(&dir, system);
        assert_eq!(
            resources.len(),
            expected_resources,
            "{system} declares {expected_resources} resources; {} were read — a \
             declaration nothing reads is a sheet with a hole in it",
            resources.len()
        );
    }
}

// ---------------------------------------------------------------------------
// Groups (T019g)
// ---------------------------------------------------------------------------

/// A group used to be an identifier and nothing else, which left a renderer
/// guessing at two things the system knows: what the group is called, and
/// which member to show when there is room for one. It answered both by
/// taking the first member — right by luck for Cypher, wrong the moment a
/// manifest is reordered.
mod groups {
    use super::*;

    fn manifest() -> serde_json::Value {
        serde_json::json!({
            "groups": [
                { "id": "might", "label": "Might", "headline": "mightPool" },
                { "id": "loose" }
            ]
        })
    }

    fn value(id: &str, group: Option<&str>) -> DeclaredValue {
        DeclaredValue {
            id: id.to_string(),
            label: id.to_string(),
            abbreviation: None,
            value: DeclaredValueKind::Integer(1),
            group: group.map(str::to_string),
            group_label: None,
            headline: false,
            origin: Origin::Stored,
        }
    }

    #[test]
    fn a_declared_group_names_every_one_of_its_members() {
        let groups = groups_from_manifest(&manifest());
        let mut values = vec![
            value("mightPool", Some("might")),
            value("mightEdge", Some("might")),
        ];
        apply_groups(&mut values, &groups);

        // Both members carry it, and the repetition cannot drift: the manifest
        // said it once and this stamped it twice.
        assert_eq!(values[0].group_label.as_deref(), Some("Might"));
        assert_eq!(values[1].group_label.as_deref(), Some("Might"));
    }

    #[test]
    fn exactly_the_named_member_is_the_headline() {
        let groups = groups_from_manifest(&manifest());
        let mut values = vec![
            // Declared second, and still the headline. The whole point: a
            // Cypher stat shows its pool, not its edge, whatever the order.
            value("mightEdge", Some("might")),
            value("mightPool", Some("might")),
        ];
        apply_groups(&mut values, &groups);

        assert!(!values[0].headline, "the edge is not the headline");
        assert!(values[1].headline, "the pool is");
    }

    #[test]
    fn a_group_the_manifest_did_not_name_is_left_for_the_renderer_to_infer() {
        // Not an error. A system may group values purely to keep them together
        // on the sheet, with nothing to add about the grouping itself — and
        // the first-member fallback is a reasonable answer for that.
        let groups = groups_from_manifest(&manifest());
        let mut values = vec![value("a", Some("loose")), value("b", Some("loose"))];
        apply_groups(&mut values, &groups);

        assert!(values.iter().all(|v| v.group_label.is_none()));
        assert!(values.iter().all(|v| !v.headline));
    }

    #[test]
    fn a_value_in_no_group_is_untouched() {
        let groups = groups_from_manifest(&manifest());
        let mut values = vec![value("solo", None)];
        apply_groups(&mut values, &groups);

        assert!(values[0].group_label.is_none());
        assert!(!values[0].headline);
    }

    #[test]
    fn a_manifest_with_no_groups_block_declares_none() {
        assert!(groups_from_manifest(&serde_json::json!({})).is_empty());
    }

    #[test]
    fn cypher_declares_a_group_for_each_of_its_three_stats() {
        // Read from the shipping manifest rather than a fixture, because the
        // failure this guards is the manifest and the code disagreeing. Cypher
        // is the system the group model was built for: each stat is a current
        // value, a pool and an edge, and before this the pool lived in the
        // `resources` block where nothing could put it in a group at all — so
        // every Cypher group had exactly one member.
        let groups = crate::sheet::groups_from_manifest(
            &serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string("../../packs/systems/cypher_system/system.json")
                    .expect("cypher manifest"),
            )
            .expect("cypher manifest parses"),
        );

        for stat in ["mightPool", "speedPool", "intellectPool"] {
            let group = groups
                .iter()
                .find(|g| g.id == stat)
                .unwrap_or_else(|| panic!("{stat} declares no group"));
            assert!(group.label.is_some(), "{stat} group has no name");
            assert_eq!(
                group.headline.as_deref(),
                Some(stat),
                "{stat} group should headline its pool"
            );
        }
    }
}
