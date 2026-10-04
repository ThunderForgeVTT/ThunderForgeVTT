use super::*;
use crate::rules::{ability_modifier, proficiency_bonus_for_challenge, DnD5eRules};
use crate::validators::{
    declared_size_ids, validate_ability_data, validate_proficiency_data, validate_resource_data,
    validate_trait_data,
};
use thunderforge_canvas_core::system_rules::{
    DeclaredValue, DeclaredValueKind, DeclaredValues, Origin, SystemRules,
};

fn manifest() -> serde_json::Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../system.json");
    let text = std::fs::read_to_string(path).expect("5e's manifest should be readable");
    serde_json::from_str(&text).expect("valid manifest json")
}

fn block(id: &str) -> StatBlock {
    stat_blocks()
        .into_iter()
        .find(|b| b.id == id)
        .unwrap_or_else(|| panic!("no stat block {id}"))
}

/// `3d6-3` to (count, die, modifier).
fn dice(text: &str) -> (i64, i64, i64) {
    let (count, rest) = text.split_once('d').expect("dice");
    let (die, modifier) = match rest.find(['+', '-']) {
        Some(at) => (&rest[..at], rest[at..].parse::<i64>().expect("modifier")),
        None => (rest, 0),
    };
    (count.parse().unwrap(), die.parse().unwrap(), modifier)
}

/// The slots flattened the way the server hands them to a rule.
fn stored(slots: &StatBlockSlots) -> DeclaredValues {
    let mut values = Vec::new();
    for slot in [
        &slots.ability_data,
        &slots.resource_data,
        &slots.proficiency_data,
        &slots.trait_data,
    ] {
        for (field, raw) in slot.as_object().expect("a slot is an object") {
            let value = match raw {
                Value::String(s) => DeclaredValueKind::Text(s.clone()),
                Value::Number(n) => DeclaredValueKind::Integer(n.as_i64().unwrap() as i32),
                Value::Array(items) => DeclaredValueKind::List(
                    items
                        .iter()
                        .filter_map(|i| i.as_str().map(str::to_string))
                        .collect(),
                ),
                _ => continue,
            };
            values.push(DeclaredValue {
                id: field.clone(),
                label: field.clone(),
                abbreviation: None,
                value,
                group: None,
                group_label: None,
                headline: false,
                origin: Origin::Stored,
            });
        }
    }
    DeclaredValues::new(values)
}

fn derived(block: &StatBlock, id: &str) -> Option<i32> {
    DnD5eRules::from_manifest(&manifest())
        .derive(&stored(&block.slots()))
        .iter()
        .find(|v| v.id == id)
        .and_then(|v| v.value.as_integer())
}

#[test]
fn the_file_parses_and_no_id_is_used_twice() {
    let blocks = stat_blocks();
    assert!(blocks.len() >= 16, "the bundled set went missing");
    let mut ids: Vec<&str> = blocks.iter().map(|b| b.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), blocks.len(), "two blocks share an id");

    let mut slugs: Vec<&str> = blocks
        .iter()
        .filter_map(|b| b.bestiary.as_deref())
        .collect();
    slugs.sort_unstable();
    let before = slugs.len();
    slugs.dedup();
    assert_eq!(
        slugs.len(),
        before,
        "two blocks answer to one bestiary creature"
    );
}

/// **The book's own check on a hit point total.**
///
/// A printed block gives the average beside the dice, and the average is
/// arithmetic: count × (die + 1) / 2, rounded down, plus count × Constitution
/// modifier. A block where the two disagree was typed wrong somewhere, in the
/// hit points, the dice, or the Constitution.
#[test]
fn every_hit_point_total_is_the_average_of_its_dice() {
    for b in stat_blocks() {
        let (count, die, modifier) = dice(&b.hit_dice);
        assert_eq!(
            b.hit_points,
            count * (die + 1) / 2 + modifier,
            "{}: hit points against {}",
            b.name,
            b.hit_dice
        );
        let constitution = ability_modifier(b.abilities["constitution"] as i32) as i64;
        assert_eq!(
            modifier,
            count * constitution,
            "{}: the modifier on {} is not {count} x its Constitution modifier",
            b.name,
            b.hit_dice
        );
    }
}

/// **The book's check on an attack bonus**: proficiency bonus plus Strength
/// or Dexterity modifier, and the same modifier is the flat part of the
/// damage. An attack matching neither ability was typed wrong.
#[test]
fn every_attack_bonus_is_proficiency_plus_strength_or_dexterity() {
    for b in stat_blocks() {
        let bonus = proficiency_bonus_for_challenge(&b.challenge)
            .unwrap_or_else(|| panic!("{}: challenge {}", b.name, b.challenge))
            as i64;
        let strength = ability_modifier(b.abilities["strength"] as i32) as i64;
        let dexterity = ability_modifier(b.abilities["dexterity"] as i32) as i64;
        for attack in &b.attacks {
            let modifier = attack.to_hit - bonus;
            assert!(
                modifier == strength || modifier == dexterity,
                "{} {}: +{} is neither Strength nor Dexterity plus {bonus}",
                b.name,
                attack.name,
                attack.to_hit
            );
            let (_, _, flat) = dice(
                attack
                    .damage
                    .split('+')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join("+")
                    .as_str(),
            );
            assert_eq!(
                flat, modifier,
                "{} {}: damage {} does not add the modifier the attack bonus uses",
                b.name, attack.name, attack.damage
            );
            assert!(
                attack.reach.is_some() || attack.range.is_some(),
                "{} {}: neither a reach nor a range",
                b.name,
                attack.name
            );
            if let Some((normal, long)) = attack.range {
                assert!(normal > 0 && long >= normal, "{} {}", b.name, attack.name);
            }
        }
        for part in &b.multiattack {
            assert!(
                b.attacks.iter().any(|a| &a.name == part),
                "{}: Multiattack names {part}, which it does not have",
                b.name
            );
        }
    }
}

/// Every block, spread across the slots, is something this pack's own
/// validators accept. This is the test that failed for an ogre while ability
/// scores stopped at twenty.
#[test]
fn every_block_passes_the_validators_it_will_be_saved_through() {
    let sizes = declared_size_ids();
    for b in stat_blocks() {
        assert!(sizes.contains(&b.size), "{}: size {}", b.name, b.size);
        let slots = b.slots();
        for (slot, result) in [
            ("ability_data", validate_ability_data(&slots.ability_data)),
            (
                "resource_data",
                validate_resource_data(&slots.resource_data),
            ),
            (
                "proficiency_data",
                validate_proficiency_data(&slots.proficiency_data),
            ),
            ("trait_data", validate_trait_data(&slots.trait_data)),
        ] {
            if let Err(refused) = result {
                panic!("{} {slot}: {} {}", b.name, refused.field, refused.message);
            }
        }
    }
}

/// The numbers a Game Master will check against the book, derived rather
/// than stored: a goblin's Stealth is +6 because Dexterity 15 and a doubled
/// bonus say so.
#[test]
fn the_rules_derive_what_the_book_prints() {
    let goblin = block("goblin-warrior");
    assert_eq!(goblin.hit_points, 10);
    assert_eq!(goblin.armor_class, 15);
    assert_eq!(derived(&goblin, "proficiencyBonus"), Some(2));
    assert_eq!(derived(&goblin, "skillStealth"), Some(6));
    assert_eq!(derived(&goblin, "passivePerception"), Some(9));

    let troll = block("troll");
    assert_eq!(derived(&troll, "proficiencyBonus"), Some(3));
    assert_eq!(derived(&troll, "skillPerception"), Some(5));
    assert_eq!(derived(&troll, "passivePerception"), Some(15));

    let wyrmling = block("white-dragon-wyrmling");
    assert_eq!(derived(&wyrmling, "saveDexterity"), Some(2));
    assert_eq!(derived(&wyrmling, "saveWisdom"), Some(2));
    assert_eq!(derived(&wyrmling, "skillPerception"), Some(4));

    let zombie = block("zombie");
    assert_eq!(derived(&zombie, "saveWisdom"), Some(0));
    assert_eq!(derived(&zombie, "strengthMod"), Some(1));
}

/// What the board reads is written where the manifest says it reads it.
#[test]
fn a_block_writes_the_fields_the_manifest_points_combat_at() {
    let manifest = manifest();
    let imp = block("imp").slots();

    let hp = &manifest["combat"]["hitPoints"];
    assert_eq!(imp.resource_data[hp["max"].as_str().unwrap()], json!(21));
    assert_eq!(
        imp.resource_data[hp["current"].as_str().unwrap()],
        json!(21)
    );
    let defence = manifest["combat"]["defence"]["field"].as_str().unwrap();
    assert_eq!(imp.ability_data[defence], json!(13));
    let size = manifest["combat"]["sizes"]["source"]["field"]
        .as_str()
        .unwrap();
    assert_eq!(imp.trait_data[size], json!("tiny"));
    assert_eq!(imp.trait_data["darkvision"], json!(120));
    assert_eq!(imp.trait_data["speed_walk"], json!(20));
    assert_eq!(imp.trait_data["speed_fly"], json!(40));
}
