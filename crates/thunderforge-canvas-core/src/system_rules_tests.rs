//! What the resolver guarantees, tested without a database — which is the
//! property that put this contract in this crate rather than in the engine.

use super::*;

fn declaration(id: &str, order: usize) -> AttributeDeclaration {
    AttributeDeclaration {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        source: id.to_string(),
        order,
    }
}

fn stored(id: &str, value: i32) -> DeclaredValue {
    DeclaredValue {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        value: DeclaredValueKind::Integer(value),
        origin: Origin::Stored,
    }
}

fn derived(id: &str, value: i32) -> DeclaredValue {
    DeclaredValue {
        origin: Origin::Derived,
        ..stored(id, value)
    }
}

/// A system that halves whatever it is given, and declares that it will.
struct Halver;

impl SystemRules for Halver {
    fn id(&self) -> &str {
        "halver"
    }

    fn derived_declarations(&self) -> Vec<AttributeDeclaration> {
        vec![declaration("half", 0)]
    }

    fn derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue> {
        // Omitted rather than zeroed when the input is absent — an unfilled
        // sheet is not a character with a score of nothing.
        match stored.integer("whole") {
            Some(whole) => vec![derived("half", whole / 2)],
            None => Vec::new(),
        }
    }
}

/// A system that returns a value it never declared. This is a bug, and the
/// resolver's job is to make it a contained one.
struct Undeclared;

impl SystemRules for Undeclared {
    fn id(&self) -> &str {
        "undeclared"
    }

    fn derived_declarations(&self) -> Vec<AttributeDeclaration> {
        vec![declaration("promised", 0)]
    }

    fn derive(&self, _stored: &DeclaredValues) -> Vec<DeclaredValue> {
        vec![derived("promised", 1), derived("smuggled", 99)]
    }
}

#[test]
fn a_system_with_no_rules_yields_exactly_what_was_stored() {
    let values = vec![stored("whole", 10)];
    let out = resolve(None, values.clone(), &DeclaredValues::new(values));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].origin, Origin::Stored);
}

#[test]
fn derived_values_join_the_stored_ones_and_say_which_they_are() {
    let values = vec![stored("whole", 10)];
    let out = resolve(Some(&Halver), values.clone(), &DeclaredValues::new(values));

    let half = out
        .iter()
        .find(|v| v.id == "half")
        .expect("half is derived");
    assert_eq!(half.value.as_integer(), Some(5));
    assert_eq!(half.origin, Origin::Derived);

    let whole = out
        .iter()
        .find(|v| v.id == "whole")
        .expect("whole is stored");
    assert_eq!(whole.origin, Origin::Stored);
}

/// T008, and the reason `derived_declarations` is stated separately from
/// `derive` at all.
///
/// An interface pack's layout is validated against the declarations, so a
/// value outside them has no declared place to appear and no label anybody
/// approved. Rendering it anyway would put a number on a sheet that no pack
/// could ever have been checked against.
#[test]
fn a_value_the_system_never_declared_is_dropped_rather_than_rendered() {
    let out = resolve(Some(&Undeclared), Vec::new(), &DeclaredValues::default());

    assert!(
        out.iter().any(|v| v.id == "promised"),
        "a declared derivation is kept"
    );
    assert!(
        !out.iter().any(|v| v.id == "smuggled"),
        "an undeclared one must not reach a surface: no pack was validated against it"
    );
}

/// The disagreement this whole contract exists to prevent, in its smallest form.
#[test]
fn a_derived_value_never_overwrites_a_stored_one_of_the_same_name() {
    struct Shadower;
    impl SystemRules for Shadower {
        fn id(&self) -> &str {
            "shadower"
        }
        fn derived_declarations(&self) -> Vec<AttributeDeclaration> {
            vec![declaration("whole", 0)]
        }
        fn derive(&self, _stored: &DeclaredValues) -> Vec<DeclaredValue> {
            vec![derived("whole", 999)]
        }
    }

    let values = vec![stored("whole", 10)];
    let out = resolve(
        Some(&Shadower),
        values.clone(),
        &DeclaredValues::new(values),
    );
    assert_eq!(out.len(), 1, "one identifier, one value");
    assert_eq!(out[0].value.as_integer(), Some(10), "the typed-in one wins");
    assert_eq!(out[0].origin, Origin::Stored);
}

/// Purity, as far as a test can hold it: the same input, the same output,
/// every time. A rule that consulted a clock or a database would fail this
/// only intermittently, which is why the requirement is also written on the
/// trait in prose.
#[test]
fn the_same_stored_values_always_yield_the_same_derived_ones() {
    let input = || vec![stored("whole", 7)];
    let call = || resolve(Some(&Halver), input(), &DeclaredValues::new(input()));
    let first = call();
    for _ in 0..64 {
        assert_eq!(call(), first);
    }
}

#[test]
fn a_rule_whose_input_is_missing_omits_its_output_rather_than_zeroing_it() {
    let values = vec![stored("unrelated", 3)];
    let out = resolve(Some(&Halver), values.clone(), &DeclaredValues::new(values));
    assert!(
        !out.iter().any(|v| v.id == "half"),
        "a zero is a statement; an unfilled sheet is the absence of one"
    );
}

#[test]
fn a_whole_numbered_float_is_an_integer_and_a_fractional_one_is_not() {
    assert_eq!(DeclaredValueKind::Number(14.0).as_integer(), Some(14));
    assert_eq!(DeclaredValueKind::Number(14.5).as_integer(), None);
    assert_eq!(DeclaredValueKind::Text("14".into()).as_integer(), None);
}

/// The case the two arguments exist for: Genie's Wish Points rule reads a
/// `level` that lives in the trait slot and is not one of the three
/// attributes Genie puts on a sheet.
#[test]
fn a_rule_can_read_context_a_sheet_does_not_show() {
    let visible = vec![stored("whole", 10)];
    let context = DeclaredValues::new([stored("whole", 10), stored("hidden", 4)]);

    struct ReadsHidden;
    impl SystemRules for ReadsHidden {
        fn id(&self) -> &str {
            "reads-hidden"
        }
        fn derived_declarations(&self) -> Vec<AttributeDeclaration> {
            vec![declaration("doubled", 0)]
        }
        fn derive(&self, stored: &DeclaredValues) -> Vec<DeclaredValue> {
            match stored.integer("hidden") {
                Some(hidden) => vec![derived("doubled", hidden * 2)],
                None => Vec::new(),
            }
        }
    }

    let out = resolve(Some(&ReadsHidden), visible, &context);

    assert!(
        !out.iter().any(|v| v.id == "hidden"),
        "context is legible to a rule, not automatically shown on a sheet"
    );
    let doubled = out.iter().find(|v| v.id == "doubled").expect("derived");
    assert_eq!(doubled.value.as_integer(), Some(8));
}
