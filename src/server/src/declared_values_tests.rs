use super::*;

fn packs() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packs/systems")
        .to_string_lossy()
        .into_owned()
}

fn genie_actor(level: Option<i64>) -> ActorSlots {
    let mut trait_data = serde_json::json!({ "active_conditions": ["bound"] });
    if let Some(level) = level {
        trait_data["level"] = serde_json::json!(level);
    }
    ActorSlots {
        ability_data: Some(serde_json::json!({
            "might": 3, "cunning": 2, "spirit": 4
        })),
        resource_data: Some(serde_json::json!({
            "current_health": 8, "max_health": 10,
            "current_wish_points": 1, "max_wish_points": 5
        })),
        proficiency_data: None,
        trait_data: Some(trait_data),
    }
}

fn find<'a>(values: &'a [DeclaredValue], id: &str) -> Option<&'a DeclaredValue> {
    values.iter().find(|v| v.id == id)
}

/// The end of the wire: a manifest declares, a pack computes, and one set
/// comes out with each value saying which half it came from.
#[test]
fn a_genie_actor_reports_stored_and_derived_values_through_one_path() {
    let values = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));

    let might = find(&values, "might").expect("a declared attribute");
    assert_eq!(might.value.as_integer(), Some(3));
    assert_eq!(might.origin, Origin::Stored);

    let wish = find(&values, "wishPointsForLevel").expect("genie derives this");
    assert_eq!(
        wish.value.as_integer(),
        Some(5),
        "the manifest's ladder gives a level-4 Genie five Wish Points"
    );
    assert_eq!(wish.origin, Origin::Derived);
}

/// The rule's input lives in the trait slot, which is not where a sheet's
/// attributes come from. This is the case `resolve`'s two arguments exist for.
#[test]
fn a_rule_reads_a_slot_the_attribute_list_never_touches() {
    let with_level = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));
    let without = declared_values_for_actor(&packs(), "genie", &genie_actor(None));

    assert!(find(&with_level, "wishPointsForLevel").is_some());
    assert!(
        find(&without, "wishPointsForLevel").is_none(),
        "no level recorded, nothing to look up — omitted rather than defaulted"
    );
    assert!(
        find(&without, "might").is_some(),
        "and the stored half is unaffected"
    );
}

/// A raw stored field is legible to a rule and is not thereby on the sheet.
#[test]
fn stored_fields_a_system_never_declared_stay_out_of_the_answer() {
    let values = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));

    for hidden in ["level", "active_conditions", "current_health"] {
        assert!(
            find(&values, hidden).is_none(),
            "{hidden} is readable by a rule, not published as an attribute"
        );
    }
}

/// FR-019, on the values side: a system this build does not have costs the
/// stored half nothing.
#[test]
fn an_unknown_system_still_reports_nothing_rather_than_failing() {
    let values = declared_values_for_actor(&packs(), "no_such_system", &genie_actor(Some(4)));
    assert!(
        values.is_empty(),
        "no declarations, so nothing to publish — and no panic"
    );
}

/// A system that computes nothing is not a broken one.
///
/// Fate Core, which declares eighteen skills, **zero** abilities and no rules
/// implementation. Originally written against 5e, which derived nothing at the
/// time; 5e now derives most of its sheet, so the assertion moved to a system
/// where "nothing derived" is a fact about the ruleset rather than a fact
/// about how far the work had got.
#[test]
fn a_system_with_no_rules_reports_its_stored_values_unchanged() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "athletics": 3 })),
        resource_data: Some(serde_json::json!({ "fate_points": 3, "refresh": 3 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "fate_core", &slots);

    assert!(
        values.iter().all(|v| v.origin == Origin::Stored),
        "Fate Core computes nothing, which is a property of the ruleset"
    );
}

// ---------------------------------------------------------------------------
// T019a: a pool arrives as two numbers, not as text to be parsed
// ---------------------------------------------------------------------------

/// The regression this exists to prevent: a bar recovered by parsing `"4 / 7"`
/// back apart. That is branching on what a value means, and a system writing
/// `"4 of 7"` would have lost its bar with nothing failing anywhere.
#[test]
fn a_genie_actors_pools_arrive_with_their_maximums_intact() {
    let values = declared_values_for_actor(&packs(), "genie", &genie_actor(Some(4)));

    let health = find(&values, "health").expect("genie declares a health pool");
    assert_eq!(
        health.value,
        DeclaredValueKind::Fraction {
            current: 8,
            max: Some(10)
        },
        "both halves together, as numbers"
    );
    assert_eq!(
        health.origin,
        Origin::Stored,
        "a pool is typed in, not derived"
    );

    let wish = find(&values, "wishPoints").expect("genie declares Wish Points");
    assert_eq!(
        wish.value,
        DeclaredValueKind::Fraction {
            current: 1,
            max: Some(5)
        }
    );
}

/// A system that declares no pools gets none — which is correct for a ruleset
/// that tracks none, not a gap to fill with a default.
///
/// Written against Fate Core, which declared no resources at the time. It does
/// now — T075 gave it the fate points and refresh its `resource_data` had
/// always stored — so the assertion moved to `year_zero_engine`, which still
/// declares none. It kept passing after Fate changed, because the fixture
/// happened to supply no resource data either: a test can go on being green
/// while its message becomes false, and the message was the thing that was
/// wrong.
#[test]
fn a_system_with_no_declared_resources_publishes_none() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "strength": 3, "agility": 2 })),
        resource_data: Some(serde_json::json!({ "anything": 4, "max_anything": 8 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "year_zero_engine", &slots);
    assert!(
        values
            .iter()
            .all(|v| !matches!(v.value, DeclaredValueKind::Fraction { .. })),
        "Year Zero declares no resources, so stored numbers that look like          pools are not published as pools"
    );
}

/// The converse, and the one the previous test stopped covering: a system that
/// *does* declare pools publishes them.
#[test]
fn a_system_that_declares_pools_publishes_them() {
    let slots = ActorSlots {
        resource_data: Some(serde_json::json!({ "fate_points": 3, "refresh": 3 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "fate_core", &slots);
    assert!(
        find(&values, "fatePoints").is_some(),
        "Fate stored these all along and declared them nowhere until T075"
    );
}

/// An actor whose sheet has no resource slot is not an actor with empty pools.
#[test]
fn an_actor_with_no_resource_data_publishes_no_pools() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "might": 3 })),
        ..ActorSlots::default()
    };
    let values = declared_values_for_actor(&packs(), "genie", &slots);
    assert!(find(&values, "health").is_none());
    assert!(
        find(&values, "might").is_some(),
        "and the attributes are unaffected"
    );
}

/// 5e derives, now — the second system to do so, and the one whose derived
/// half is most of its sheet.
#[test]
fn a_5e_actor_derives_its_modifiers_saves_and_skills() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({
            "strength": 10, "dexterity": 16, "constitution": 14,
            "intelligence": 8, "wisdom": 12, "charisma": 7
        })),
        resource_data: Some(serde_json::json!({ "current_hp": 22, "max_hp": 38 })),
        proficiency_data: Some(serde_json::json!({
            "skill_proficiencies": ["stealth", "perception"],
            "saving_throw_proficiencies": ["dexterity"]
        })),
        trait_data: Some(serde_json::json!({ "level": 5 })),
    };
    let values = declared_values_for_actor(&packs(), "dnd5e", &slots);

    let modifier = find(&values, "dexterityMod").expect("modifiers are derived");
    assert_eq!(modifier.value.as_integer(), Some(3));
    assert_eq!(modifier.origin, Origin::Derived);

    // The charisma 7 case: floor, not truncation. -2, never -1.
    assert_eq!(
        find(&values, "charismaMod").and_then(|v| v.value.as_integer()),
        Some(-2)
    );

    assert_eq!(
        find(&values, "passivePerception").and_then(|v| v.value.as_integer()),
        Some(14)
    );

    // And the stored half is untouched, pools included.
    let hp = find(&values, "hitPoints").expect("5e declares a hit point pool");
    assert_eq!(
        hp.value,
        DeclaredValueKind::Fraction {
            current: 22,
            max: Some(38)
        }
    );
    assert_eq!(
        find(&values, "strength").map(|v| v.origin),
        Some(Origin::Stored)
    );
}

// ---------------------------------------------------------------------------
// Increment E: every shipping system has a sheet (SC-012)
// ---------------------------------------------------------------------------

/// Fate Core, which declares no abilities at all and whose sheet is mostly
/// text. Before this increment it published two numbers and nothing else.
#[test]
fn a_fate_actor_publishes_its_aspects_its_stress_and_its_named_skills() {
    let slots = ActorSlots {
        ability_data: None,
        resource_data: Some(serde_json::json!({
            "fate_points": 3, "refresh": 3, "stress": 2
        })),
        proficiency_data: Some(serde_json::json!({
            "skills": [{ "name": "Burglary", "value": 3 }, { "name": "Notice", "value": 2 }]
        })),
        trait_data: Some(serde_json::json!({
            "high_concept": "Disgraced Knight of the Ninth Gate",
            "trouble": "Sworn to a debt I cannot name",
            "consequence_mild": "Bruised Ribs",
            "stunts": ["Riposte", "Reputation Precedes Me"]
        })),
    };
    let values = declared_values_for_actor(&packs(), "fate_core", &slots);

    assert_eq!(
        find(&values, "highConcept").map(|v| v.value.clone()),
        Some(DeclaredValueKind::Text(
            "Disgraced Knight of the Ninth Gate".to_string()
        ))
    );

    // A flat run of eight, two ticked — not a bar.
    assert_eq!(
        find(&values, "stress").map(|v| v.value.clone()),
        Some(DeclaredValueKind::Track { filled: 2, of: 8 })
    );

    // Player-named slots take the player's words.
    assert_eq!(
        find(&values, "skill1").map(|v| v.label.clone()),
        Some("Burglary".to_string())
    );

    // And the pools it always stored and never declared.
    assert!(find(&values, "fatePoints").is_some());

    assert!(
        values.len() > 6,
        "Fate rendered two numbers before this increment; it now publishes {}",
        values.len()
    );
}

/// Cypher, whose stat is a triple and whose damage track has no marks.
#[test]
fn a_cypher_actor_publishes_its_pools_its_edges_and_its_damage_ladder() {
    let slots = ActorSlots {
        ability_data: Some(serde_json::json!({ "might": 10, "speed": 9, "intellect": 12 })),
        resource_data: Some(serde_json::json!({
            "might": 7, "might_pool": 10, "might_edge": 1,
            "speed": 9, "speed_pool": 9, "speed_edge": 0,
            "intellect": 12, "intellect_pool": 12, "intellect_edge": 2,
            "effort": 1, "xp": 4
        })),
        proficiency_data: Some(serde_json::json!({
            "skills": [{ "name": "Stealth", "value": 1 }]
        })),
        trait_data: Some(serde_json::json!({
            "type": "Explorer", "descriptor": "Clever", "focus": "Bears a Halo of Fire",
            "tier": 2, "damage_track": "impaired",
            "cyphers": ["Detonation (level 4)"]
        })),
    };
    let values = declared_values_for_actor(&packs(), "cypher_system", &slots);

    // A pool with its maximum, as numbers.
    assert_eq!(
        find(&values, "mightPool").map(|v| v.value.clone()),
        Some(DeclaredValueKind::Fraction {
            current: 7,
            max: Some(10)
        })
    );

    // The edge belongs to the same thing — FR-033, so a sheet can show them
    // together rather than as unrelated rows.
    assert_eq!(
        find(&values, "mightEdge").and_then(|v| v.group.clone()),
        Some("mightPool".to_string())
    );

    // A ladder, with its rungs, and a character partway down it.
    match find(&values, "damageTrack").map(|v| v.value.clone()) {
        Some(DeclaredValueKind::State { current, options }) => {
            assert_eq!(current.as_deref(), Some("impaired"));
            assert_eq!(options.len(), 3, "the whole ladder travels");
        }
        other => panic!("expected a state ladder, got {other:?}"),
    }

    assert_eq!(
        find(&values, "focus").map(|v| v.value.clone()),
        Some(DeclaredValueKind::Text("Bears a Halo of Fire".to_string()))
    );
}

/// SC-012, read from the directory rather than from a list here, so a pack
/// added later is covered without anyone remembering to add it.
#[test]
fn every_bundled_system_publishes_something() {
    let dir = std::fs::read_dir(packs()).expect("the systems directory");
    let mut checked = 0;

    for entry in dir.filter_map(Result::ok) {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();

        // A generic actor: whatever the system reads, it reads from here.
        let slots = ActorSlots {
            ability_data: Some(serde_json::json!({
                "strength": 12, "dexterity": 12, "constitution": 12,
                "intelligence": 12, "wisdom": 12, "charisma": 12,
                "might": 10, "speed": 10, "intellect": 10,
                "insight": 2, "prowess": 2, "resolve": 2,
                "cunning": 2, "spirit": 2, "agility": 2, "wits": 2, "empathy": 2,
                "willpower": 2
            })),
            resource_data: Some(serde_json::json!({
                "current_health": 5, "max_health": 10,
                "current_wish_points": 1, "max_wish_points": 3,
                "current_hp": 9, "max_hp": 15,
                "might": 8, "might_pool": 10,
                "fate_points": 3, "refresh": 3, "stress": 1
            })),
            proficiency_data: Some(
                serde_json::json!({ "skills": [{"name":"Something","value":1}] }),
            ),
            trait_data: Some(serde_json::json!({ "level": 3, "tier": 2 })),
        };

        let values = declared_values_for_actor(&packs(), &id, &slots);
        assert!(
            !values.is_empty(),
            "{id} publishes nothing at all — a world bound to it would show an \
             empty sheet (SC-012)"
        );
        checked += 1;
    }

    assert!(
        checked >= 7,
        "expected the bundled systems, checked {checked}"
    );
}
