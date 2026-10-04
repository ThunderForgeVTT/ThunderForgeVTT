use serde_json::json;

use super::*;

fn manifest(settings: Value) -> Value {
    json!({ "settings": settings })
}

fn boolean(id: &str) -> Value {
    json!({ "id": id, "label": "Inspiration", "type": "boolean", "default": true })
}

#[test]
fn a_manifest_with_no_block_declares_nothing_and_is_valid() {
    assert!(settings_from_manifest(&json!({})).is_empty());
    assert!(validate_settings_content(&json!({})).is_ok());
}

#[test]
fn settings_read_in_declared_order_with_ties_as_written() {
    let read = settings_from_manifest(&manifest(json!([
        { "id": "late", "label": "Late", "type": "boolean", "default": false, "order": 5 },
        { "id": "first", "label": "First", "type": "boolean", "default": false },
        { "id": "second", "label": "Second", "type": "boolean", "default": false },
    ])));
    let ids: Vec<&str> = read.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, ["first", "second", "late"]);
}

#[test]
fn an_unreadable_block_declares_nothing_rather_than_panicking() {
    assert!(settings_from_manifest(&manifest(json!("nonsense"))).is_empty());
}

#[test]
fn a_repeated_id_is_refused() {
    let err = validate_settings_content(&manifest(json!([boolean("a"), boolean("a")])))
        .expect_err("two settings may not share an id");
    assert!(err.contains("declared twice"), "{err}");
}

#[test]
fn an_unknown_type_is_refused() {
    let err = validate_settings_content(&manifest(json!([
        { "id": "a", "label": "A", "type": "colour", "default": "red" }
    ])))
    .expect_err("an unknown type cannot be rendered");
    assert!(err.starts_with("settings:"), "{err}");
}

#[test]
fn a_default_its_own_declaration_refuses_is_refused() {
    let err = validate_settings_content(&manifest(json!([
        { "id": "rests", "label": "Rests", "type": "integer", "default": 9, "max": 3 }
    ])))
    .expect_err("a default above max is not a default");
    assert!(err.contains("settings.rests"), "{err}");
}

#[test]
fn a_choice_without_options_is_refused() {
    let err = validate_settings_content(&manifest(json!([
        { "id": "mode", "label": "Mode", "type": "choice", "default": "a" }
    ])))
    .expect_err("a choice with nothing to choose is not a choice");
    assert!(err.contains("needs options"), "{err}");
}

#[test]
fn each_type_allows_its_own_values_and_refuses_the_rest() {
    let read = settings_from_manifest(&manifest(json!([
        boolean("flag"),
        { "id": "count", "label": "Count", "type": "integer", "default": 1, "min": 0, "max": 3 },
        { "id": "mode", "label": "Mode", "type": "choice", "default": "a",
          "options": [{ "value": "a", "label": "A" }, { "value": "b", "label": "B" }] },
        { "id": "note", "label": "Note", "type": "text", "default": "", "maxLength": 3 },
    ])));
    let by = |id: &str| read.iter().find(|s| s.id == id).expect("declared");

    assert!(by("flag").check(&json!(false)).is_ok());
    assert!(by("flag").check(&json!("false")).is_err());

    assert!(by("count").check(&json!(3)).is_ok());
    assert!(by("count").check(&json!(4)).is_err());
    assert!(by("count").check(&json!(-1)).is_err());
    assert!(by("count").check(&json!(1.5)).is_err());

    assert!(by("mode").check(&json!("b")).is_ok());
    assert!(by("mode").check(&json!("c")).is_err());

    assert!(by("note").check(&json!("abc")).is_ok());
    assert!(by("note").check(&json!("abcd")).is_err());
    assert!(by("note").check(&json!(3)).is_err());
}

#[test]
fn a_refusal_names_the_setting() {
    let read = settings_from_manifest(&manifest(json!([boolean("flag")])));
    let err = read[0].check(&json!(3)).expect_err("3 is not on or off");
    assert!(err.contains("Inspiration"), "{err}");
}
