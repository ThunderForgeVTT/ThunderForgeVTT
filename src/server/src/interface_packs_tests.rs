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
