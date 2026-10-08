//! Spec 084 T010: the host side of the roll-facets slot.

use std::sync::Mutex;

use thunderforge_canvas_core::roll_facets::{
    Advantage, FacetLabel, RerollInput, RerollPlan, RollFacets, RollKind, ShapeInput, Shaped,
};
use thunderforge_canvas_core::system_contribution::SystemContribution;

use super::*;

/// What the test slot was last called with: kind, formula, advantage,
/// melee, item properties and the sheet's `marker`.
type Seen = (
    RollKind,
    String,
    Advantage,
    bool,
    Vec<String>,
    serde_json::Value,
);
static SEEN: Mutex<Option<Seen>> = Mutex::new(None);

fn test_shape(input: &ShapeInput<'_>) -> Result<Option<Shaped>, String> {
    // Only the one test that reads `SEEN` rolls damage, so tests running
    // side by side cannot overwrite what it looks at.
    if input.kind == RollKind::Damage {
        *SEEN.lock().unwrap() = Some((
            input.kind,
            input.formula.to_string(),
            input.advantage,
            input.melee,
            input.item_properties.to_vec(),
            input.trait_data["marker"].clone(),
        ));
    }
    if input.formula == "refuse" {
        return Err("No.".to_string());
    }
    if input.formula == "untouched" {
        return Ok(None);
    }
    Ok(Some(Shaped {
        formula: format!("{} + 1", input.formula),
        facets: vec!["blessed".to_string()],
    }))
}

fn test_reroll(_: &RerollInput<'_>) -> Result<RerollPlan, String> {
    Err("never".to_string())
}

static TEST_FACETS: RollFacets = RollFacets {
    shape: test_shape,
    reroll: test_reroll,
    spends: &[FacetLabel {
        id: "token",
        label: "A Token",
    }],
    labels: &[FacetLabel {
        id: "blessed",
        label: "Blessed",
    }],
};

inventory::submit! {
    SystemContribution {
        roll_facets: Some(&TEST_FACETS),
        ..SystemContribution::new("test-facets")
    }
}

fn input<'a>(formula: &'a str, advantage: Advantage) -> ShapeInput<'a> {
    static NULL: serde_json::Value = serde_json::Value::Null;
    ShapeInput {
        kind: RollKind::Check,
        formula,
        trait_data: &NULL,
        advantage,
        melee: false,
        item_properties: &[],
    }
}

#[test]
fn a_system_without_the_slot_rolls_the_formula_as_written() {
    let shaped = shape_roll("no-such-system", input("1d20 + STR", Advantage::Normal)).unwrap();
    assert_eq!(shaped.formula, "1d20 + STR");
    assert!(shaped.facets.is_empty());
}

#[test]
fn a_system_without_the_slot_refuses_advantage() {
    for advantage in [Advantage::Advantage, Advantage::Disadvantage] {
        assert_eq!(
            shape_roll("no-such-system", input("1d20", advantage)).unwrap_err(),
            "This system does not roll with advantage."
        );
    }
}

#[test]
fn the_slot_is_called_with_what_the_host_knows() {
    let sheet = serde_json::json!({ "marker": "ayla" });
    let properties = vec!["two_handed".to_string()];
    let shaped = shape_roll(
        "test-facets",
        ShapeInput {
            kind: RollKind::Damage,
            formula: "2d6 + STR",
            trait_data: &sheet,
            advantage: Advantage::Normal,
            melee: true,
            item_properties: &properties,
        },
    )
    .unwrap();
    assert_eq!(shaped.formula, "2d6 + STR + 1");
    assert_eq!(shaped.facets, vec!["blessed".to_string()]);
    let seen = SEEN.lock().unwrap().clone().unwrap();
    assert_eq!(
        seen,
        (
            RollKind::Damage,
            "2d6 + STR".to_string(),
            Advantage::Normal,
            true,
            properties,
            serde_json::json!("ayla"),
        )
    );
}

#[test]
fn the_slot_may_leave_a_roll_alone_or_refuse_it() {
    let untouched = shape_roll("test-facets", input("untouched", Advantage::Advantage)).unwrap();
    assert_eq!(untouched.formula, "untouched");
    assert!(untouched.facets.is_empty());
    assert_eq!(
        shape_roll("test-facets", input("refuse", Advantage::Normal)).unwrap_err(),
        "No."
    );
}

#[test]
fn labels_come_from_the_pack_and_an_unknown_id_names_itself() {
    let ids = ["blessed", "token", "mystery"].map(String::from);
    assert_eq!(
        facet_labels("test-facets", &ids),
        vec![
            ("blessed".to_string(), "Blessed".to_string()),
            ("token".to_string(), "A Token".to_string()),
            ("mystery".to_string(), "mystery".to_string()),
        ]
    );
    assert_eq!(
        facet_labels("no-such-system", &ids[..1]),
        vec![("blessed".to_string(), "blessed".to_string())]
    );
}
