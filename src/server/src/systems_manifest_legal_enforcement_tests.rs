use super::*;
use crate::test_support::test_app_state;
use axum::extract::{Path as AxumPath, State};

fn state_with_temp_systems_dir() -> (AppState, std::path::PathBuf) {
    let mut state = test_app_state();
    let tmp = std::env::temp_dir().join(format!("tf-systems-test-{}", uuid::Uuid::now_v7()));
    state.directories.systems_dir = tmp.to_str().unwrap().to_string();
    (state, tmp)
}

fn write_manifest(systems_dir: &std::path::Path, slug: &str, manifest_json: &str) {
    let pack_dir = systems_dir.join(slug);
    std::fs::create_dir_all(&pack_dir).unwrap();
    std::fs::write(pack_dir.join("system.json"), manifest_json).unwrap();
}

/// Spec 032 T085: the list comes from the directory, not the table.
///
/// The point of this test is what it does *not* do — it never touches
/// `game_systems`. That table holds zero rows on every install, which is
/// why the client had to carry a hand-kept list of all seven systems; a
/// pack written into a temp directory and seeded nowhere must be listed.
#[test]
fn list_installed_reports_a_pack_that_was_never_seeded_into_the_table() {
    let (_state, systems_dir) = state_with_temp_systems_dir();
    write_manifest(
        &systems_dir,
        "unseeded-pack",
        r#"{"id": "unseeded-pack", "title": "Unseeded Pack",
            "description": "Never inserted anywhere.", "version": "2.1.0"}"#,
    );

    let listed = list_installed(systems_dir.to_str().unwrap());

    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "unseeded-pack");
    assert_eq!(listed[0].title, "Unseeded Pack");
    assert_eq!(listed[0].version, "2.1.0");
    assert_eq!(listed[0].description, "Never inserted anywhere.");
}

/// Title order, not directory order, which is whatever the filesystem
/// hands back. A picker that reorders itself between two machines is a
/// picker nobody can be told where to click.
#[test]
fn list_installed_orders_by_title() {
    let (_state, systems_dir) = state_with_temp_systems_dir();
    for (slug, title) in [("zzz", "Aardvark"), ("aaa", "Zeppelin"), ("mmm", "Middle")] {
        write_manifest(
            &systems_dir,
            slug,
            &format!(r#"{{"id": "{slug}", "title": "{title}", "version": "1.0.0"}}"#),
        );
    }

    let listed = list_installed(systems_dir.to_str().unwrap());
    let titles: Vec<&str> = listed.iter().map(|s| s.title.as_str()).collect();

    assert_eq!(titles, vec!["Aardvark", "Middle", "Zeppelin"]);
}

/// A directory that is not a readable pack is omitted, not listed with a
/// blank name. Offering a Game Master something that cannot be chosen is
/// worse than not offering it.
#[test]
fn list_installed_omits_a_pack_it_cannot_read() {
    let (_state, systems_dir) = state_with_temp_systems_dir();
    write_manifest(&systems_dir, "broken", "{ this is not json");
    write_manifest(
        &systems_dir,
        "untitled",
        r#"{"id": "untitled", "version": "1.0.0"}"#,
    );
    write_manifest(
        &systems_dir,
        "fine",
        r#"{"id": "fine", "title": "Fine", "version": "1.0.0"}"#,
    );

    let listed = list_installed(systems_dir.to_str().unwrap());
    let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();

    assert_eq!(ids, vec!["fine"]);
}

/// A pack may say it is a starting point rather than a ruleset, and be
/// believed. `basic-game-system` is the one that does.
///
/// The declaration is the mechanism on purpose: shared code omitting a
/// pack by name would put back exactly the hardcoded knowledge T085 took
/// out, and this test would pass either way — which is why the second
/// assertion is here, naming a template this file has never heard of.
#[test]
fn list_installed_omits_a_pack_that_declares_itself_a_template() {
    let (_state, systems_dir) = state_with_temp_systems_dir();
    write_manifest(
        &systems_dir,
        "starting-point",
        r#"{"id": "starting-point", "title": "Starting Point",
            "version": "1.0.0", "template": true}"#,
    );
    write_manifest(
        &systems_dir,
        "a-ruleset",
        r#"{"id": "a-ruleset", "title": "A Ruleset", "version": "1.0.0"}"#,
    );

    let listed = list_installed(systems_dir.to_str().unwrap());
    let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();

    assert_eq!(ids, vec!["a-ruleset"]);
}

/// The bundled packs, listed from the real directory this deployment
/// ships — the case the client used to answer from a literal.
#[test]
fn the_bundled_packs_directory_lists_every_shipping_system() {
    let packs = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .canonicalize()
        .expect("packs/systems must exist");

    let listed = list_installed(packs.to_str().unwrap());

    // Every directory that is not a declared template, and nothing else.
    let expected = std::fs::read_dir(&packs)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            let text = std::fs::read_to_string(e.path().join("system.json")).unwrap();
            let m: serde_json::Value = serde_json::from_str(&text).unwrap();
            m.get("template").and_then(serde_json::Value::as_bool) != Some(true)
        })
        .count();

    assert_eq!(listed.len(), expected);
    assert!(
        listed.len() >= 7,
        "expected the shipping rulesets, got {listed:?}"
    );
    assert!(
        !listed.iter().any(|s| s.id == "basic-game-system"),
        "the template pack must not be offered as a ruleset"
    );
    assert!(listed.iter().all(|s| !s.title.is_empty()));
}

#[tokio::test]
async fn get_system_manifest_rejects_a_manifest_missing_legal() {
    let (state, systems_dir) = state_with_temp_systems_dir();
    write_manifest(
        &systems_dir,
        "no-legal-pack",
        r#"{"id": "no-legal-pack", "title": "No Legal Pack", "version": "0.1.0"}"#,
    );

    let result = get_system_manifest(AxumPath("no-legal-pack".to_string()), State(state)).await;

    let (status, _) = result.expect_err("manifest missing legal must be rejected");
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn get_system_manifest_serves_a_manifest_with_valid_legal() {
    let (state, systems_dir) = state_with_temp_systems_dir();
    write_manifest(
        &systems_dir,
        "compliant-pack",
        r#"{
            "id": "compliant-pack",
            "title": "Compliant Pack",
            "version": "0.1.0",
            "legal": {
                "licenseName": "CC-BY-4.0",
                "attributionText": "Built from an open reference document."
            }
        }"#,
    );

    let result = get_system_manifest(AxumPath("compliant-pack".to_string()), State(state)).await;

    let Json(manifest) = result.expect("a compliant manifest must be served");
    assert_eq!(manifest["legal"]["licenseName"], "CC-BY-4.0");
}

/// Spec 016 (T006, SC-001): the real, shipped `dnd5e` manifest — not a
/// synthetic fixture — has a compliant `legal` object.
#[tokio::test]
async fn dnd5e_system_json_has_a_compliant_legal_object() {
    let mut state = test_app_state();
    // test_app_state()'s Directories::from(temp_dir()) computes
    // systems_dir under the temp dir, not this repo's real
    // packs/systems — point it at the real one so this exercises the
    // actual shipped manifest, not a fixture.
    let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .canonicalize()
        .expect("packs/systems must exist relative to src/server");
    state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

    let result = get_system_manifest(AxumPath("dnd5e".to_string()), State(state)).await;

    let Json(manifest) = result.expect("dnd5e's real manifest must pass legal validation");
    assert_eq!(manifest["legal"]["licenseName"], "CC-BY-4.0");
    assert!(
        manifest["legal"]["attributionText"]
            .as_str()
            .unwrap()
            .contains("System Reference Document")
    );
}

/// Spec 018 (T014): the real, shipped `genie` manifest has a compliant
/// `legal` object declaring original, ThunderForgeVTT-owned content —
/// mirrors dnd5e_system_json_has_a_compliant_legal_object above.
#[tokio::test]
async fn genie_system_json_has_a_compliant_legal_object() {
    let mut state = test_app_state();
    let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .canonicalize()
        .expect("packs/systems must exist relative to src/server");
    state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

    let result = get_system_manifest(AxumPath("genie".to_string()), State(state)).await;

    let Json(manifest) = result.expect("genie's real manifest must pass legal validation");
    assert_eq!(
        manifest["legal"]["licenseName"],
        "ThunderForgeVTT Original Content"
    );
    assert!(
        manifest["legal"]["trademarkRestrictions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// Spec 016 (Edge Cases, "no external license at all"): the real,
/// shipped `basic-game-system` manifest — a minimal, generic starter
/// template pack with no third-party-derived content — has a compliant
/// `legal` object declaring original, ThunderForgeVTT-owned content.
/// Mirrors genie_system_json_has_a_compliant_legal_object above.
#[tokio::test]
async fn basic_game_system_json_has_a_compliant_legal_object() {
    let mut state = test_app_state();
    let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .canonicalize()
        .expect("packs/systems must exist relative to src/server");
    state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

    let result = get_system_manifest(AxumPath("basic-game-system".to_string()), State(state)).await;

    let Json(manifest) =
        result.expect("basic-game-system's real manifest must pass legal validation");
    assert_eq!(
        manifest["legal"]["licenseName"],
        "ThunderForgeVTT Original Content"
    );
    assert!(
        manifest["legal"]["trademarkRestrictions"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

/// Shared helper for the five research-digest-backed packs below —
/// mirrors dnd5e/genie's own compliance tests but parameterized, since
/// all five follow the identical assertion shape (real manifest,
/// pointed at the actual packs/systems dir, checked against the
/// licenseName recorded in the corresponding research digest).
async fn assert_manifest_has_license(system_id: &str, expected_license_name: &str) {
    let mut state = test_app_state();
    let real_systems_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .canonicalize()
        .expect("packs/systems must exist relative to src/server");
    state.directories.systems_dir = real_systems_dir.to_str().unwrap().to_string();

    let result = get_system_manifest(AxumPath(system_id.to_string()), State(state)).await;

    let Json(manifest) =
        result.unwrap_or_else(|_| panic!("{system_id}'s real manifest must pass legal validation"));
    assert_eq!(manifest["legal"]["licenseName"], expected_license_name);
}

/// Spec: research/system_pathfinder2e.json's `legal.licenseName`.
#[tokio::test]
async fn pathfinder2e_system_json_has_a_compliant_legal_object() {
    assert_manifest_has_license("pathfinder2e", "Open RPG Creative License (ORC)").await;
}

/// Spec: research/system_cypher_system.json's `legal.licenseName`.
#[tokio::test]
async fn cypher_system_json_has_a_compliant_legal_object() {
    assert_manifest_has_license("cypher_system", "Cypher System Open License").await;
}

/// Spec: research/system_fate_core.json's `legal.licenseName`.
#[tokio::test]
async fn fate_core_system_json_has_a_compliant_legal_object() {
    assert_manifest_has_license(
        "fate_core",
        "Creative Commons Attribution 3.0 Unported license",
    )
    .await;
}

/// Spec: research/system_blades_in_the_dark.json's `legal.licenseName`.
#[tokio::test]
async fn blades_in_the_dark_system_json_has_a_compliant_legal_object() {
    assert_manifest_has_license(
        "blades_in_the_dark",
        "Creative Commons Attribution 3.0 Unported (CC BY 3.0)",
    )
    .await;
}

/// Spec: research/system_year_zero_engine.json's `legal.licenseName`.
#[tokio::test]
async fn year_zero_engine_system_json_has_a_compliant_legal_object() {
    assert_manifest_has_license("year_zero_engine", "Year Zero Engine Free Tabletop License").await;
}
