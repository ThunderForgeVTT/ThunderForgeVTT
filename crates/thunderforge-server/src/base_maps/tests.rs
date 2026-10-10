use super::*;

const CREDIT: &str = r#"{
  "author": "MBRound18",
  "licence": "CC BY-SA 4.0",
  "licenceUrl": "https://creativecommons.org/licenses/by-sa/4.0/",
  "source": "https://github.com/mbround18/vtt-maps",
  "catalog": "https://vtt-maps.dnd-apps.dev/catalog"
}"#;

fn listing(names: &[&str]) -> String {
    let maps: Vec<serde_json::Value> = names
        .iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "width": 4096,
                "height": 2304,
                "gridSize": 128,
                "ambientLight": "dim",
                "walls": [
                    {"x1": -10.0, "y1": 0.0, "x2": 10.0, "y2": 0.0,
                     "blocksVision": true, "blocksMovement": true, "doorState": "none"},
                    {"x1": 0.0, "y1": -10.0, "x2": 0.0, "y2": 10.0,
                     "blocksVision": true, "blocksMovement": true, "doorState": "closed"}
                ],
                "lights": [
                    {"x": 1.0, "y": 2.0, "radius": 256.0, "brightRadius": 128.0,
                     "intensity": 1.0, "color": "#ffaa00", "castsShadows": true}
                ],
                "hasPreview": true,
                "byteSize": 3
            })
        })
        .collect();
    serde_json::Value::Array(maps).to_string()
}

/// A directory as `make base-maps` writes it, with the named maps' images.
fn directory(names: &[&str], images: &[&str]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("maps.json"), listing(names)).unwrap();
    std::fs::write(dir.path().join("credit.json"), CREDIT).unwrap();
    std::fs::write(dir.path().join("NOTICE.txt"), "notice").unwrap();
    for image in images {
        std::fs::write(dir.path().join(format!("{image}.webp")), b"RIFF").unwrap();
        std::fs::write(dir.path().join(format!("{image}.thumb.webp")), b"RIFF").unwrap();
    }
    dir
}

#[test]
fn a_full_directory_lists_every_map_with_its_name_geometry_and_credit() {
    let dir = directory(
        &["grassy-path-ambush", "dwarven-forge"],
        &["grassy-path-ambush", "dwarven-forge"],
    );
    let maps = BaseMaps::load(Some(dir.path()));

    assert_eq!(maps.list().len(), 2);
    let grassy = maps.get("grassy-path-ambush").expect("listed");
    assert_eq!(grassy.name, "Grassy Path Ambush");
    assert_eq!(
        (grassy.width, grassy.height, grassy.grid_size),
        (4096, 2304, 128)
    );
    assert_eq!(grassy.ambient_light, "dim");
    assert_eq!(grassy.walls.len(), 2);
    assert_eq!(grassy.walls[1].door_state, "closed");
    assert_eq!(grassy.lights.len(), 1);
    assert_eq!(
        grassy.thumbnail_url(),
        "/api/base-maps/grassy-path-ambush.thumb.webp"
    );
    assert_eq!(maps.default_id(), Some("grassy-path-ambush"));
    assert_eq!(maps.credit().expect("credit").author, "MBRound18");
    assert_eq!(
        maps.picture_path("dwarven-forge", Picture::Thumbnail),
        Some(dir.path().join("dwarven-forge.thumb.webp"))
    );
}

#[test]
fn the_operator_can_choose_the_default_or_none() {
    let dir = directory(
        &["grassy-path-ambush", "dwarven-forge"],
        &["grassy-path-ambush", "dwarven-forge"],
    );
    let load = || BaseMaps::load(Some(dir.path()));

    assert_eq!(
        load().with_default(None).default_id(),
        Some("grassy-path-ambush")
    );
    assert_eq!(
        load().with_default(Some("dwarven-forge")).default_id(),
        Some("dwarven-forge")
    );
    assert_eq!(load().with_default(Some("none")).default_id(), None);
    assert_eq!(load().with_default(Some("")).default_id(), None);
    // A default that is not in the set is no default, never a guess.
    assert_eq!(load().with_default(Some("not-a-map")).default_id(), None);
    // The maps are still offered either way.
    assert_eq!(load().with_default(Some("none")).list().len(), 2);
}

#[test]
fn no_directory_configured_is_the_empty_set() {
    let maps = BaseMaps::load(None);
    assert!(maps.list().is_empty());
    assert_eq!(maps.default_id(), None);
    assert!(maps.credit().is_none());
    assert!(maps.notice_path().is_none());
}

#[test]
fn a_missing_directory_is_the_empty_set() {
    let dir = tempfile::tempdir().unwrap();
    let maps = BaseMaps::load(Some(&dir.path().join("not-there")));
    assert!(maps.list().is_empty());
    assert_eq!(maps.default_id(), None);
}

#[test]
fn a_listing_that_does_not_parse_is_the_empty_set() {
    let dir = directory(&["grassy-path-ambush"], &["grassy-path-ambush"]);
    std::fs::write(dir.path().join("maps.json"), "{ not json").unwrap();
    assert!(BaseMaps::load(Some(dir.path())).list().is_empty());
}

#[test]
fn a_set_with_no_credit_is_not_offered() {
    let dir = directory(&["grassy-path-ambush"], &["grassy-path-ambush"]);
    std::fs::remove_file(dir.path().join("credit.json")).unwrap();
    assert!(BaseMaps::load(Some(dir.path())).list().is_empty());
}

#[test]
fn a_map_whose_image_is_missing_is_left_out_and_the_rest_stay() {
    let dir = directory(&["grassy-path-ambush", "dwarven-forge"], &["dwarven-forge"]);
    let maps = BaseMaps::load(Some(dir.path()));
    assert_eq!(
        maps.list()
            .iter()
            .map(|m| m.id.as_str())
            .collect::<Vec<_>>(),
        ["dwarven-forge"]
    );
    // The default is gone, so the form selects None.
    assert_eq!(maps.default_id(), None);
}

#[test]
fn an_id_that_is_a_path_never_reaches_the_filesystem() {
    let dir = directory(&["grassy-path-ambush"], &["grassy-path-ambush"]);
    let maps = BaseMaps::load(Some(dir.path()));
    for id in ["..", "../maps", "grassy-path-ambush/../x", "", "NOTICE"] {
        assert_eq!(maps.picture_path(id, Picture::Full), None, "{id}");
    }
}

#[test]
fn a_listed_name_that_is_not_a_plain_id_is_left_out() {
    let dir = directory(
        &["../escape", "grassy-path-ambush"],
        &["grassy-path-ambush"],
    );
    std::fs::write(dir.path().join("../escape.webp"), b"RIFF").ok();
    let maps = BaseMaps::load(Some(dir.path()));
    assert_eq!(maps.list().len(), 1);
}

#[test]
fn names_are_title_cased_from_the_stem() {
    assert_eq!(title_case("little-fish-academy"), "Little Fish Academy");
    assert_eq!(title_case("road-side-in"), "Road Side In");
    assert_eq!(title_case("demo"), "Demo");
}
