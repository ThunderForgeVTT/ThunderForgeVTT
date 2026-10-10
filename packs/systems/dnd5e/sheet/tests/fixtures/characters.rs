//! The characters the fixtures print (contracts/sheet-mapping-5e.md,
//! "Fixture expectations"). Everyone here is invented, and every description
//! is in our own words; only the shapes follow D&D Beyond's export.

use crate::generator::{Character, Item, Persona, Spell, SpellLevel, Spellcasting};

const FIGHTER_FEATURES: &str = "=== FIGHTER FEATURES ===
* Fighting Style • PHB 72
Defense: +1 to AC while wearing armor.
* Second Wind • PHB 72
On your turn, regain 1d10 + fighter level hit points.
   | 1 / Short Rest • 1 Bonus Action
* Action Surge • PHB 72
Take one additional action on your turn.
   | 1 / Short Rest • Special
* Martial Archetype • PHB 72
* Improved Critical • PHB 72
Your weapon attacks score a critical hit on a roll of 19 or 20.";

const FIGHTER_TRAITS: &str = "=== HUMAN TRAITS ===
* Ability Score Increase • PHB 31
* Languages • PHB 31
=== FEATS ===
* Alert • PHB 165
You can't be surprised while you are conscious.";

const FIGHTER_BACKGROUND: &str = "=== BACKGROUND FEATURE ===
* Military Rank • PHB 140
Soldiers loyal to your former unit still recognise your rank.";

fn spell(
    prepared: &'static str,
    name: &'static str,
    source: &'static str,
    save_hit: &'static str,
    time: &'static str,
    range: &'static str,
    duration: &'static str,
    page: &'static str,
) -> Spell {
    Spell {
        prepared,
        name,
        source,
        save_hit,
        time,
        range,
        components: "V,S",
        duration,
        page,
        notes: "",
    }
}

fn equipment() -> Vec<Item> {
    vec![
        ("Chain Mail", "1", "55 lb."),
        ("Longsword", "1", "3 lb."),
        ("Shield", "1", "6 lb."),
        ("Light Crossbow", "1", "5 lb."),
        ("Crossbow Bolts", "20", "1.5 lb."),
        ("Explorer's Pack", "1", "59 lb."),
        ("Rations (1 day)", "10", "20 lb."),
        ("Insignia of Rank", "1", "--"),
    ]
}

/// The reference character: a single-class level 5 fighter, every block of
/// the export filled, no spell page.
pub fn fighter_5() -> Character {
    Character {
        file: "fighter-5.pdf",
        name: "Brannoc Vell",
        player: "fixture-player",
        class_level: "Fighter 5",
        level: 5,
        species: "Human",
        background: "Soldier",
        alignment: "Lawful Neutral",
        size: "Medium",
        xp: "6500",
        scores: [16, 14, 15, 10, 12, 8],
        save_proficient: [true, false, true, false, false, false],
        skills: vec![
            ("athletics", 'P'),
            ("intimidation", 'P'),
            ("perception", 'P'),
            ("survival", 'P'),
        ],
        armour_class: 19,
        max_hp: 44,
        speed: "30 ft. (Walking)",
        hit_dice: "5d10",
        senses: "",
        defenses: "",
        save_modifiers: "",
        proficiencies: "=== ARMOR ===
Heavy Armor, Light Armor, Medium Armor, Shields

=== WEAPONS ===
Martial Weapons, Simple Weapons

=== TOOLS ===
Dice Set, Vehicles (Land)

=== LANGUAGES ===
Common, Orc",
        weapons: vec![
            (
                "Longsword",
                "+6",
                "1d8+3 Slashing",
                "Martial, Versatile, Sap",
            ),
            (
                "Light Crossbow",
                "+5",
                "1d8+2 Piercing",
                "Simple, Ammunition, Loading, Range (80/320)",
            ),
            ("Unarmed Strike", "+6", "4 Bludgeoning", ""),
        ],
        actions: [
            "=== ACTIONS ===
Attack: 2 per Attack action
Action Surge • 1 / Short Rest
Standard Actions
   Attack, Dash, Disengage, Dodge, Help, Hide, Ready, Search, Use an Object",
            "=== BONUS ACTIONS ===
Second Wind • 1 / Short Rest",
        ],
        features: vec![FIGHTER_FEATURES, FIGHTER_TRAITS, FIGHTER_BACKGROUND],
        coins: ["12", "40", "0", "1,215", "3"],
        weight_carried: "149.5 lb.",
        equipment: equipment(),
        attuned: Vec::new(),
        persona: Persona {
            gender: "Male",
            age: "34",
            height: "6'1\"",
            weight: "210 lb.",
            faith: "The Lantern Keeper",
            skin: "Weathered",
            eyes: "Grey",
            hair: "Black, cropped",
            personality: "Counts exits in every room he enters.",
            ideals: "Duty. A promise made to the unit is kept.",
            bonds: "He still carries his old captain's whistle.",
            flaws: "Takes orders from no one he has not seen fight.",
            appearance: "A scar runs from his left ear to his chin.",
            backstory: "Brannoc held the river ford at Dunmere for three days \
and has not spoken of it since.",
            allies: "The Ford Wardens",
            notes: "Owes a favour to a ferryman.",
        },
        spells: None,
        overrides: Vec::new(),
    }
}

fn wizard_spells(slots_first: &'static str, second: bool) -> Spellcasting {
    let mut levels = vec![
        SpellLevel {
            header: "=== CANTRIPS ===",
            slots: "(At Will)",
            spells: vec![
                spell(
                    "",
                    "Fire Bolt",
                    "Wizard",
                    "+6",
                    "1A",
                    "120 ft.",
                    "Instantaneous",
                    "PHB 242",
                ),
                spell(
                    "",
                    "Mage Hand",
                    "Wizard",
                    "--",
                    "1A",
                    "30 ft.",
                    "1 minute",
                    "PHB 256",
                ),
                spell(
                    "",
                    "Prestidigitation",
                    "Wizard",
                    "--",
                    "1A",
                    "10 ft.",
                    "Up to 1 hour",
                    "PHB 267",
                ),
            ],
        },
        SpellLevel {
            header: "=== 1st LEVEL ===",
            slots: slots_first,
            spells: vec![
                spell(
                    "P",
                    "Magic Missile",
                    "Wizard",
                    "--",
                    "1A",
                    "120 ft.",
                    "Instantaneous",
                    "PHB 257",
                ),
                spell(
                    "P", "Shield", "Wizard", "--", "1R", "Self", "1 round", "PHB 275",
                ),
                spell(
                    "P", "Sleep", "Wizard", "--", "1A", "90 ft.", "1 minute", "PHB 276",
                ),
                spell(
                    "O",
                    "Detect Magic",
                    "Wizard",
                    "--",
                    "1A",
                    "Self",
                    "Concentration, up to 10 minutes",
                    "PHB 231",
                ),
            ],
        },
    ];
    if second {
        levels.push(SpellLevel {
            header: "=== 2nd LEVEL ===",
            slots: "2 Slots OO",
            spells: vec![
                spell(
                    "P",
                    "Misty Step",
                    "Wizard",
                    "--",
                    "1BA",
                    "Self",
                    "Instantaneous",
                    "PHB 260",
                ),
                spell(
                    "O",
                    "Scorching Ray",
                    "Wizard",
                    "+6",
                    "1A",
                    "120 ft.",
                    "Instantaneous",
                    "PHB 273",
                ),
            ],
        });
    }
    Spellcasting {
        class: "Wizard",
        ability: "INT",
        save_dc: "14",
        attack: "+6",
        levels,
    }
}

/// Two classes, level 5: d10 and d6 hit dice, one casting class.
pub fn fighter3_wizard2() -> Character {
    let mut c = fighter_5();
    c.file = "fighter3-wizard2.pdf";
    c.name = "Isolde Maraven";
    c.class_level = "Fighter 3 / Wizard 2";
    c.species = "High Elf";
    c.background = "Sage";
    c.alignment = "Neutral Good";
    c.xp = "(Milestone)";
    c.scores = [13, 14, 14, 16, 12, 10];
    c.skills = vec![
        ("arcana", 'P'),
        ("history", 'P'),
        ("perception", 'P'),
        ("athletics", 'P'),
    ];
    c.armour_class = 16;
    c.max_hp = 38;
    c.speed = "30 ft. (Walking)";
    c.hit_dice = "3d10 + 2d6";
    c.senses = "Darkvision 60 ft.";
    c.proficiencies = "=== ARMOR ===
Heavy Armor, Light Armor, Medium Armor, Shields

=== WEAPONS ===
Longbow, Longsword, Martial Weapons, Shortbow, Shortsword, Simple Weapons

=== TOOLS ===
Calligrapher's Supplies

=== LANGUAGES ===
Common, Draconic, Elvish";
    c.weapons = vec![
        ("Longsword", "+4", "1d8+1 Slashing", "Martial, Versatile"),
        ("Fire Bolt", "+6", "1d10 Fire", "Cantrip, V/S"),
    ];
    c.persona = Persona {
        backstory: "Isolde left the archive to see the battles she catalogued.",
        ..Persona::default()
    };
    c.spells = Some(wizard_spells("3 Slots OOO", false));
    c
}

/// The same character a level later (Wizard 3), for the re-import diff.
pub fn fighter3_wizard2_l6() -> Character {
    let mut c = fighter3_wizard2();
    c.file = "fighter3-wizard2-l6.pdf";
    c.class_level = "Fighter 3 / Wizard 3";
    c.level = 6;
    c.max_hp = 44;
    c.hit_dice = "3d10 + 3d6";
    c.spells = Some(wizard_spells("4 Slots OOOO", true));
    c
}

/// Prepared marks, a domain spell, slots to level 4, and a features list
/// long enough for the overflow page.
pub fn cleric_7() -> Character {
    let domain = |name, time, range, duration, page| {
        spell("P", name, "Life Domain", "--", time, range, duration, page)
    };
    let cleric = |prepared, name, save_hit, time, range, duration, page| {
        spell(
            prepared, name, "Cleric", save_hit, time, range, duration, page,
        )
    };
    let mut c = fighter_5();
    c.file = "cleric-7.pdf";
    c.name = "Sister Aubrel";
    c.class_level = "Cleric 7";
    c.level = 7;
    c.species = "Hill Dwarf";
    c.background = "Acolyte";
    c.alignment = "Neutral Good";
    c.xp = "23000";
    c.scores = [14, 10, 14, 10, 18, 12];
    c.save_proficient = [false, false, false, false, true, true];
    c.skills = vec![("insight", 'P'), ("medicine", 'P'), ("religion", 'P')];
    c.armour_class = 18;
    c.max_hp = 64;
    c.speed = "25 ft. (Walking)";
    c.hit_dice = "7d8";
    c.senses = "Darkvision 60 ft.";
    c.defenses = "Poison - Resistance";
    c.save_modifiers = "Advantage on saves against poison";
    c.weapons = vec![
        ("Mace", "+5", "1d6+2 Bludgeoning", "Simple"),
        ("Sacred Flame", "DEX 15", "2d8 Radiant", "Cantrip, V/S"),
    ];
    c.actions = [
        "=== ACTIONS ===
Channel Divinity • 2 / Short Rest
Standard Actions
   Attack, Dash, Disengage, Dodge, Help, Hide, Ready, Search, Use an Object",
        "",
    ];
    c.features = vec![
        "=== CLERIC FEATURES ===
* Spellcasting • PHB 58
* Divine Domain • PHB 58
* Disciple of Life • PHB 60
Healing spells of 1st level or higher restore extra hit points.",
        "* Channel Divinity • PHB 58
   | 2 / Short Rest • 1 Action
* Preserve Life • PHB 60
Share out five times your cleric level in hit points among nearby creatures.",
        "* Destroy Undead • PHB 59
* Divine Strike • PHB 60
Once per turn, a weapon hit deals an extra 1d8 radiant damage.",
        "=== HILL DWARF TRAITS ===
* Dwarven Resilience • PHB 20
* Dwarven Toughness • PHB 20
* Stonecunning • PHB 20",
        "=== BACKGROUND FEATURE ===
* Shelter of the Faithful • PHB 127",
    ];
    c.equipment = vec![
        ("Chain Mail", "1", "55 lb."),
        ("Mace", "1", "4 lb."),
        ("Shield", "1", "6 lb."),
        ("Holy Symbol", "1", "--"),
        ("Priest's Pack", "1", "24 lb."),
    ];
    c.attuned = vec!["Periapt of Wound Closure"];
    c.persona = Persona {
        faith: "The Hearthmother",
        personality: "Hums while she bandages.",
        ..Persona::default()
    };
    c.spells = Some(Spellcasting {
        class: "Cleric",
        ability: "WIS",
        save_dc: "15",
        attack: "+7",
        levels: vec![
            SpellLevel {
                header: "=== CANTRIPS ===",
                slots: "(At Will)",
                spells: vec![
                    cleric(
                        "",
                        "Sacred Flame",
                        "DEX 15",
                        "1A",
                        "60 ft.",
                        "Instantaneous",
                        "PHB 272",
                    ),
                    cleric(
                        "",
                        "Guidance",
                        "--",
                        "1A",
                        "Touch",
                        "Concentration, up to 1 minute",
                        "PHB 248",
                    ),
                    cleric(
                        "",
                        "Spare the Dying",
                        "--",
                        "1A",
                        "Touch",
                        "Instantaneous",
                        "PHB 277",
                    ),
                ],
            },
            SpellLevel {
                header: "=== 1st LEVEL ===",
                slots: "4 Slots OOOO",
                spells: vec![
                    domain(
                        "Bless",
                        "1A",
                        "30 ft.",
                        "Concentration, up to 1 minute",
                        "PHB 219",
                    ),
                    domain("Cure Wounds", "1A", "Touch", "Instantaneous", "PHB 230"),
                    cleric(
                        "P",
                        "Healing Word",
                        "--",
                        "1BA",
                        "60 ft.",
                        "Instantaneous",
                        "PHB 250",
                    ),
                    cleric(
                        "O",
                        "Guiding Bolt",
                        "+7",
                        "1A",
                        "120 ft.",
                        "1 round",
                        "PHB 248",
                    ),
                ],
            },
            SpellLevel {
                header: "=== 2nd LEVEL ===",
                slots: "3 Slots OOO",
                spells: vec![
                    domain(
                        "Lesser Restoration",
                        "1A",
                        "Touch",
                        "Instantaneous",
                        "PHB 255",
                    ),
                    cleric(
                        "P",
                        "Spiritual Weapon",
                        "+7",
                        "1BA",
                        "60 ft.",
                        "1 minute",
                        "PHB 278",
                    ),
                ],
            },
            SpellLevel {
                header: "=== 3rd LEVEL ===",
                slots: "3 Slots OOO",
                spells: vec![
                    domain("Revivify", "1A", "Touch", "Instantaneous", "PHB 272"),
                    cleric(
                        "P",
                        "Spirit Guardians",
                        "WIS 15",
                        "1A",
                        "Self/15 ft. Sphere",
                        "Concentration, up to 10 minutes",
                        "PHB 278",
                    ),
                ],
            },
            SpellLevel {
                header: "=== 4th LEVEL ===",
                slots: "1 Slot O",
                spells: vec![domain("Death Ward", "1A", "Touch", "8 hours", "PHB 230")],
            },
        ],
    });
    c
}

/// Expertise marks, and Sneak Attack as a feature with no uses.
pub fn rogue_4() -> Character {
    let mut c = fighter_5();
    c.file = "rogue-4.pdf";
    c.name = "Pell Quickfinger";
    c.class_level = "Rogue 4";
    c.level = 4;
    c.species = "Lightfoot Halfling";
    c.background = "Urchin";
    c.alignment = "Chaotic Neutral";
    c.size = "Small";
    c.xp = "2700";
    c.scores = [8, 18, 14, 13, 12, 10];
    c.save_proficient = [false, true, false, true, false, false];
    c.skills = vec![
        ("stealth", 'E'),
        ("sleight_of_hand", 'E'),
        ("acrobatics", 'P'),
        ("perception", 'P'),
        ("deception", 'P'),
        ("investigation", 'P'),
    ];
    c.armour_class = 15;
    c.max_hp = 31;
    c.speed = "25 ft. (Walking)";
    c.hit_dice = "4d8";
    c.weapons = vec![
        (
            "Shortsword",
            "+6",
            "1d6+4 Piercing",
            "Simple, Finesse, Light",
        ),
        (
            "Shortbow",
            "+6",
            "1d6+4 Piercing",
            "Simple, Ammunition, Range (80/320)",
        ),
    ];
    c.actions = [
        "=== ACTIONS ===
Standard Actions
   Attack, Dash, Disengage, Dodge, Help, Hide, Ready, Search, Use an Object",
        "=== BONUS ACTIONS ===
Cunning Action",
    ];
    c.features = vec![
        "=== ROGUE FEATURES ===
* Expertise • PHB 96
* Sneak Attack • PHB 96
Once per turn, deal an extra 2d6 damage to a creature you hit with a finesse \
or ranged weapon when you have advantage.
* Thieves' Cant • PHB 96
* Cunning Action • PHB 96",
        "=== LIGHTFOOT HALFLING TRAITS ===
* Lucky • PHB 28
* Brave • PHB 28
* Naturally Stealthy • PHB 29",
        "",
    ];
    c.proficiencies = "=== ARMOR ===
Light Armor

=== WEAPONS ===
Hand Crossbow, Longsword, Rapier, Shortsword, Simple Weapons

=== TOOLS ===
Disguise Kit, Thieves' Tools

=== LANGUAGES ===
Common, Halfling, Thieves' Cant";
    c.equipment = vec![
        ("Leather Armor", "1", "10 lb."),
        ("Shortsword", "1", "2 lb."),
        ("Thieves' Tools", "1", "1 lb."),
    ];
    c.persona = Persona::default();
    c
}

/// A resistance in the defences block, and an immunity to disease, which no
/// damage type or condition names, printed there and in the trait text.
pub fn warforged_defences() -> Character {
    let mut c = fighter_5();
    c.file = "warforged-defences.pdf";
    c.name = "Ledger";
    c.species = "Warforged";
    c.defenses = "Poison - Resistance, Disease - Immunity";
    c.features = vec![
        FIGHTER_FEATURES,
        "=== WARFORGED TRAITS ===
* Constructed Resilience • ERLW 36
Poison troubles you less, you are immune to disease, and you do not need \
to eat, drink or breathe.
* Sentry's Rest • ERLW 36
* Integrated Protection • ERLW 36",
        FIGHTER_BACKGROUND,
    ];
    c.persona = Persona::default();
    c
}

/// A skill's proficiency mark is a glyph the export never uses.
pub fn uncertain_mark() -> Character {
    let mut c = fighter_5();
    c.file = "uncertain-mark.pdf";
    c.persona = Persona::default();
    c.overrides = vec![("AthleticsProf", "\u{25A1}")];
    c
}

pub fn all() -> Vec<Character> {
    vec![
        fighter_5(),
        fighter3_wizard2(),
        fighter3_wizard2_l6(),
        cleric_7(),
        rogue_4(),
        warforged_defences(),
        uncertain_mark(),
    ]
}
