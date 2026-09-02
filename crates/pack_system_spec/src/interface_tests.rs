//! What validation guarantees, and what the base pack has to demonstrate.

use super::*;

fn forge_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/interface/forge")
}

/// The real base pack, not a fixture — so these tests fail if Forge changes
/// and nobody thought about the rules that read it.
fn forge() -> InterfaceManifest {
    let text = std::fs::read_to_string(forge_path().join("interface.json"))
        .expect("the base pack must exist; it is the fallback for every world");
    serde_json::from_str(&text).expect("forge must deserialise under deny_unknown_fields")
}

fn parse(json: serde_json::Value) -> Result<InterfaceManifest, serde_json::Error> {
    serde_json::from_value(json)
}

/// A minimal pack, as the contract's worked example has it.
fn minimal(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "type": "interface",
        "title": "Test Pack",
        "version": "1.0.0",
        "description": "A pack for tests.",
        "compatibility": { "minimum": "0.1.0", "verified": "0.1.0", "maximum": null },
        "legal": {
            "licenseName": "AGPL-3.0-or-later",
            "attributionText": "ThunderForgeVTT Contributors"
        },
        "light": { "background": "#ffffff", "foreground": "#000000" },
        "dark":  { "background": "#000000", "foreground": "#ffffff" },
        "targets": []
    })
}

fn findings_of(value: serde_json::Value, directory: &str) -> Findings {
    let manifest = parse(value).expect("fixture should deserialise");
    validate(&manifest, directory, &forge()).unwrap_err()
}

// ---------------------------------------------------------------------------
// Structural — one rejection per contract row
// ---------------------------------------------------------------------------

#[test]
fn the_smallest_valid_pack_validates() {
    let manifest = parse(minimal("tiny")).expect("deserialises");
    validate(&manifest, "tiny", &forge()).expect("a pack that changes two colours is valid");
}

#[test]
fn an_unknown_key_is_refused_rather_than_ignored() {
    let mut value = minimal("tiny");
    value["lightt"] = serde_json::json!({});
    assert!(
        parse(value).is_err(),
        "a misspelling must be found at validation, not by looking at a screen \
         that is subtly wrong"
    );
}

#[test]
fn an_unknown_token_key_is_refused_too() {
    let mut value = minimal("tiny");
    value["light"]["backgorund"] = serde_json::json!("#fff");
    assert!(parse(value).is_err());
}

#[test]
fn a_pack_claiming_to_be_a_system_pack_is_refused() {
    let mut value = minimal("tiny");
    value["type"] = serde_json::json!("system");
    let findings = findings_of(value, "tiny");
    assert!(
        findings.iter().any(|f| f.contains("interface")),
        "the type is exclusive because the safety rule attaches to it: {findings:?}"
    );
}

#[test]
fn an_id_that_disagrees_with_its_directory_is_refused() {
    let findings = findings_of(minimal("stated"), "actual");
    assert!(
        findings.iter().any(|f| f.contains("directory")),
        "a pack referable two ways has no identity: {findings:?}"
    );
}

#[test]
fn a_colour_nothing_can_parse_is_refused_and_named() {
    let mut value = minimal("tiny");
    value["light"]["background"] = serde_json::json!("not-a-colour");
    let findings = findings_of(value, "tiny");
    assert!(
        findings.iter().any(|f| f.contains("not-a-colour")),
        "an unparseable colour is a rejection, not a fallback: {findings:?}"
    );
}

#[test]
fn missing_legal_metadata_is_refused() {
    let mut value = minimal("tiny");
    value["legal"]["licenseName"] = serde_json::json!("");
    let findings = findings_of(value, "tiny");
    assert!(
        findings
            .iter()
            .any(|f| f.to_lowercase().contains("license")),
        "a pack is a redistributable artifact whoever wrote it: {findings:?}"
    );
}

// ---------------------------------------------------------------------------
// The legibility floor (FR-012a, SC-003a)
// ---------------------------------------------------------------------------

/// The failure that matters most is the one that reads fine in one mode.
///
/// A message saying only "contrast too low" sends an author to the wrong half
/// of their file.
#[test]
fn a_contrast_failure_in_one_mode_names_that_mode() {
    let mut value = minimal("tiny");
    value["light"]["foreground"] = serde_json::json!("#f4f4f4"); // on white
    let findings = findings_of(value, "tiny");

    let complaint = findings
        .iter()
        .find(|f| f.contains("foreground on background"))
        .unwrap_or_else(|| panic!("expected a legibility finding, got {findings:?}"));
    assert!(
        complaint.starts_with("light:"),
        "names the mode: {complaint}"
    );
    assert!(
        complaint.contains("4.5"),
        "names the requirement: {complaint}"
    );
    assert!(
        findings.iter().all(|f| !f.starts_with("dark:")),
        "and says nothing about the mode that was fine: {findings:?}"
    );
}

#[test]
fn a_pack_that_omits_a_token_is_measured_against_what_a_reader_sees() {
    // Declares a foreground and no background. Alone it is unmeasurable; over
    // Forge it is a light foreground on Forge's white, and unreadable.
    let mut value = minimal("tiny");
    value["light"] = serde_json::json!({ "foreground": "#fafafa" });
    let findings = findings_of(value, "tiny");
    assert!(
        findings
            .iter()
            .any(|f| f.contains("foreground on background")),
        "contrast is a property of what a reader sees, not of what a file \
         happens to contain: {findings:?}"
    );
}

// ---------------------------------------------------------------------------
// Targeting (FR-026, SC-003b) and the untargeted rule (FR-025b)
// ---------------------------------------------------------------------------

fn with_layout(
    id: &str,
    targets: serde_json::Value,
    layout: serde_json::Value,
) -> InterfaceManifest {
    let mut value = minimal(id);
    value["targets"] = targets;
    value["layout"] = layout;
    parse(value).expect("deserialises")
}

#[test]
fn naming_an_identifier_a_target_does_not_declare_is_refused_by_both_names() {
    let manifest = with_layout(
        "steel",
        serde_json::json!(["dnd5e"]),
        serde_json::json!([{ "kind": "value", "id": "wishPoints" }]),
    );
    let findings = validate_targeting(&manifest, &|system| match system {
        "dnd5e" => Some(vec!["strength".to_string(), "hitPoints".to_string()]),
        _ => None,
    })
    .unwrap_err();

    let complaint = &findings[0];
    assert!(
        complaint.contains("wishPoints"),
        "names the identifier: {complaint}"
    );
    assert!(complaint.contains("dnd5e"), "and the system: {complaint}");
}

/// Per target, never against their union.
#[test]
fn two_targets_are_checked_independently() {
    let manifest = with_layout(
        "steel",
        serde_json::json!(["dnd5e", "blades_in_the_dark"]),
        serde_json::json!([{ "kind": "value", "id": "hitPoints" }]),
    );
    let findings = validate_targeting(&manifest, &|system| match system {
        "dnd5e" => Some(vec!["hitPoints".to_string()]),
        "blades_in_the_dark" => Some(vec!["stress".to_string()]),
        _ => None,
    })
    .unwrap_err();

    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].contains("blades_in_the_dark"),
        "existing in one target does not excuse referencing it in another: {findings:?}"
    );
}

#[test]
fn an_untargeted_pack_may_not_name_anything() {
    let manifest = with_layout(
        "anywhere",
        serde_json::json!([]),
        serde_json::json!([{ "kind": "value", "id": "strength" }]),
    );
    let findings = validate(&manifest, "anywhere", &forge()).unwrap_err();
    assert!(
        findings.iter().any(|f| f.contains("strength")),
        "naming an identifier is naming a system, whatever the list says: {findings:?}"
    );
}

#[test]
fn a_generic_layout_targets_everything_and_is_accepted() {
    let manifest = with_layout(
        "anywhere",
        serde_json::json!([]),
        serde_json::json!([{ "kind": "badgeGrid", "of": "attributes" }]),
    );
    validate(&manifest, "anywhere", &forge()).expect("generic addressing names nothing");
    validate_targeting(&manifest, &|_| None).expect("and has no targets to check");
}

// ---------------------------------------------------------------------------
// Forge (FR-007, FR-007a, FR-025b)
// ---------------------------------------------------------------------------

#[test]
fn the_base_pack_validates_against_itself() {
    let forge = forge();
    validate(&forge, "forge", &forge).expect("the fallback for every world must be valid");
}

#[test]
fn the_base_pack_exercises_every_construct_it_is_allowed_to_use() {
    validate_conformance(&forge()).expect(
        "a construct the format offers and the reference pack cannot demonstrate \
         is one nobody has shown can be built",
    );
}

#[test]
fn the_base_pack_names_no_system_identifier() {
    assert!(
        forge().referenced_ids().is_empty(),
        "Forge is the fallback for a world bound to a system that ships next year"
    );
}

#[test]
fn the_base_pack_targets_every_system() {
    assert!(
        forge().targets.is_empty(),
        "an empty target list is what makes Forge the system-agnostic default of \
         FR-006 a mechanism rather than a promise"
    );
}

// ---------------------------------------------------------------------------
// T028: Forge and the stylesheet cannot drift apart
// ---------------------------------------------------------------------------

/// `--kebab-case` to the camelCase key `TokenMap` uses.
///
/// Mechanical except for the charts: `--chart-1` is `chart1`, with the digit
/// fused to the word rather than separated, because that is what the Rust
/// field is called.
fn token_key(custom_property: &str) -> String {
    let name = custom_property.trim_start_matches("--");
    let mut out = String::new();
    let mut upper_next = false;
    for c in name.chars() {
        match c {
            '-' => upper_next = true,
            c if upper_next => {
                out.extend(c.to_uppercase());
                upper_next = false;
            }
            c => out.push(c),
        }
    }
    out
}

/// The `--property: value;` declarations of one block in `globals.css`.
fn css_block(source: &str, selector: &str) -> Vec<(String, String)> {
    let start = source
        .find(selector)
        .unwrap_or_else(|| panic!("{selector} should exist in globals.css"));
    let body_start = source[start..].find('{').expect("a block") + start + 1;
    let body_end = source[body_start..].find('}').expect("a close") + body_start;

    source[body_start..body_end]
        .lines()
        .filter_map(|line| {
            let line = line.trim().trim_end_matches(';');
            let (name, value) = line.split_once(':')?;
            let name = name.trim();
            name.starts_with("--")
                .then(|| (token_key(name), value.trim().to_string()))
        })
        .collect()
}

/// Forge is the product's current look, written down. If the two drift, the
/// base pack stops being the thing it exists to be.
///
/// This repo has been bitten by exactly this shape before — MVP.md's own
/// header records a size figure that drifted 16% before anyone noticed — and a
/// transcription is the most driftable artifact there is, because nothing
/// about editing one half reminds you the other exists.
#[test]
fn the_base_pack_still_reproduces_the_stylesheet_exactly() {
    let css = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/web/src/styles/globals.css"),
    )
    .expect("globals.css should be readable");

    let forge = forge();

    for (selector, tokens) in [(":root {", &forge.light), (".dark {", &forge.dark)] {
        let declared = css_block(&css, selector);
        assert!(
            !declared.is_empty(),
            "{selector} parsed to nothing — the parser, not the pack, is wrong"
        );

        for (key, css_value) in &declared {
            // `radius` is a length rather than a colour and lives only in
            // `:root`; everything else must round-trip.
            let pack_value = if key == "radius" {
                tokens.radius.as_deref()
            } else {
                tokens.get(key)
            };

            assert_eq!(
                pack_value,
                Some(css_value.as_str()),
                "{selector} defines {key} as {css_value:?}, Forge says {pack_value:?}. \
                 The base pack is the product's current look written down; a difference \
                 here is a visible change nobody asked for."
            );
        }

        // And nothing in the pack that the stylesheet does not define, which
        // would be a token the application never consumes.
        let names: Vec<&str> = declared.iter().map(|(k, _)| k.as_str()).collect();
        for (key, _) in tokens.colours() {
            assert!(
                names.contains(&key),
                "Forge declares {key}, which {selector} does not define"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// A targeted pack (T052)
// ---------------------------------------------------------------------------

fn forged_steel() -> InterfaceManifest {
    let text = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packs/interface/forged-steel/interface.json"),
    )
    .expect("the targeted pack should exist");
    serde_json::from_str(&text).expect("forged-steel must deserialise")
}

/// 5e's declared identifiers, stored and derived, read from the pack itself.
///
/// Not a list restated here: a pack is validated against what a system
/// actually publishes, and hardcoding that set would let this test pass while
/// the real check failed.
fn dnd5e_declares() -> Vec<String> {
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packs/systems/dnd5e/system.json"),
        )
        .expect("5e's manifest"),
    )
    .expect("valid json");

    let mut ids: Vec<String> = manifest
        .get("abilities")
        .and_then(|a| a.as_object())
        .map(|block| block.keys().cloned().collect())
        .unwrap_or_default();

    if let Some(resources) = manifest.get("resources").and_then(|r| r.as_array()) {
        ids.extend(
            resources
                .iter()
                .filter_map(|r| r.get("id")?.as_str().map(str::to_string)),
        );
    }

    // The derived half, from 5e's own rules.
    let rules = dnd5e_server::DnD5eRules::from_manifest(&manifest);
    ids.extend(
        thunderforge_canvas_core::system_rules::SystemRules::derived_declarations(&rules)
            .into_iter()
            .map(|d| d.id),
    );
    ids
}

#[test]
fn the_targeted_pack_is_structurally_valid() {
    let pack = forged_steel();
    validate(&pack, "forged-steel", &forge()).expect("a shipped pack must validate");
}

/// FR-026, against the real system rather than a fixture. Every identifier
/// Forged Steel names must be one 5e actually publishes — and after T051 that
/// includes the derived half, which is most of what this pack lays out.
#[test]
fn the_targeted_pack_names_only_identifiers_5e_declares() {
    let pack = forged_steel();
    let declared = dnd5e_declares();

    validate_targeting(&pack, &|system| {
        (system == "dnd5e").then(|| declared.clone())
    })
    .expect("every name in the layout must be one 5e publishes");

    assert!(
        !pack.referenced_ids().is_empty(),
        "a targeted pack that names nothing is not exercising specific \
         addressing, and this test would pass vacuously"
    );
}

/// The two packs must actually differ, or SC-005's "visibly and structurally
/// different" is a claim nothing checks.
#[test]
fn the_two_shipped_packs_are_structurally_different() {
    let base = forge();
    let steel = forged_steel();

    assert!(base.referenced_ids().is_empty());
    assert!(!steel.referenced_ids().is_empty());
    assert!(base.targets.is_empty());
    assert_eq!(steel.targets, vec!["dnd5e".to_string()]);
    assert_ne!(base.light, steel.light, "and they do not look the same");
}

// ---------------------------------------------------------------------------
// Increment E: nothing a system publishes falls off the base pack
// ---------------------------------------------------------------------------

/// FR-034 and SC-012's mechanism, asserted rather than assumed.
///
/// Forge addresses the five named declaration sets *and* everything else. A
/// system declaring something this build has never heard of still has it
/// drawn, because a value missing from a sheet is indistinguishable from the
/// character not having it.
#[test]
fn the_base_pack_addresses_everything_a_system_can_declare() {
    let forge = forge();
    let layout = forge.layout.as_deref().expect("forge declares a layout");

    fn sets(nodes: &[crate::layout::LayoutNode], out: &mut Vec<crate::layout::DeclarationSet>) {
        use crate::layout::LayoutNode::*;
        for node in nodes {
            match node {
                Section { children, .. } | Column { children } | Row { children } => {
                    sets(children, out)
                }
                BadgeGrid { of, .. } | BarStack { of } | RowList { of } => out.push(*of),
                _ => {}
            }
        }
    }

    let mut addressed = Vec::new();
    sets(layout, &mut addressed);

    for set in [
        crate::layout::DeclarationSet::Attributes,
        crate::layout::DeclarationSet::Resources,
        crate::layout::DeclarationSet::Skills,
        crate::layout::DeclarationSet::Movement,
        crate::layout::DeclarationSet::Derived,
        crate::layout::DeclarationSet::Other,
    ] {
        assert!(
            addressed.contains(&set),
            "the base pack must address {set:?}, or a system declaring into it \
             gets a sheet with a hole in it"
        );
    }
}
