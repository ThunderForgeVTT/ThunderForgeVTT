//! The D&D Beyond reader against the generated fixtures (spec 048 T029).
//!
//! Each fixture is read from its committed file, the way an upload is, and
//! checked against the invented character it was printed from
//! (`fixtures/characters.rs`) and against contracts/sheet-mapping-5e.md.

#[allow(dead_code)]
#[path = "fixtures/characters.rs"]
mod characters;
#[allow(dead_code)]
#[path = "fixtures/gen.rs"]
mod generator;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::json;
use thunderforge_pdf::Document;
use thunderforge_sheet_import::{
    Certainty, Field, ImportedCharacter, NamedContent, ReadError, Recognition, SheetReader,
    SkillMark,
};
use thunderforge_system_dnd5e_sheet::DdbPdf;

fn open(file: &str) -> Document {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(file);
    Document::open(&path).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn read(file: &str) -> ImportedCharacter {
    let doc = open(file);
    assert_eq!(DdbPdf.recognise(&doc), Recognition::Yes, "{file}");
    DdbPdf.read(&doc).unwrap_or_else(|e| panic!("{file}: {e}"))
}

/// The value of a field the reader read plainly.
#[track_caller]
fn got<T: Clone + std::fmt::Debug>(field: &Field<T>) -> T {
    assert_eq!(field.certainty, Certainty::Read, "{field:?}");
    field.value.clone().expect("a read field has a value")
}

fn named<'a>(c: &'a ImportedCharacter, kind: &str, name: &str) -> &'a NamedContent {
    c.content
        .iter()
        .find(|item| item.kind == kind && item.name.value.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {kind} named {name}"))
}

fn names(c: &ImportedCharacter, kind: &str) -> Vec<String> {
    c.content
        .iter()
        .filter(|item| item.kind == kind)
        .map(|item| got(&item.name))
        .collect()
}

const SKILLS: [&str; 18] = [
    "acrobatics",
    "animal_handling",
    "arcana",
    "athletics",
    "deception",
    "history",
    "insight",
    "intimidation",
    "investigation",
    "medicine",
    "nature",
    "perception",
    "performance",
    "persuasion",
    "religion",
    "sleight_of_hand",
    "stealth",
    "survival",
];
const ABILITIES: [&str; 6] = ["str", "dex", "con", "int", "wis", "cha"];

// --- recognition -----------------------------------------------------------

#[test]
fn every_export_is_recognised_and_nothing_else_is() {
    for character in characters::all() {
        let doc = open(character.file);
        assert_eq!(
            DdbPdf.recognise(&doc),
            Recognition::Yes,
            "{}",
            character.file
        );
    }
    let other = open("not-a-ddb-sheet.pdf");
    assert_eq!(
        DdbPdf.recognise(&other),
        Recognition::No {
            reason: "This does not look like a D&D Beyond character sheet.".into()
        }
    );
    assert!(matches!(
        DdbPdf.read(&other),
        Err(ReadError::NotRecognised(reason)) if reason.contains("D&D Beyond")
    ));
    assert_eq!(DdbPdf.id(), "ddb-pdf");
}

#[test]
fn the_reading_names_its_reader() {
    let c = read("fighter-5.pdf");
    assert_eq!(c.reader.id, "ddb-pdf");
    assert_eq!(c.reader.version, DdbPdf.version());
}

// --- every table row, on the reference character -------------------------

#[test]
fn fighter_identity_and_classes() {
    let c = read("fighter-5.pdf");
    let id = &c.identity;
    assert_eq!(got(&id.name), "Brannoc Vell");
    assert_eq!(got(&id.player_name), "fixture-player");
    assert_eq!(got(&id.species), "Human");
    assert_eq!(got(&id.background), "Soldier");
    assert_eq!(got(&id.alignment), "Lawful Neutral");
    assert_eq!(got(&id.size), "Medium");
    assert_eq!(got(&id.xp), 6500);

    assert_eq!(c.classes.len(), 1);
    assert_eq!(got(&c.classes[0].name), "Fighter");
    assert_eq!(got(&c.classes[0].level), 5);
    assert_eq!(got(&c.classes[0].hit_die), 10);
    // The export prints no subclass on the class line.
    assert_eq!(c.classes[0].subclass.certainty, Certainty::Unread);

    let source = c.identity.name.source.as_ref().expect("a source");
    assert_eq!(source.page, 1);
    assert_eq!(source.text, "Brannoc Vell");
    assert!(source.rect.x1 > source.rect.x0 && source.rect.y1 > source.rect.y0);
}

#[test]
fn fighter_abilities_saves_and_skills() {
    let c = read("fighter-5.pdf");
    let printed = characters::fighter_5();
    for (index, key) in ABILITIES.iter().enumerate() {
        assert_eq!(got(&c.abilities[*key]), printed.scores[index], "{key}");
        assert_eq!(
            got(&c.proficiencies.saves[*key]),
            printed.save_proficient[index],
            "{key}"
        );
    }
    assert_eq!(
        c.proficiencies.skills.keys().cloned().collect::<Vec<_>>(),
        {
            let mut sorted = SKILLS.map(String::from).to_vec();
            sorted.sort();
            sorted
        }
    );
    for skill in SKILLS {
        let want = if ["athletics", "intimidation", "perception", "survival"].contains(&skill) {
            SkillMark::Proficient
        } else {
            SkillMark::None
        };
        assert_eq!(got(&c.proficiencies.skills[skill]), want, "{skill}");
    }

    let p = &c.proficiencies;
    assert_eq!(
        got(&p.armour),
        ["Heavy Armor", "Light Armor", "Medium Armor", "Shields"]
    );
    assert_eq!(got(&p.weapons), ["Martial Weapons", "Simple Weapons"]);
    assert_eq!(got(&p.tools), ["Dice Set", "Vehicles (Land)"]);
    assert_eq!(got(&p.languages), ["Common", "Orc"]);
}

#[test]
fn fighter_derived_values_are_kept_to_be_checked() {
    let c = read("fighter-5.pdf");
    let derived: BTreeMap<&str, i32> = c
        .derived
        .iter()
        .map(|(key, field)| (key.as_str(), got(field)))
        .collect();
    // STR 16, DEX 14, CON 15, INT 10, WIS 12, CHA 8; level 5, so +3.
    assert_eq!(derived["proficiency_bonus"], 3);
    assert_eq!(derived["initiative"], 2);
    assert_eq!(derived["modifier.str"], 3);
    assert_eq!(derived["modifier.cha"], -1);
    assert_eq!(derived["save.str"], 6);
    assert_eq!(derived["save.dex"], 2);
    assert_eq!(derived["skill.athletics"], 6);
    assert_eq!(derived["skill.stealth"], 2);
    assert_eq!(derived["passive.perception"], 14);
    assert_eq!(derived["passive.insight"], 11);
    assert_eq!(derived["passive.investigation"], 10);
    assert_eq!(
        derived.len(),
        3 + 6 + 6 + 18 + 2,
        "passives, modifiers, saves, skills, initiative and bonus"
    );
}

#[test]
fn fighter_defences_resources_and_movement() {
    let c = read("fighter-5.pdf");
    let d = &c.defences;
    assert_eq!(got(&d.armour_class), 19);
    for list in [
        &d.resistances,
        &d.immunities,
        &d.vulnerabilities,
        &d.condition_immunities,
    ] {
        assert_eq!(got(list), Vec::<String>::new());
    }

    let r = &c.resources;
    assert_eq!(got(&r.hp_max), 44);
    // D&D Beyond prints current hit points blank: nothing to write.
    assert_eq!(r.hp_current.certainty, Certainty::Unread);
    assert_eq!(got(&r.hp_temp), 0);
    assert_eq!(r.hit_dice.len(), 1);
    assert_eq!(got(&r.hit_dice[0].die), 10);
    assert_eq!(got(&r.hit_dice[0].total), 5);
    assert_eq!(r.hit_dice[0].used.certainty, Certainty::Unread);
    assert_eq!(got(&r.death_saves.successes), 0);
    assert_eq!(got(&r.death_saves.failures), 0);
    assert!(!got(&r.inspiration));
    let coins: BTreeMap<&str, i64> = r.coins.iter().map(|(k, v)| (k.as_str(), got(v))).collect();
    assert_eq!(
        coins,
        BTreeMap::from([("cp", 12), ("sp", 40), ("ep", 0), ("gp", 1215), ("pp", 3)])
    );

    assert_eq!(
        c.movement
            .speeds
            .iter()
            .map(|(k, v)| (k.as_str(), got(v)))
            .collect::<Vec<_>>(),
        [("walk", 30)]
    );
    assert!(c.movement.senses.is_empty());
    assert!(c.spellcasting.is_empty());
}

#[test]
fn fighter_persona_and_notes() {
    let c = read("fighter-5.pdf");
    let persona: BTreeMap<&str, String> = c
        .persona
        .iter()
        .map(|(k, v)| (k.as_str(), got(v)))
        .collect();
    assert_eq!(persona["gender"], "Male");
    assert_eq!(persona["age"], "34");
    assert_eq!(persona["height"], "6'1\"");
    assert_eq!(persona["weight"], "210 lb.");
    assert_eq!(persona["faith"], "The Lantern Keeper");
    assert_eq!(persona["skin"], "Weathered");
    assert_eq!(persona["eyes"], "Grey");
    assert_eq!(persona["hair"], "Black, cropped");
    assert_eq!(
        persona["personality_traits"],
        "Counts exits in every room he enters."
    );
    assert_eq!(
        persona["ideals"],
        "Duty. A promise made to the unit is kept."
    );
    assert_eq!(
        persona["bonds"],
        "He still carries his old captain's whistle."
    );
    assert_eq!(
        persona["flaws"],
        "Takes orders from no one he has not seen fight."
    );
    assert_eq!(
        persona["appearance"],
        "A scar runs from his left ear to his chin."
    );
    assert!(persona["backstory"].starts_with("Brannoc held the river ford"));
    assert_eq!(persona["allies_and_organizations"], "The Ford Wardens");
    assert_eq!(persona.len(), 15);

    let notes: BTreeMap<&str, String> = c
        .notes
        .iter()
        .map(|n| (n.label.as_str(), got(&n.text)))
        .collect();
    assert_eq!(notes["Additional notes"], "Owes a favour to a ferryman.");
    assert!(notes["Actions"].contains("Attack: 2 per Attack action"));
    assert!(notes["Actions"].contains("=== BONUS ACTIONS ==="));
    assert!(
        !notes.contains_key("Saving throw modifiers"),
        "it was blank"
    );
}

/// Every value on the reference sheet is read plainly, or not at all where
/// the export prints nothing; nothing is uncertain, and nothing read lacks
/// the box it came from.
#[test]
fn fighter_every_row_is_read_or_unread() {
    let c = read("fighter-5.pdf");
    let mut unread = BTreeSet::new();
    for leaf in c.leaves() {
        match &leaf.certainty {
            Certainty::Read => {
                // A blank box (an unmarked skill, no defences) is read as
                // blank, and still points at its box.
                let source = leaf.source.as_ref();
                assert!(
                    source.is_some_and(|s| s.page >= 1 && s.rect.x1 > s.rect.x0),
                    "{} has no source",
                    leaf.path
                );
            }
            Certainty::Unread => {
                unread.insert(leaf.path.clone());
            }
            Certainty::Uncertain { reason } => panic!("{} uncertain: {reason}", leaf.path),
        }
    }
    assert_eq!(
        unread,
        BTreeSet::from([
            "classes.0.subclass".to_string(),
            "resources.hit_dice.0.used".to_string(),
            "resources.hp_current".to_string(),
        ])
    );
}

// --- content rows ----------------------------------------------------------

#[test]
fn fighter_features_with_uses_and_who_grants_them() {
    let c = read("fighter-5.pdf");
    assert_eq!(
        names(&c, "feature"),
        [
            "Fighting Style",
            "Second Wind",
            "Action Surge",
            "Martial Archetype",
            "Improved Critical",
            "Military Rank",
        ]
    );
    assert_eq!(
        names(&c, "species_trait"),
        ["Ability Score Increase", "Languages"]
    );
    assert_eq!(names(&c, "feat"), ["Alert"]);

    let wind = named(&c, "feature", "Second Wind");
    assert_eq!(wind.link.granted_by, ["Fighter"]);
    assert_eq!(wind.link.uses, Some(1));
    assert_eq!(wind.link.recharge.as_deref(), Some("short_rest"));
    assert_eq!(wind.fields["activation"], json!("1 Bonus Action"));
    assert_eq!(wind.fields["source"], json!("PHB 72"));
    assert_eq!(
        wind.fields["description"],
        json!("On your turn, regain 1d10 + fighter level hit points.")
    );

    let style = named(&c, "feature", "Fighting Style");
    assert_eq!(style.link.uses, None);
    assert_eq!(style.link.recharge, None);
    assert!(
        !named(&c, "feature", "Martial Archetype")
            .fields
            .contains_key("description")
    );

    assert_eq!(
        named(&c, "species_trait", "Languages").link.granted_by,
        ["Human"]
    );
    assert_eq!(
        named(&c, "feature", "Military Rank").link.granted_by,
        ["Soldier"]
    );
    assert!(named(&c, "feat", "Alert").link.granted_by.is_empty());
}

#[test]
fn fighter_equipment_and_attacks() {
    let c = read("fighter-5.pdf");
    let items: Vec<(String, Option<i32>, Option<f64>)> = c
        .content
        .iter()
        .filter(|item| item.kind == "item")
        .map(|item| {
            (
                got(&item.name),
                item.link.quantity,
                item.fields.get("weight").and_then(|w| w.as_f64()),
            )
        })
        .collect();
    assert_eq!(
        items,
        [
            ("Chain Mail".into(), Some(1), Some(55.0)),
            ("Longsword".into(), Some(1), Some(3.0)),
            ("Shield".into(), Some(1), Some(6.0)),
            ("Light Crossbow".into(), Some(1), Some(5.0)),
            ("Crossbow Bolts".into(), Some(20), Some(1.5)),
            ("Explorer's Pack".into(), Some(1), Some(59.0)),
            ("Rations (1 day)".into(), Some(10), Some(20.0)),
            ("Insignia of Rank".into(), Some(1), None),
        ]
    );
    // The export has no equipped column, so equipped is never claimed.
    assert!(c.content.iter().all(|item| item.link.equipped.is_none()));

    assert_eq!(
        names(&c, "attack"),
        ["Longsword", "Light Crossbow", "Unarmed Strike"]
    );
    let sword = named(&c, "attack", "Longsword");
    assert_eq!(sword.fields["to_hit"], json!(6));
    assert_eq!(sword.fields["damage"], json!("1d8+3"));
    assert_eq!(sword.fields["damage_type"], json!("slashing"));
    assert_eq!(sword.fields["notes"], json!("Martial, Versatile, Sap"));
    let fist = named(&c, "attack", "Unarmed Strike");
    assert_eq!(fist.fields["damage"], json!("4"));
    assert_eq!(fist.fields["damage_type"], json!("bludgeoning"));
    assert!(!fist.fields.contains_key("notes"));
}

#[test]
fn a_non_caster_has_no_spell_page_and_no_spells() {
    for file in ["fighter-5.pdf", "rogue-4.pdf"] {
        let c = read(file);
        assert!(c.spellcasting.is_empty(), "{file}");
        assert!(names(&c, "spell").is_empty(), "{file}");
    }
}

// --- the other fixtures ----------------------------------------------------

#[test]
fn multiclass_reads_two_classes_and_one_casting_class() {
    let c = read("fighter3-wizard2.pdf");
    let classes: Vec<(String, i32, i32)> = c
        .classes
        .iter()
        .map(|k| (got(&k.name), got(&k.level), got(&k.hit_die)))
        .collect();
    assert_eq!(
        classes,
        [("Fighter".into(), 3, 10), ("Wizard".into(), 2, 6)]
    );
    let pools: Vec<(i32, i32)> = c
        .resources
        .hit_dice
        .iter()
        .map(|h| (got(&h.die), got(&h.total)))
        .collect();
    assert_eq!(pools, [(10, 3), (6, 2)]);
    // "(Milestone)" is read, and holds no number.
    assert_eq!(c.identity.xp.certainty, Certainty::Read);
    assert_eq!(c.identity.xp.value, None);
    assert_eq!(got(&c.derived["proficiency_bonus"]), 3);
    assert_eq!(got(&c.movement.senses["darkvision"]), 60);

    assert_eq!(c.spellcasting.len(), 1);
    let w = &c.spellcasting[0];
    assert_eq!(got(&w.class), "Wizard");
    assert_eq!(got(&w.ability), "int");
    assert_eq!(got(&w.save_dc), 14);
    assert_eq!(got(&w.attack_bonus), 6);
    assert_eq!(
        w.slots
            .iter()
            .map(|(k, v)| (k.as_str(), got(v)))
            .collect::<Vec<_>>(),
        [("1", 3)]
    );
    assert!(w.pact_slots.is_none());

    assert_eq!(
        names(&c, "spell"),
        [
            "Fire Bolt",
            "Mage Hand",
            "Prestidigitation",
            "Magic Missile",
            "Shield",
            "Sleep",
            "Detect Magic",
        ]
    );
    let bolt = named(&c, "spell", "Fire Bolt");
    assert_eq!(bolt.fields["level"], json!(0));
    assert_eq!(bolt.link.prepared, None);
    assert_eq!(bolt.link.granted_by, ["Wizard"]);
    assert_eq!(bolt.fields["range"], json!("120 ft."));
    assert_eq!(bolt.fields["casting_time"], json!("1A"));
    assert_eq!(bolt.fields["components"], json!("V,S"));
    assert_eq!(bolt.fields["duration"], json!("Instantaneous"));
    assert_eq!(bolt.fields["source"], json!("PHB 242"));
    assert_eq!(bolt.fields["save_hit"], json!("+6"));
    let missile = named(&c, "spell", "Magic Missile");
    assert_eq!(missile.fields["level"], json!(1));
    assert_eq!(missile.link.prepared, Some(true));
    assert_eq!(
        named(&c, "spell", "Detect Magic").link.prepared,
        Some(false)
    );

    // Fire Bolt is an attack row too; the mapping decides where it lands.
    assert_eq!(names(&c, "attack"), ["Longsword", "Fire Bolt"]);
}

#[test]
fn cleric_prepared_marks_domain_spells_and_slots_to_four() {
    let c = read("cleric-7.pdf");
    let cleric = &c.spellcasting[0];
    assert_eq!(got(&cleric.ability), "wis");
    assert_eq!(
        cleric
            .slots
            .iter()
            .map(|(k, v)| (k.as_str(), got(v)))
            .collect::<Vec<_>>(),
        [("1", 4), ("2", 3), ("3", 3), ("4", 1)]
    );
    let bless = named(&c, "spell", "Bless");
    assert_eq!(bless.link.granted_by, ["Life Domain"]);
    assert_eq!(bless.link.prepared, Some(true));
    assert_eq!(bless.fields["level"], json!(1));
    assert_eq!(
        named(&c, "spell", "Guiding Bolt").link.prepared,
        Some(false)
    );
    assert_eq!(
        named(&c, "spell", "Healing Word").link.granted_by,
        ["Cleric"]
    );
    let ward = named(&c, "spell", "Death Ward");
    assert_eq!(ward.fields["level"], json!(4));
    assert_eq!(ward.link.granted_by, ["Life Domain"]);
    assert_eq!(names(&c, "spell").len(), 3 + 4 + 2 + 2 + 1);
    let guardians = named(&c, "spell", "Spirit Guardians");
    assert_eq!(guardians.fields["save_hit"], json!("WIS 15"));
    assert_eq!(guardians.fields["range"], json!("Self/15 ft. Sphere"));

    // Features run over five columns and the overflow page.
    let channel = named(&c, "feature", "Channel Divinity");
    assert_eq!(channel.link.uses, Some(2));
    assert_eq!(channel.link.recharge.as_deref(), Some("short_rest"));
    assert_eq!(channel.link.granted_by, ["Cleric"]);
    assert_eq!(
        names(&c, "feature"),
        [
            "Spellcasting",
            "Divine Domain",
            "Disciple of Life",
            "Channel Divinity",
            "Preserve Life",
            "Destroy Undead",
            "Divine Strike",
            "Shelter of the Faithful",
        ]
    );
    assert_eq!(
        named(&c, "species_trait", "Stonecunning").link.granted_by,
        ["Hill Dwarf"]
    );

    let periapt = named(&c, "item", "Periapt of Wound Closure");
    assert_eq!(periapt.link.attuned, Some(true));
    assert_eq!(periapt.link.quantity, Some(1));
    assert_eq!(named(&c, "item", "Mace").link.attuned, None);

    assert_eq!(got(&c.defences.resistances), ["poison"]);
    assert_eq!(got(&c.movement.speeds["walk"]), 25);
    let saves = c
        .notes
        .iter()
        .find(|n| n.label == "Saving throw modifiers")
        .expect("the save modifiers are kept");
    assert_eq!(got(&saves.text), "Advantage on saves against poison");

    let flame = named(&c, "attack", "Sacred Flame");
    assert_eq!(flame.fields["save_ability"], json!("dex"));
    assert_eq!(flame.fields["save_dc"], json!(15));
    assert!(!flame.fields.contains_key("to_hit"));
}

#[test]
fn rogue_expertise_and_a_feature_with_no_uses() {
    let c = read("rogue-4.pdf");
    let skills = &c.proficiencies.skills;
    assert_eq!(got(&skills["stealth"]), SkillMark::Expertise);
    assert_eq!(got(&skills["sleight_of_hand"]), SkillMark::Expertise);
    assert_eq!(got(&skills["acrobatics"]), SkillMark::Proficient);
    assert_eq!(got(&skills["athletics"]), SkillMark::None);
    assert_eq!(got(&c.identity.size), "Small");
    assert_eq!(got(&c.classes[0].hit_die), 8);

    let sneak = named(&c, "feature", "Sneak Attack");
    assert_eq!(sneak.link.uses, None);
    assert_eq!(sneak.link.recharge, None);
    assert!(
        sneak.fields["description"]
            .as_str()
            .is_some_and(|d| d.starts_with("Once per turn"))
    );
    assert_eq!(
        names(&c, "species_trait"),
        ["Lucky", "Brave", "Naturally Stealthy"]
    );
    // The details page is blank; nothing is invented for it.
    assert!(c.persona.is_empty());
}

#[test]
fn warforged_poison_is_a_resistance() {
    let c = read("warforged-defences.pdf");
    assert_eq!(got(&c.defences.resistances), ["poison"]);
    assert_eq!(got(&c.defences.immunities), Vec::<String>::new());
    assert!(
        named(&c, "species_trait", "Constructed Resilience").fields["description"]
            .as_str()
            .is_some_and(|d| d.contains("immune to disease"))
    );
}

#[test]
fn an_unknown_mark_is_uncertain_and_nothing_is_invented() {
    let c = read("uncertain-mark.pdf");
    let athletics = &c.proficiencies.skills["athletics"];
    assert_eq!(athletics.value, None);
    let Certainty::Uncertain { reason } = &athletics.certainty else {
        panic!("{athletics:?}");
    };
    assert!(reason.contains("Athletics"), "{reason}");
    assert!(reason.contains('\u{25A1}'), "{reason}");
    assert_eq!(
        athletics.source.as_ref().map(|s| s.text.as_str()),
        Some("\u{25A1}")
    );
    let uncertain: Vec<String> = c
        .leaves()
        .into_iter()
        .filter(|leaf| matches!(leaf.certainty, Certainty::Uncertain { .. }))
        .map(|leaf| leaf.path)
        .collect();
    assert_eq!(uncertain, ["proficiencies.skills.athletics"]);
}

#[test]
fn reading_is_deterministic() {
    for character in characters::all() {
        assert_eq!(
            read(character.file),
            read(character.file),
            "{}",
            character.file
        );
    }
}
