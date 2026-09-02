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
        group: None,
        group_label: None,
        headline: false,
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

// ---------------------------------------------------------------------------
// The kinds a whole character sheet needs (FR-031)
// ---------------------------------------------------------------------------

/// A track and a pool look alike and are not.
///
/// A pool is a quantity with a maximum and the numbers are the point; a track
/// is a set of marks and the count is the point. Drawing one as the other
/// gives a player a bar where they expect boxes to tick, which is why they are
/// separate kinds rather than one with a flag.
#[test]
fn a_track_is_not_a_pool() {
    let track = DeclaredValueKind::Track { filled: 3, of: 8 };
    let pool = DeclaredValueKind::Fraction {
        current: 3,
        max: Some(8),
    };

    assert_ne!(track, pool, "the same numbers, and not the same thing");
    assert_eq!(track.as_integer(), Some(3), "a track's number is its marks");
    assert_eq!(pool.as_integer(), Some(3));
}

/// Fate's stress is one flat run of eight; 5e's death saves are two separate
/// runs of three meaning opposite things. Two tracks is what two tracks are,
/// which is why a track carries no notion of rows.
#[test]
fn the_two_shipping_track_shapes_are_both_expressible() {
    let fate_stress = DeclaredValueKind::Track { filled: 2, of: 8 };
    let successes = DeclaredValueKind::Track { filled: 1, of: 3 };
    let failures = DeclaredValueKind::Track { filled: 2, of: 3 };

    assert_eq!(fate_stress.as_integer(), Some(2));
    assert_ne!(
        successes, failures,
        "two runs of three, and the difference between them is which one it is"
    );
}

/// Cypher's damage track has no marks to count. Asking a state set for a
/// number is a category error, and returning its index would invent an
/// arithmetic the system never declared.
#[test]
fn a_state_set_has_no_number() {
    let damage = DeclaredValueKind::State {
        current: Some("impaired".to_string()),
        options: vec![
            "impaired".to_string(),
            "debilitated".to_string(),
            "dead".to_string(),
        ],
    };
    assert_eq!(damage.as_integer(), None);
}

/// An uninjured character is at no position on a damage track. `None` is a
/// real answer, not a missing one.
#[test]
fn a_state_set_with_nothing_current_is_a_character_who_is_fine() {
    let damage = DeclaredValueKind::State {
        current: None,
        options: vec!["impaired".to_string(), "dead".to_string()],
    };
    match damage {
        DeclaredValueKind::State { current, options } => {
            assert!(current.is_none());
            assert_eq!(options.len(), 2, "the ladder still exists");
        }
        _ => unreachable!(),
    }
}

/// The edge case a saved character produces: a stored state the system no
/// longer declares. It must read as unknown rather than as the first option,
/// which would silently heal them.
#[test]
fn a_state_the_system_no_longer_declares_is_not_silently_the_first_one() {
    let damage = DeclaredValueKind::State {
        current: Some("shaken".to_string()),
        options: vec!["impaired".to_string(), "dead".to_string()],
    };
    match &damage {
        DeclaredValueKind::State { current, options } => {
            let current = current.as_deref().expect("something is stored");
            assert!(
                !options.iter().any(|o| o == current),
                "this is the unknown case"
            );
            assert_ne!(
                current, options[0],
                "and it must not be read as the mildest state"
            );
        }
        _ => unreachable!(),
    }
}

/// FR-033: one thing with parts, not three unrelated identifiers.
#[test]
fn grouped_values_carry_the_relationship_a_sheet_shows() {
    let stat = |id: &str, value: i32| DeclaredValue {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        value: DeclaredValueKind::Integer(value),
        group: Some("might".to_string()),
        group_label: None,
        headline: false,
        origin: Origin::Stored,
    };

    // A Cypher stat: a current value, the pool that is its maximum, and the
    // edge that modifies spending from it.
    let values = vec![
        stat("might", 10),
        stat("mightPool", 12),
        stat("mightEdge", 1),
    ];

    let together: Vec<&DeclaredValue> = values
        .iter()
        .filter(|v| v.group.as_deref() == Some("might"))
        .collect();
    assert_eq!(together.len(), 3, "three parts of one thing");

    // And the list is still a list — nothing downstream has to learn to nest.
    assert_eq!(values.len(), 3);
}

#[test]
fn an_ungrouped_value_says_so_rather_than_belonging_to_a_group_of_one() {
    assert_eq!(stored("strength", 14).group, None);
}

/// The system's declaration order survives `resolve` (spec 032).
///
/// It did not. `visible` was funnelled through `DeclaredValues`, a `BTreeMap`
/// keyed by id, which deduplicated correctly and alphabetised everything on
/// the way past. Genie declares might, cunning, spirit and a sheet showed
/// cunning, might, spirit; 5e declares walk, fly, swim, climb and a sheet
/// showed climb first.
///
/// Every layer above this one documents that a set arrives in the system's own
/// order and that a pack never reorders it. The order was gone before any of
/// them saw it, which is why none of their tests could catch this.
#[test]
fn resolve_keeps_the_order_the_system_declared_and_not_the_alphabet() {
    let value = |id: &str| DeclaredValue {
        id: id.to_string(),
        label: id.to_string(),
        abbreviation: None,
        value: DeclaredValueKind::Integer(1),
        group: None,
        group_label: None,
        headline: false,
        origin: Origin::Stored,
    };

    // Deliberately in an order the alphabet would destroy.
    let declared = vec![value("walk"), value("fly"), value("swim"), value("climb")];
    let resolved = resolve(None, declared, &DeclaredValues::default());

    let order: Vec<&str> = resolved.iter().map(|v| v.id.as_str()).collect();
    assert_eq!(order, vec!["walk", "fly", "swim", "climb"]);
}

/// Deduplication is still wanted; it is only the sorting that was not.
#[test]
fn resolve_keeps_the_first_of_two_values_sharing_an_identifier() {
    let value = |id: &str, n: i32| DeclaredValue {
        id: id.to_string(),
        label: format!("{id}-{n}"),
        abbreviation: None,
        value: DeclaredValueKind::Integer(n),
        group: None,
        group_label: None,
        headline: false,
        origin: Origin::Stored,
    };

    let resolved = resolve(
        None,
        vec![value("might", 1), value("cunning", 2), value("might", 3)],
        &DeclaredValues::default(),
    );

    assert_eq!(resolved.len(), 2, "one identifier is one value");
    // The earlier one is the one the system reached for first, matching
    // `indexById` on the other side of the wire.
    assert_eq!(resolved[0].value.as_integer(), Some(1));
    assert_eq!(resolved[0].label, "might-1");
}
