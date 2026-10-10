use serde_json::json;

use super::*;

#[test]
fn a_reading_within_the_bounds_passes() {
    check_bounds(&json!({ "notes": [{ "text": "short" }] })).unwrap();
}

#[test]
fn a_reading_nested_too_deep_is_refused() {
    let mut value = json!(1);
    for _ in 0..=MAX_DEPTH {
        value = json!([value]);
    }
    assert_eq!(
        check_bounds(&value).unwrap_err().code(),
        "VALIDATION_FAILED"
    );
}

#[test]
fn a_long_text_or_list_is_refused() {
    let text = "x".repeat(MAX_STRING + 1);
    assert!(check_bounds(&json!({ "backstory": text })).is_err());
    let list = vec![0; MAX_LIST + 1];
    assert!(check_bounds(&json!({ "content": list })).is_err());
}

#[test]
fn an_unknown_field_in_a_reading_is_refused() {
    let error = parse_reading(&json!({ "surprise": true })).unwrap_err();
    assert_eq!(error.code(), "VALIDATION_FAILED");
}

#[test]
fn no_corrections_is_an_empty_map() {
    assert!(parse_corrections(None).unwrap().is_empty());
    assert!(parse_corrections(Some(&json!(null))).unwrap().is_empty());
    assert_eq!(
        parse_corrections(Some(&json!({ "abilities.str": 15 })))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn the_flag_off_is_feature_disabled() {
    assert_eq!(
        require_enabled(false).unwrap_err().code(),
        "FEATURE_DISABLED"
    );
    require_enabled(true).unwrap();
}
