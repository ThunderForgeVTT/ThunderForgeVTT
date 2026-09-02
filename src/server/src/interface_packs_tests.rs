use super::*;

fn packs_dir() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/interface")
        .to_string_lossy()
        .into_owned()
}

#[test]
fn the_base_pack_is_installed_and_valid() {
    load_validated(&packs_dir(), BASE_PACK_ID).expect("the fallback for every world must load");
}

#[test]
fn the_listing_has_no_special_position_for_the_base_pack() {
    let listed = list_installed(&packs_dir());
    assert!(
        listed.iter().any(|p| p.id == BASE_PACK_ID),
        "Forge appears by being in the directory, like anything else"
    );

    let titles: Vec<&str> = listed.iter().map(|p| p.title.as_str()).collect();
    let mut sorted = titles.clone();
    sorted.sort_unstable();
    assert_eq!(titles, sorted, "title order, with nothing pinned");
}

/// The only thing between a path parameter and the filesystem.
#[test]
fn a_pack_id_that_is_a_path_is_refused() {
    for hostile in ["../systems", "..", "a/b", "a\\b", ""] {
        assert!(
            read_manifest(&packs_dir(), hostile).is_err(),
            "{hostile:?} must not resolve"
        );
    }
}

#[test]
fn an_absent_pack_is_an_error_rather_than_an_empty_pack() {
    let findings = load_validated(&packs_dir(), "no-such-pack").unwrap_err();
    assert!(!findings.is_empty());
}

// ---------------------------------------------------------------------------
// T078: the acceptance test for Increment E
// ---------------------------------------------------------------------------

/// Everything a system publishes, as a pack is validated against it.
///
/// Attributes, resources, the derived half, and the sheet block — the same
/// four sources `declared_values_for_actor` resolves from. A pack validated
/// against a narrower set than the one that actually reaches a sheet would
/// reject layouts that work, and accept ones that do not.
fn published_ids(systems_dir: &str, system_id: &str) -> Vec<String> {
    let mut ids: Vec<String> =
        crate::attributes::attribute_declarations_for_system(systems_dir, system_id)
            .into_iter()
            .map(|d| d.id)
            .collect();

    ids.extend(
        crate::status_display::declarations_for_system(systems_dir, system_id)
            .into_iter()
            .map(|d| d.definition.id),
    );

    ids.extend(
        crate::sheet::declarations_for_system(systems_dir, system_id)
            .into_iter()
            .map(|d| d.id),
    );

    let manifest_path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    if let Ok(text) = std::fs::read_to_string(manifest_path)
        && let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(rules) = crate::systems::rules_for_system(system_id, &manifest)
    {
        ids.extend(rules.derived_declarations().into_iter().map(|d| d.id));
    }

    ids
}

/// **The acceptance test for Increment E**, and deliberately a piece of work
/// rather than a check: two packs were written for two systems whose sheets
/// disagree with 5e's and with each other. If either had needed a change to
/// the format, the format was not finished — which is how every previous gap
/// in it was found, and never by reading the type.
#[test]
fn the_fate_and_cypher_packs_validate_against_their_own_systems() {
    let packs = packs_dir();
    let systems = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .to_string_lossy()
        .into_owned();

    for (pack_id, system_id) in [
        ("forged-silver", "fate_core"),
        ("forged-bronze", "cypher_system"),
        ("forged-steel", "dnd5e"),
    ] {
        let manifest = load_validated(&packs, pack_id)
            .unwrap_or_else(|findings| panic!("{pack_id} does not validate: {findings:?}"));

        let declared = published_ids(&systems, system_id);
        pack_system_spec::interface::validate_targeting(&manifest, &|system| {
            (system == system_id).then(|| declared.clone())
        })
        .unwrap_or_else(|findings| {
            panic!("{pack_id} names something {system_id} does not publish: {findings:?}")
        });

        assert!(
            !manifest.referenced_ids().is_empty(),
            "{pack_id} names nothing, so this would pass without testing anything"
        );
    }
}

/// SC-013's other half: the three targeted packs are different from each
/// other, not one layout wearing three palettes.
#[test]
fn the_three_targeted_packs_are_structurally_different() {
    let packs = packs_dir();
    let shapes: Vec<(String, Vec<String>)> = ["forged-silver", "forged-bronze", "forged-steel"]
        .into_iter()
        .map(|id| {
            let m = load_validated(&packs, id).expect("valid");
            let mut ids: Vec<String> = m.referenced_ids().into_iter().map(str::to_string).collect();
            ids.sort();
            (id.to_string(), ids)
        })
        .collect();

    for (a, b) in [(0, 1), (0, 2), (1, 2)] {
        assert_ne!(
            shapes[a].1, shapes[b].1,
            "{} and {} lay out the same identifiers",
            shapes[a].0, shapes[b].0
        );
    }
}
