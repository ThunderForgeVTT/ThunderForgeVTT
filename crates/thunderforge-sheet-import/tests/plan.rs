//! The mapping engine's invariants (contracts/sheet-import-core.md), each
//! one a test, on hand-built readings with no PDF.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use thunderforge_pdf::Rect;
use thunderforge_sheet_import::*;

// ---------------------------------------------------------------------------
// Fixtures

fn manifest() -> Value {
    json!({
        "data_types": {
            "ability_data": { "properties": {
                "strength": {}, "dexterity": {}, "armor_class": {}
            }},
            "resource_data": { "properties": {
                "max_hp": {}, "current_hp": {}, "coins": {}
            }},
            "trait_data": { "properties": {
                "classes": {}, "race": {}, "notes": {}, "inspiration": {}, "size": {}
            }}
        },
        "abilityVocabulary": { "types": [ { "id": "spell" }, { "id": "feature" } ] },
        "sheetImport": {
            "readers": ["test-reader"],
            "fields": {
                "identity.name": "actor.label",
                "identity.species": "trait_data.race",
                "identity.size": "trait_data.size",
                "abilities.str": "ability_data.strength",
                "abilities.dex": "ability_data.dexterity",
                "classes": "trait_data.classes",
                "defences.armour_class": "ability_data.armor_class",
                "resources.hp_max": "resource_data.max_hp",
                "resources.hp_current": "resource_data.current_hp",
                "resources.coins": "resource_data.coins",
                "resources.inspiration": "trait_data.inspiration"
            },
            "content": {
                "spell": { "ability": "spell" },
                "feature": { "ability": "feature" },
                "item": { "item": true },
                "attack": { "refine": true }
            },
            "derived": ["skill.*", "initiative"],
            "playState": ["resource_data.current_hp", "trait_data.inspiration"],
            "ignore": ["identity.player_name"],
            "notes": "trait_data.notes"
        }
    })
}

fn mapping() -> SheetMapping {
    SheetMapping::from_manifest(&manifest())
        .expect("the fixture declaration is valid")
        .expect("the fixture declares sheetImport")
}

fn src(page: u32, text: &str) -> Option<Source> {
    Some(Source {
        page,
        rect: Rect {
            x0: 10.0,
            y0: 20.0,
            x1: 30.0,
            y1: 40.0,
        },
        text: text.to_string(),
    })
}

fn class(name: &str, level: i32, die: i32) -> ClassLevel {
    ClassLevel {
        name: Field::read(name.to_string(), src(1, name)),
        subclass: Field::unread(),
        level: Field::read(level, src(1, &level.to_string())),
        hit_die: Field::read(die, None),
    }
}

fn spell(name: &str, granted_by: &str) -> NamedContent {
    NamedContent {
        kind: "spell".into(),
        name: Field::read(name.to_string(), src(3, name)),
        fields: BTreeMap::from([("range".to_string(), json!("120 feet"))]),
        link: ContentLink {
            prepared: Some(true),
            granted_by: vec![granted_by.to_string()],
            ..ContentLink::default()
        },
    }
}

/// A fighter 3 / wizard 2 with a little of everything.
fn reading() -> ImportedCharacter {
    let mut r = ImportedCharacter {
        reader: ReaderStamp {
            id: "test-reader".into(),
            version: "1".into(),
        },
        ..ImportedCharacter::default()
    };
    r.identity.name = Field::read("Brannoc Vell".into(), src(1, "Brannoc Vell"));
    r.identity.player_name = Field::read("Someone".into(), src(1, "Someone"));
    r.identity.species = Field::read("Human".into(), src(1, "Human"));
    r.identity.size = Field::unread();
    r.abilities
        .insert("str".into(), Field::read(16, src(1, "16")));
    r.abilities
        .insert("dex".into(), Field::read(14, src(1, "14")));
    r.classes = vec![class("Fighter", 3, 10), class("Wizard", 2, 6)];
    r.defences.armour_class = Field::read(16, src(1, "16"));
    r.resources.hp_max = Field::read(38, src(1, "38"));
    r.resources.hp_current = Field::read(30, src(1, "30"));
    r.resources
        .coins
        .insert("gp".into(), Field::read(42, src(2, "42")));
    r.resources
        .coins
        .insert("sp".into(), Field::read(7, src(2, "7")));
    r.resources.inspiration = Field::read(false, None);
    // Read, but this declaration has nowhere for it.
    r.defences.resistances = Field::read(vec!["Poison".into()], src(1, "Poison"));
    r.derived
        .insert("skill.athletics".into(), Field::read(6, src(1, "+6")));
    r.derived
        .insert("initiative".into(), Field::read(2, src(1, "+2")));
    r.content = vec![
        spell("Magic Missile", "Wizard"),
        spell("Shield (PHB)", "Wizard"),
        NamedContent {
            kind: "feature".into(),
            name: Field::read("Second Wind".into(), src(2, "Second Wind")),
            link: ContentLink {
                uses: Some(1),
                recharge: Some("short_rest".into()),
                ..ContentLink::default()
            },
            ..NamedContent::default()
        },
        NamedContent {
            kind: "attack".into(),
            name: Field::read("Longsword".into(), src(1, "Longsword")),
            fields: BTreeMap::from([("damage".to_string(), json!("1d8+3"))]),
            ..NamedContent::default()
        },
    ];
    r.notes = vec![Note {
        label: "Immune to disease".into(),
        text: Field::read("You are immune to disease.".into(), src(2, "Immune")),
    }];
    r
}

#[derive(Default)]
struct Index(BTreeMap<(String, String), Indexed>);

impl ContentIndex for Index {
    fn lookup(&self, kind: &str, normalised: &str) -> Option<Indexed> {
        self.0
            .get(&(kind.to_string(), normalised.to_string()))
            .cloned()
    }
}

fn first_import() -> ActorSnapshot {
    ActorSnapshot::default()
}

fn run(
    reading: &ImportedCharacter,
    corrections: &Corrections,
    current: &ActorSnapshot,
) -> ImportPlan {
    plan(
        &mapping(),
        None,
        reading,
        corrections,
        current,
        &Index::default(),
    )
}

fn field<'a>(plan: &'a ImportPlan, path: &str) -> Option<&'a FieldChange> {
    plan.fields.iter().find(|f| f.path == path)
}

// ---------------------------------------------------------------------------
// Determinism

#[test]
fn the_plan_is_the_same_byte_for_byte() {
    let a = run(&reading(), &Corrections::new(), &first_import());
    let b = run(&reading(), &Corrections::new(), &first_import());
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    assert_eq!(plan_hash(&a), plan_hash(&b));
}

#[test]
fn the_plan_does_not_depend_on_the_order_content_was_read() {
    let mut shuffled = reading();
    shuffled.content.reverse();
    let a = run(&reading(), &Corrections::new(), &first_import());
    let b = run(&shuffled, &Corrections::new(), &first_import());
    let names = |p: &ImportPlan| p.content.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&a), names(&b));
}

#[test]
fn the_hash_is_stable_and_pinned() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    let hash = plan_hash(&plan);
    assert_eq!(hash.len(), 64);
    assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    // Any reviewed difference changes it.
    let mut corrected = Corrections::new();
    corrected.insert("abilities.str".into(), json!(17));
    let other = run(&reading(), &corrected, &first_import());
    assert_ne!(hash, plan_hash(&other));
}

// ---------------------------------------------------------------------------
// Unread and uncertain

#[test]
fn an_unread_field_is_never_written() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    let size = field(&plan, "identity.size").expect("shown in the review");
    assert_eq!(size.certainty, PlanCertainty::Unread);
    assert_eq!(size.new, None);
    let overwrite = BTreeSet::new();
    assert!(plan.writes(&overwrite).all(|f| f.path != "identity.size"));
}

#[test]
fn an_unread_field_keeps_the_actors_value() {
    let current = ActorSnapshot {
        values: BTreeMap::from([("trait_data.size".to_string(), json!("Medium"))]),
        ..ActorSnapshot::default()
    };
    let plan = run(&reading(), &Corrections::new(), &current);
    let size = field(&plan, "identity.size").unwrap();
    assert_eq!(size.old, Some(json!("Medium")));
    assert!(
        plan.writes(&BTreeSet::new())
            .all(|f| f.path != "identity.size")
    );
}

#[test]
fn an_unread_field_the_person_fills_in_is_written_as_corrected() {
    let mut corrections = Corrections::new();
    corrections.insert("identity.size".into(), json!("Medium"));
    let plan = run(&reading(), &corrections, &first_import());
    let size = field(&plan, "identity.size").unwrap();
    assert_eq!(size.certainty, PlanCertainty::Corrected);
    assert_eq!(size.new, Some(json!("Medium")));
}

#[test]
fn an_uncertain_field_is_written_only_as_shown_and_marked() {
    let mut r = reading();
    r.abilities.insert(
        "dex".into(),
        Field::uncertain(Some(14), "the 4 is smudged", src(1, "1?")),
    );
    let plan = run(&r, &Corrections::new(), &first_import());
    let dex = field(&plan, "abilities.dex").unwrap();
    assert_eq!(dex.certainty, PlanCertainty::Uncertain);
    assert_eq!(dex.reason.as_deref(), Some("the 4 is smudged"));
    assert_eq!(
        dex.new,
        Some(json!(14)),
        "left as read, it is written as read"
    );
    assert!(dex.source.is_some(), "the review can point at it");
}

#[test]
fn an_uncertain_field_corrected_is_written_as_corrected() {
    let mut r = reading();
    r.abilities.insert(
        "dex".into(),
        Field::uncertain(Some(14), "smudged", src(1, "1?")),
    );
    let mut corrections = Corrections::new();
    corrections.insert("abilities.dex".into(), json!(11));
    let plan = run(&r, &corrections, &first_import());
    let dex = field(&plan, "abilities.dex").unwrap();
    assert_eq!(dex.certainty, PlanCertainty::Corrected);
    assert_eq!(dex.new, Some(json!(11)));
    assert_eq!(dex.reason, None);
}

#[test]
fn an_uncertain_part_makes_the_whole_value_uncertain() {
    let mut r = reading();
    r.classes[1].level = Field::uncertain(Some(2), "no number after Wizard", src(1, "Wizard"));
    let plan = run(&r, &Corrections::new(), &first_import());
    let classes = field(&plan, "classes").unwrap();
    assert_eq!(classes.certainty, PlanCertainty::Uncertain);
    assert!(classes.reason.as_deref().unwrap().contains("1.level"));
}

// ---------------------------------------------------------------------------
// Derived values

#[test]
fn a_derived_value_is_never_written() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    assert!(plan.fields.iter().all(|f| !f.path.starts_with("derived.")));
    assert_eq!(
        plan.checked,
        vec![
            "derived.initiative".to_string(),
            "derived.skill.athletics".to_string()
        ]
    );
}

fn derive_initiative(reading: &ImportedCharacter, _: &ActorSnapshot, plan: &mut ImportPlan) {
    let dex = reading.abilities["dex"].value.unwrap();
    let derived = (dex - 10).div_euclid(2);
    let sheet = reading.derived["initiative"].value.unwrap();
    plan.cross_check(
        "derived.initiative",
        json!(sheet),
        json!(derived),
        &["abilities.dex"],
    );
}

#[test]
fn a_derived_value_that_agrees_is_no_cross_check() {
    let plan = plan(
        &mapping(),
        Some(derive_initiative),
        &reading(),
        &Corrections::new(),
        &first_import(),
        &Index::default(),
    );
    assert!(plan.cross_checks.is_empty());
    assert_eq!(
        field(&plan, "abilities.dex").unwrap().certainty,
        PlanCertainty::Read
    );
}

#[test]
fn a_derived_value_that_disagrees_is_a_cross_check_and_the_base_is_uncertain() {
    let mut r = reading();
    r.derived
        .insert("initiative".into(), Field::read(5, src(1, "+5")));
    let plan = plan(
        &mapping(),
        Some(derive_initiative),
        &r,
        &Corrections::new(),
        &first_import(),
        &Index::default(),
    );
    assert_eq!(plan.cross_checks.len(), 1);
    assert_eq!(plan.cross_checks[0].sheet, json!(5));
    assert_eq!(plan.cross_checks[0].derived, json!(2));
    let dex = field(&plan, "abilities.dex").unwrap();
    assert_eq!(dex.certainty, PlanCertainty::Uncertain);
    let reason = dex.reason.as_deref().unwrap();
    assert!(
        reason.contains('5') && reason.contains('2'),
        "both numbers: {reason}"
    );
}

// ---------------------------------------------------------------------------
// Nothing is dropped

fn accounted(plan: &ImportPlan, path: &str) -> usize {
    let covers = |declared: &str| path == declared || path.starts_with(&format!("{declared}."));
    plan.fields.iter().filter(|f| covers(&f.path)).count()
        + plan
            .content
            .iter()
            .filter(|c| c.from.iter().any(|p| p == path))
            .count()
        + plan.unmapped.iter().filter(|u| u.path == path).count()
        + plan.identical.iter().filter(|p| covers(p)).count()
        + plan.checked.iter().filter(|p| *p == path).count()
        + plan.ignored.iter().filter(|p| *p == path).count()
}

#[test]
fn every_leaf_with_a_value_ends_in_exactly_one_place() {
    let r = reading();
    let current = ActorSnapshot {
        // Identical: nothing to do, but accounted for.
        values: BTreeMap::from([("ability_data.armor_class".to_string(), json!(16))]),
        ..ActorSnapshot::default()
    };
    let plan = run(&r, &Corrections::new(), &current);
    for leaf in r.leaves() {
        if leaf.value.is_none() {
            continue;
        }
        assert_eq!(
            accounted(&plan, &leaf.path),
            1,
            "{} lands exactly once",
            leaf.path
        );
    }
    assert_eq!(plan.identical, vec!["defences.armour_class".to_string()]);
}

#[test]
fn what_has_no_home_goes_to_the_notes_labelled() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    let paths: Vec<&str> = plan.unmapped.iter().map(|u| u.path.as_str()).collect();
    assert!(paths.contains(&"defences.resistances"));
    assert!(paths.contains(&"notes.0"));
    let note = plan.unmapped.iter().find(|u| u.path == "notes.0").unwrap();
    assert_eq!(note.label, "Immune to disease");
    assert!(
        plan.unmapped
            .iter()
            .all(|u| u.goes_to == "trait_data.notes")
    );
}

#[test]
fn a_refine_kind_left_undecided_is_kept_in_the_notes() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    assert!(plan.content.iter().all(|c| c.kind != "attack"));
    let attack = plan
        .unmapped
        .iter()
        .find(|u| u.path == "content.3")
        .unwrap();
    assert_eq!(attack.label, "Longsword");
}

#[test]
fn a_player_name_is_read_and_ignored() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    assert_eq!(plan.ignored, vec!["identity.player_name".to_string()]);
    assert!(plan.fields.iter().all(|f| f.path != "identity.player_name"));
}

// ---------------------------------------------------------------------------
// Composite values

#[test]
fn a_list_lands_whole_and_a_purse_keeps_unread_coins() {
    let current = ActorSnapshot {
        values: BTreeMap::from([(
            "resource_data.coins".to_string(),
            json!({ "gp": 1, "pp": 3 }),
        )]),
        ..ActorSnapshot::default()
    };
    let plan = run(&reading(), &Corrections::new(), &current);
    let classes = field(&plan, "classes").unwrap();
    assert_eq!(
        classes.new,
        Some(json!([
            { "name": "Fighter", "level": 3, "hit_die": 10 },
            { "name": "Wizard", "level": 2, "hit_die": 6 }
        ]))
    );
    let coins = field(&plan, "resources.coins").unwrap();
    assert_eq!(coins.new, Some(json!({ "gp": 42, "sp": 7, "pp": 3 })));
}

// ---------------------------------------------------------------------------
// Content

#[test]
fn content_resolves_against_the_world_and_what_is_staged() {
    let fields = json!({ "range": "120 feet" });
    let index = Index(BTreeMap::from([
        (
            ("spell".to_string(), "magic missile".to_string()),
            Indexed::World { id: "w1".into() },
        ),
        (
            ("spell".to_string(), "shield".to_string()),
            Indexed::Staged {
                id: "s1".into(),
                content_hash: content_hash("spell", &fields),
            },
        ),
        (
            ("feature".to_string(), "second wind".to_string()),
            Indexed::Staged {
                id: "s2".into(),
                content_hash: "something else".into(),
            },
        ),
    ]));
    let plan = plan(
        &mapping(),
        None,
        &reading(),
        &Corrections::new(),
        &first_import(),
        &index,
    );
    let by_name = |name: &str| plan.content.iter().find(|c| c.name == name).unwrap();
    assert_eq!(
        by_name("Magic Missile").resolution,
        Resolution::World { id: "w1".into() }
    );
    assert_eq!(
        by_name("Shield (PHB)").resolution,
        Resolution::StagedExisting { id: "s1".into() }
    );
    assert_eq!(
        by_name("Second Wind").resolution,
        Resolution::Differs { id: "s2".into() }
    );
    assert_eq!(by_name("Second Wind").link.uses, Some(1));
}

#[test]
fn content_new_to_the_world_is_staged() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    assert!(
        plan.content
            .iter()
            .all(|c| c.resolution == Resolution::StagedNew)
    );
}

#[test]
fn the_same_spell_from_two_classes_is_one_change_granted_by_both() {
    let mut r = reading();
    r.content.push(spell("Magic Missile", "Sorcerer"));
    let plan = run(&r, &Corrections::new(), &first_import());
    let missiles: Vec<_> = plan
        .content
        .iter()
        .filter(|c| c.normalised == "magic missile")
        .collect();
    assert_eq!(missiles.len(), 1);
    assert_eq!(missiles[0].link.granted_by, vec!["Sorcerer", "Wizard"]);
    assert_eq!(missiles[0].from, vec!["content.0", "content.4"]);
}

#[test]
fn a_content_name_can_be_corrected() {
    let mut corrections = Corrections::new();
    corrections.insert("content.0.name".into(), json!("Magic Missle"));
    let plan = run(&reading(), &corrections, &first_import());
    let renamed = plan
        .content
        .iter()
        .find(|c| c.from == ["content.0"])
        .unwrap();
    assert_eq!(renamed.name, "Magic Missle");
    assert_eq!(renamed.certainty, PlanCertainty::Corrected);
}

// ---------------------------------------------------------------------------
// Re-import

fn reimport() -> ActorSnapshot {
    ActorSnapshot {
        is_reimport: true,
        values: BTreeMap::from([
            ("resource_data.current_hp".to_string(), json!(12)),
            ("trait_data.inspiration".to_string(), json!(true)),
            ("resource_data.max_hp".to_string(), json!(31)),
        ]),
        links: vec![CurrentLink {
            kind: "spell".into(),
            name: "Burning Hands".into(),
            id: "w9".into(),
            staged: false,
        }],
    }
}

#[test]
fn play_state_is_kept_on_a_reimport() {
    let plan = run(&reading(), &Corrections::new(), &reimport());
    let hp = field(&plan, "resources.hp_current").unwrap();
    assert!(hp.play_state);
    let none = BTreeSet::new();
    let written: Vec<&str> = plan.writes(&none).map(|f| f.target.as_str()).collect();
    assert!(!written.contains(&"resource_data.current_hp"));
    assert!(!written.contains(&"trait_data.inspiration"));
    assert!(written.contains(&"resource_data.max_hp"), "not play state");
    let kept: Vec<&str> = plan
        .kept_in_play
        .iter()
        .map(|k| k.target.as_str())
        .collect();
    assert_eq!(
        kept,
        vec!["resource_data.current_hp", "trait_data.inspiration"]
    );
}

#[test]
fn play_state_named_by_the_person_is_overwritten() {
    let plan = run(&reading(), &Corrections::new(), &reimport());
    let overwrite = BTreeSet::from(["resource_data.current_hp".to_string()]);
    let written: Vec<&str> = plan.writes(&overwrite).map(|f| f.target.as_str()).collect();
    assert!(written.contains(&"resource_data.current_hp"));
    assert!(!written.contains(&"trait_data.inspiration"));
}

#[test]
fn play_state_is_written_on_a_first_import() {
    let plan = run(&reading(), &Corrections::new(), &first_import());
    let written: Vec<&str> = plan
        .writes(&BTreeSet::new())
        .map(|f| f.target.as_str())
        .collect();
    assert!(written.contains(&"resource_data.current_hp"));
    assert!(plan.kept_in_play.is_empty());
}

#[test]
fn content_no_longer_on_the_sheet_is_marked_removed() {
    let plan = run(&reading(), &Corrections::new(), &reimport());
    let removed: Vec<_> = plan.content.iter().filter(|c| c.removed).collect();
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0].name, "Burning Hands");
    assert_eq!(removed[0].resolution, Resolution::World { id: "w9".into() });
}

// ---------------------------------------------------------------------------
// Names and hashes

#[test]
fn names_are_normalised_for_matching() {
    assert_eq!(normalise_name("Hunter's Mark (PHB)"), "hunters mark");
    assert_eq!(normalise_name("  hunters   MARK "), "hunters mark");
    assert_eq!(normalise_name("Half-Orc"), "half orc");
    assert_eq!(normalise_name("Hunter\u{2019}s Mark"), "hunters mark");
    assert_eq!(normalise_name("Shield [TCE]"), "shield");
}

#[test]
fn content_hash_ignores_key_order_and_sees_values() {
    let a = json!({ "range": "60 feet", "school": "evocation" });
    let mut b = serde_json::Map::new();
    b.insert("school".into(), json!("evocation"));
    b.insert("range".into(), json!("60 feet"));
    assert_eq!(
        content_hash("spell", &a),
        content_hash("spell", &Value::Object(b))
    );
    assert_ne!(
        content_hash("spell", &a),
        content_hash(
            "spell",
            &json!({ "range": "90 feet", "school": "evocation" })
        )
    );
    assert_ne!(content_hash("spell", &a), content_hash("feat", &a));
}

/// The hash is pinned, so a change to the canonical form, or to what a plan
/// holds, is a deliberate one: the browser's review and the server's apply
/// must hash the same plan the same way.
#[test]
fn the_hash_of_a_small_plan_is_pinned() {
    let mut r = ImportedCharacter {
        reader: ReaderStamp {
            id: "test-reader".into(),
            version: "1".into(),
        },
        ..ImportedCharacter::default()
    };
    r.abilities
        .insert("str".into(), Field::read(16, src(1, "16")));
    let plan = run(&r, &Corrections::new(), &first_import());
    assert_eq!(
        plan_hash(&plan),
        "af0c0a959c82af7b7de9c0398f32a1e81838474878db73793d64f419834d5d4c"
    );
}

// ---------------------------------------------------------------------------
// The refine hook

/// A refine that writes a target the declaration cannot express (a level
/// that is a sum), and reshapes one it can (a race in capitals).
fn sums_the_level(reading: &ImportedCharacter, current: &ActorSnapshot, plan: &mut ImportPlan) {
    let level: i32 = reading
        .classes
        .iter()
        .filter_map(|class| class.level.value)
        .sum();
    plan.set_field(
        current,
        FieldChange {
            path: "classes".into(),
            target: "trait_data.level".into(),
            old: None,
            new: Some(json!(level)),
            certainty: PlanCertainty::Read,
            reason: None,
            source: None,
            play_state: false,
        },
    );
    if let Some(mut race) = plan.take_field("trait_data.race") {
        race.new = race
            .new
            .and_then(|v| v.as_str().map(|s| json!(s.to_uppercase())));
        plan.set_field(current, race);
    }
}

fn run_refined(reading: &ImportedCharacter, current: &ActorSnapshot) -> ImportPlan {
    plan(
        &mapping(),
        Some(sums_the_level),
        reading,
        &Corrections::new(),
        current,
        &Index::default(),
    )
}

#[test]
fn a_refined_field_carries_the_actors_current_value() {
    let current = ActorSnapshot {
        values: BTreeMap::from([("trait_data.level".to_string(), json!(2))]),
        ..ActorSnapshot::default()
    };
    let plan = run_refined(&reading(), &current);
    let level = plan
        .fields
        .iter()
        .find(|f| f.target == "trait_data.level")
        .expect("refine added the level");
    assert_eq!(level.old, Some(json!(2)));
    assert!(level.new.is_some());
}

#[test]
fn a_refined_field_equal_to_the_actor_is_identical_not_a_change() {
    let first = run_refined(&reading(), &first_import());
    let race = first
        .fields
        .iter()
        .find(|f| f.target == "trait_data.race")
        .expect("a race")
        .new
        .clone()
        .unwrap();
    let current = ActorSnapshot {
        values: BTreeMap::from([("trait_data.race".to_string(), race)]),
        ..ActorSnapshot::default()
    };
    let again = run_refined(&reading(), &current);
    assert!(again.fields.iter().all(|f| f.target != "trait_data.race"));
    assert!(again.identical.contains(&"identity.species".to_string()));
    assert_eq!(
        again
            .fields
            .iter()
            .filter(|f| f.target == "trait_data.level")
            .count(),
        1,
        "set_field replaces, never duplicates"
    );
}
