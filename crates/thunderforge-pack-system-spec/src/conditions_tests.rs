use serde_json::json;

use super::*;

fn manifest(conditions: Value) -> Value {
    json!({ "conditions": conditions })
}

fn condition(id: &str) -> Value {
    json!({
        "id": id,
        "label": "Poisoned",
        "marker": { "glyph": "dot", "color": "danger" },
    })
}

#[test]
fn a_manifest_with_no_block_declares_nothing_and_is_valid() {
    assert!(conditions_from_manifest(&json!({})).is_empty());
    assert!(validate_conditions_content(&json!({})).is_ok());
}

#[test]
fn conditions_read_in_the_manifests_order_with_their_markers() {
    let read = conditions_from_manifest(&manifest(json!([
        condition("poisoned"),
        {
            "id": "prone",
            "label": "Prone",
            "description": "Lying on the ground.",
            "marker": { "glyph": "bar", "color": "neutral" },
        },
    ])));
    let ids: Vec<&str> = read.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["poisoned", "prone"]);
    assert_eq!(read[1].marker.glyph, ConditionGlyph::Bar);
    assert_eq!(read[1].marker.color, ConditionColor::Neutral);
    assert_eq!(read[1].description.as_deref(), Some("Lying on the ground."));
}

#[test]
fn an_unreadable_block_declares_nothing_rather_than_panicking() {
    assert!(conditions_from_manifest(&manifest(json!("nonsense"))).is_empty());
}

#[test]
fn the_old_map_shape_is_refused() {
    let err = validate_conditions_content(&manifest(json!({
        "bound": { "label": "Bound" }
    })))
    .expect_err("a map keyed by id is the shape this replaced");
    assert!(err.starts_with("conditions:"), "{err}");
}

#[test]
fn a_repeated_id_is_refused() {
    let err = validate_conditions_content(&manifest(json!([condition("a"), condition("a")])))
        .expect_err("two conditions may not share an id");
    assert!(err.contains("declared twice"), "{err}");
}

#[test]
fn a_blank_label_is_refused() {
    let mut blank = condition("a");
    blank["label"] = json!("  ");
    let err = validate_conditions_content(&manifest(json!([blank])))
        .expect_err("a condition nobody can read is refused");
    assert!(err.contains("a label is required"), "{err}");
}

#[test]
fn a_condition_with_no_marker_is_refused() {
    let err = validate_conditions_content(&manifest(json!([
        { "id": "a", "label": "A" }
    ])))
    .expect_err("the board has to be told what to draw");
    assert!(err.contains("marker"), "{err}");
}

#[test]
fn a_glyph_or_colour_outside_the_lists_is_refused() {
    for marker in [
        json!({ "glyph": "skull", "color": "danger" }),
        json!({ "glyph": "dot", "color": "#ff0000" }),
    ] {
        let mut odd = condition("a");
        odd["marker"] = marker;
        assert!(validate_conditions_content(&manifest(json!([odd]))).is_err());
    }
}

#[test]
fn the_names_a_manifest_writes_are_the_names_the_board_is_handed() {
    let read = conditions_from_manifest(&manifest(json!([condition("a")])));
    assert_eq!(read[0].marker.glyph.as_str(), "dot");
    assert_eq!(read[0].marker.color.as_str(), "danger");
}
