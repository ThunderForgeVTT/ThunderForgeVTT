use super::*;

fn parse(json: &str) -> Result<LayoutNode, serde_json::Error> {
    serde_json::from_str(json)
}

#[test]
fn a_generic_node_names_nothing() {
    let node = parse(r#"{"kind":"badgeGrid","of":"attributes","columns":3}"#).expect("valid");
    assert!(
        node.referenced_ids().is_empty(),
        "generic addressing is what lets one layout serve a system it has never heard of"
    );
}

#[test]
fn a_specific_node_reports_every_identifier_it_names() {
    let node = parse(
        r#"{"kind":"section","title":"Combat","children":[
             {"kind":"pair","value":"strength","beside":"strengthMod"},
             {"kind":"tracker","id":"deathSaves","boxes":3,"rows":2}
           ]}"#,
    )
    .expect("valid");

    let mut ids = node.referenced_ids();
    ids.sort_unstable();
    assert_eq!(ids, vec!["deathSaves", "strength", "strengthMod"]);
}

/// FR-003a, as a property of the type rather than a rule someone enforces.
///
/// Each of these is a real thing a pack author would want and a real thing
/// that would make the pack a program. None of them parse, and none of them
/// can be made to parse without adding a variant.
#[test]
fn nothing_that_computes_or_decides_can_be_expressed() {
    for forbidden in [
        // an expression where a reference belongs
        r#"{"kind":"value","id":"(strength - 10) / 2"}"#,
        // a conditional
        r#"{"kind":"value","id":"strength","when":"strength > 10"}"#,
        // a threshold: a claim about what a number means
        r#"{"kind":"barStack","of":"resources","dangerBelow":0.25}"#,
        // a colour ramp keyed to a value
        r#"{"kind":"barStack","of":"resources","colorBy":"value"}"#,
        // an unknown construct
        r#"{"kind":"script","src":"./sheet.js"}"#,
    ] {
        let parsed = parse(forbidden);
        match parsed {
            Err(_) => {}
            Ok(node) => {
                // The expression case is the subtle one: it parses as an
                // identifier, because any string is a legal identifier. It is
                // caught by FR-026 instead — no system declares a value called
                // "(strength - 10) / 2", so the pack is rejected naming it.
                assert_eq!(
                    node.referenced_ids(),
                    vec!["(strength - 10) / 2"],
                    "the only thing that may parse here is a reference, and an \
                     expression parsed as one is caught by targeting validation"
                );
            }
        }
    }
}

/// `deny_unknown_fields` is doing real work: a misspelling is a rejection, not
/// a value that quietly does nothing.
#[test]
fn an_unknown_field_is_refused_rather_than_ignored() {
    assert!(parse(r#"{"kind":"badgeGrid","of":"attributes","colums":3}"#).is_err());
}

#[test]
fn every_kind_the_format_offers_is_named_in_all_kinds() {
    // Each variant, once, so a new construct that is not added to ALL_KINDS
    // shrinks what Forge must demonstrate without anyone noticing.
    let one_of_each: Vec<LayoutNode> = vec![LayoutNode::Section {
        title: None,
        collapsed: false,
        children: vec![
            LayoutNode::Column { children: vec![] },
            LayoutNode::Row { children: vec![] },
            LayoutNode::BadgeGrid {
                of: DeclarationSet::Attributes,
                columns: None,
            },
            LayoutNode::BarStack {
                of: DeclarationSet::Resources,
            },
            LayoutNode::RowList {
                of: DeclarationSet::Skills,
            },
            LayoutNode::Value {
                id: "a".to_string(),
            },
            LayoutNode::Pair {
                value: "a".to_string(),
                beside: "b".to_string(),
            },
            LayoutNode::Tracker {
                id: "a".to_string(),
                boxes: 3,
                rows: 2,
            },
            LayoutNode::SlotGrid {
                id: "a".to_string(),
                levels: 9,
            },
        ],
    }];

    let mut present = LayoutNode::kinds_present(&one_of_each);
    present.sort_unstable();
    let mut all = LayoutNode::ALL_KINDS.to_vec();
    all.sort_unstable();
    assert_eq!(present, all);
}

#[test]
fn declaration_order_is_the_systems_and_a_pack_cannot_restate_it() {
    // There is no `order` field anywhere in a generic construct. A system
    // lists its abilities the way its book does; a pack reordering them would
    // be making a claim about the ruleset.
    let node = parse(r#"{"kind":"badgeGrid","of":"attributes","order":["cunning"]}"#);
    assert!(node.is_err());
}
