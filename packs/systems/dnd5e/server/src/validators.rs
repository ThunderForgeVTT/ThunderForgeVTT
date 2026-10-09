// D&D 5e System Validators
// Validates system-specific data stored in world_actor_system_data JSONB columns
// Ensures data integrity without storing constraints in database

/// Validation error for system data
#[derive(Debug, Clone)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

impl std::error::Error for ValidationError {}

/// The size ids this pack's manifest declares under `combat.sizes`.
///
/// Read from the `system.json` compiled in beside this crate, once, rather
/// than kept as a second list here that could drift from the one the grid
/// measures by.
pub fn declared_size_ids() -> &'static [String] {
    static IDS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    IDS.get_or_init(|| {
        serde_json::from_str::<serde_json::Value>(include_str!("../../system.json"))
            .ok()
            .and_then(|manifest| {
                manifest
                    .pointer("/combat/sizes/categories")
                    .and_then(|c| c.as_array())
                    .map(|categories| {
                        categories
                            .iter()
                            .filter_map(|c| c.get("id").and_then(|id| id.as_str()))
                            .map(str::to_string)
                            .collect()
                    })
            })
            .unwrap_or_default()
    })
}

// ============================================================================
// ability_data Validators
// ============================================================================

/// Validates D&D 5e ability scores (1-30 range).
///
/// Thirty, not twenty. Twenty is where a *character's* own progression stops;
/// the game's ceiling for any creature is thirty, an ogre's Strength is 19 and
/// a giant's 25. While this refused 21 a bestiary creature's stat block could
/// not be written at all, and neither could a character wearing a belt that
/// sets Strength past its cap.
pub fn validate_ability_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "ability_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    // Required abilities
    let required_abilities = [
        "strength",
        "dexterity",
        "constitution",
        "intelligence",
        "wisdom",
        "charisma",
    ];

    for ability in &required_abilities {
        let value = obj
            .get(*ability)
            .and_then(|v| v.as_i64())
            .ok_or(ValidationError {
                field: format!("ability_data.{}", ability),
                message: "must be an integer".to_string(),
            })?;

        if !(1..=30).contains(&value) {
            return Err(ValidationError {
                field: format!("ability_data.{}", ability),
                message: "must be between 1 and 30".to_string(),
            });
        }
    }

    // Armour class (spec 046 T050): optional, and this system's declared
    // defence (`combat.defence`). A creature with none recorded has no
    // defence to beat, which the attack says rather than guessing at 10.
    if let Some(armor_class) = obj.get("armor_class") {
        let value = armor_class.as_i64().ok_or(ValidationError {
            field: "ability_data.armor_class".to_string(),
            message: "must be an integer".to_string(),
        })?;
        if value < 0 {
            return Err(ValidationError {
                field: "ability_data.armor_class".to_string(),
                message: "cannot be negative".to_string(),
            });
        }
    }

    Ok(())
}

// ============================================================================
// resource_data Validators
// ============================================================================

/// Validates D&D 5e HP and resources
pub fn validate_resource_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "resource_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    // max_hp is required
    let max_hp = obj
        .get("max_hp")
        .and_then(|v| v.as_i64())
        .ok_or(ValidationError {
            field: "resource_data.max_hp".to_string(),
            message: "must be a positive integer".to_string(),
        })?;

    if max_hp < 1 {
        return Err(ValidationError {
            field: "resource_data.max_hp".to_string(),
            message: "must be at least 1".to_string(),
        });
    }

    // current_hp validation (if present)
    if let Some(current_hp_val) = obj.get("current_hp") {
        let current_hp = current_hp_val.as_i64().ok_or(ValidationError {
            field: "resource_data.current_hp".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if current_hp < 0 {
            return Err(ValidationError {
                field: "resource_data.current_hp".to_string(),
                message: "cannot be negative".to_string(),
            });
        }

        // Optional: warn if current_hp > max_hp (but allow it for temp HP)
        if current_hp > max_hp {
            // This is allowed (temporary HP), but we could log a warning
        }
    }

    // temporary_hp validation (if present)
    if let Some(temp_hp_val) = obj.get("temporary_hp") {
        let temp_hp = temp_hp_val.as_i64().ok_or(ValidationError {
            field: "resource_data.temporary_hp".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if temp_hp < 0 {
            return Err(ValidationError {
                field: "resource_data.temporary_hp".to_string(),
                message: "cannot be negative".to_string(),
            });
        }
    }

    // What the sheet tracks beside the hit points. All optional, so a sheet
    // saved before any of these was checked is still a sheet.
    optional_whole(obj, "resource_data", "hit_dice_used", 0, i64::MAX)?;
    optional_whole(obj, "resource_data", "death_save_successes", 0, 3)?;
    optional_whole(obj, "resource_data", "death_save_failures", 0, 3)?;

    // The dice a creature's hit points are rolled from ("3d6", "8d10+24"): a
    // stat block's own notation, kept so a Game Master can roll a monster's
    // hit points instead of taking the average.
    if let Some(hit_dice) = obj.get("hit_dice").filter(|v| !v.is_null()) {
        let text = hit_dice.as_str().ok_or(ValidationError {
            field: "resource_data.hit_dice".to_string(),
            message: "must be a string or null".to_string(),
        })?;
        if !is_hit_dice(text) {
            return Err(ValidationError {
                field: "resource_data.hit_dice".to_string(),
                message: "must read like 3d6 or 8d10+24".to_string(),
            });
        }
    }

    crate::validators_sheet::resource_fields(obj)?;
    Ok(())
}

/// `3d6`, `8d10+24`, `3d6-3`, and a multiclass character's `3d10 + 2d6`:
/// one or more dice joined by `+`, and at most one flat modifier at the end.
fn is_hit_dice(text: &str) -> bool {
    let whole = |part: &str| {
        !part.is_empty() && part.len() <= 4 && part.bytes().all(|b| b.is_ascii_digit())
    };
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let (dice, modifier) = match compact.rfind('-') {
        Some(at) => (&compact[..at], Some(&compact[at + 1..])),
        None => (compact.as_str(), None),
    };
    let mut terms: Vec<&str> = dice.split('+').collect();
    let mut flat = modifier;
    if flat.is_none() && terms.len() > 1 && !terms[terms.len() - 1].contains('d') {
        flat = terms.pop();
    }
    !terms.is_empty()
        && terms.iter().all(|term| match term.split_once('d') {
            Some((count, die)) => whole(count) && whole(die),
            None => false,
        })
        && flat.is_none_or(whole)
}

/// An optional whole number within `min..=max`. Absent or null is fine.
fn optional_whole(
    obj: &serde_json::Map<String, serde_json::Value>,
    slot: &str,
    key: &str,
    min: i64,
    max: i64,
) -> Result<(), ValidationError> {
    let Some(value) = obj.get(key).filter(|v| !v.is_null()) else {
        return Ok(());
    };
    let field = format!("{slot}.{key}");
    let number = value.as_i64().ok_or(ValidationError {
        field: field.clone(),
        message: "must be a whole number".to_string(),
    })?;
    if number < min {
        return Err(ValidationError {
            field,
            message: if min == 0 {
                "cannot be negative".to_string()
            } else {
                format!("must be at least {min}")
            },
        });
    }
    if number > max {
        return Err(ValidationError {
            field,
            message: format!("must be at most {max}"),
        });
    }
    Ok(())
}

/// An optional distance: a number, never negative. Not held to a whole
/// number, because the board converts these to cells and has always accepted
/// whatever number was stored.
fn optional_distance(
    obj: &serde_json::Map<String, serde_json::Value>,
    slot: &str,
    key: &str,
) -> Result<(), ValidationError> {
    let Some(value) = obj.get(key).filter(|v| !v.is_null()) else {
        return Ok(());
    };
    let field = format!("{slot}.{key}");
    let number = value.as_f64().ok_or(ValidationError {
        field: field.clone(),
        message: "must be a number".to_string(),
    })?;
    if number < 0.0 {
        return Err(ValidationError {
            field,
            message: "cannot be negative".to_string(),
        });
    }
    Ok(())
}

/// An optional piece of text. Absent or null is fine.
fn optional_text(
    obj: &serde_json::Map<String, serde_json::Value>,
    slot: &str,
    key: &str,
) -> Result<(), ValidationError> {
    match obj.get(key) {
        Some(value) if !value.is_string() && !value.is_null() => Err(ValidationError {
            field: format!("{slot}.{key}"),
            message: "must be a string or null".to_string(),
        }),
        _ => Ok(()),
    }
}

/// The challenge ratings a stat block may carry, as the book prints them.
///
/// Text, because three of them are fractions and a fraction stored as a
/// float is a number nobody can read back off a sheet.
pub const CHALLENGE_RATINGS: [&str; 34] = [
    "0", "1/8", "1/4", "1/2", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13",
    "14", "15", "16", "17", "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29",
    "30",
];

/// The distances this pack keeps in `trait_data`: the speeds its manifest
/// declares under `movement`, the senses, and a carried light.
const TRAIT_DISTANCES: [&str; 11] = [
    "speed_walk",
    "speed_fly",
    "speed_swim",
    "speed_climb",
    "speed_burrow",
    "darkvision",
    "blindsight",
    "tremorsense",
    "truesight",
    "light_bright",
    "light_dim",
];

// ============================================================================
// proficiency_data Validators
// ============================================================================

/// Validates D&D 5e skill and saving throw proficiencies
pub fn validate_proficiency_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "proficiency_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    // Valid skill names (matching system.json)
    let valid_skills = [
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

    // Validate skill_proficiencies (if present)
    if let Some(skills_val) = obj.get("skill_proficiencies") {
        validate_proficiency_set(
            skills_val,
            "proficiency_data.skill_proficiencies",
            &valid_skills,
            "unknown skill name",
        )?;
    }

    // Validate skill_expertise (if present). Same ids as the skills; whether
    // an expert skill is also a proficient one is the rules' business, which
    // simply ignore expertise that proficiency does not back.
    if let Some(expertise_val) = obj.get("skill_expertise") {
        validate_proficiency_set(
            expertise_val,
            "proficiency_data.skill_expertise",
            &valid_skills,
            "unknown skill name",
        )?;
    }

    // Validate saving_throw_proficiencies (if present)
    if let Some(saves_val) = obj.get("saving_throw_proficiencies") {
        let valid_abilities = [
            "strength",
            "dexterity",
            "constitution",
            "intelligence",
            "wisdom",
            "charisma",
        ];
        validate_proficiency_set(
            saves_val,
            "proficiency_data.saving_throw_proficiencies",
            &valid_abilities,
            "invalid ability name",
        )?;
    }

    // Validate proficiency_bonus (if present)
    if let Some(bonus_val) = obj.get("proficiency_bonus") {
        let bonus = bonus_val.as_i64().ok_or(ValidationError {
            field: "proficiency_data.proficiency_bonus".to_string(),
            message: "must be an integer".to_string(),
        })?;

        // Six is a level-twenty character's; a challenge-thirty creature's is nine.
        if !(2..=9).contains(&bonus) {
            return Err(ValidationError {
                field: "proficiency_data.proficiency_bonus".to_string(),
                message: "must be between 2 and 9".to_string(),
            });
        }
    }

    // The plain lists the sheet keeps beside the skills: what armour, weapons
    // and tools a character is trained with.
    for key in ["armor", "weapons", "tools"] {
        if let Some(list) = obj.get(key).filter(|v| !v.is_null()) {
            let items = list.as_array().ok_or(ValidationError {
                field: format!("proficiency_data.{key}"),
                message: "must be an array of strings".to_string(),
            })?;
            if let Some(i) = items.iter().position(|item| !item.is_string()) {
                return Err(ValidationError {
                    field: format!("proficiency_data.{key}[{i}]"),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Validate languages (if present)
    if let Some(langs_val) = obj.get("languages") {
        let _langs = langs_val.as_array().ok_or(ValidationError {
            field: "proficiency_data.languages".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        // All items should be strings
        for (i, lang) in _langs.iter().enumerate() {
            if !lang.is_string() {
                return Err(ValidationError {
                    field: format!("proficiency_data.languages[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    Ok(())
}

/// A set of proficiencies, in either shape a sheet may store it.
///
/// Two shapes are accepted on purpose. The manifest's `data_types` block and
/// the original validator described an object of booleans
/// (`{"stealth": true}`), and that is what the first sheet wrote. The rules
/// (`rules.rs`), the roll-check bindings and the server's own tests read a
/// **list of ids** (`["stealth"]`) — because `declared_values.rs` skips nested
/// objects when it flattens a slot, so a map of booleans never reached
/// `is_proficient` at all. A proficiency saved in the documented shape was
/// silently worth nothing on a roll.
///
/// The list is the shape that works end to end, and is what the pack's own
/// sheet now writes. The map is still accepted so nothing already stored is
/// refused on its next save.
fn validate_proficiency_set(
    value: &serde_json::Value,
    field: &str,
    valid: &[&str],
    unknown_message: &str,
) -> Result<(), ValidationError> {
    if let Some(items) = value.as_array() {
        for (i, item) in items.iter().enumerate() {
            let id = item.as_str().ok_or(ValidationError {
                field: format!("{field}[{i}]"),
                message: "must be a string".to_string(),
            })?;
            if !valid.contains(&id) {
                return Err(ValidationError {
                    field: format!("{field}.{id}"),
                    message: unknown_message.to_string(),
                });
            }
        }
        return Ok(());
    }

    let entries = value.as_object().ok_or(ValidationError {
        field: field.to_string(),
        message: "must be a list of ids or a JSON object of booleans".to_string(),
    })?;
    for (id, proficient) in entries {
        if !valid.contains(&id.as_str()) {
            return Err(ValidationError {
                field: format!("{field}.{id}"),
                message: unknown_message.to_string(),
            });
        }
        if !proficient.is_boolean() {
            return Err(ValidationError {
                field: format!("{field}.{id}"),
                message: "must be a boolean".to_string(),
            });
        }
    }
    Ok(())
}

// ============================================================================
// trait_data Validators
// ============================================================================

/// Spec 084: the facets a sheet may list in `trait_data.facets`.
pub const SHEET_FACETS: &[&str] = &["halfling_luck", "great_weapon_fighting", "lucky"];

/// Validates D&D 5e character class, level, race, and feats
pub fn validate_trait_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "trait_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    // class and level are checked when present, not demanded. A monster has
    // neither, and it still has a size, which lives in this slot and is what
    // the board reads its footprint from: demanding a class would refuse the
    // one field a bestiary creature needs written. The rules already derive
    // nothing proficiency-based for a sheet without a level.
    if let Some(class_val) = obj.get("class") {
        if !class_val.is_string() && !class_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.class".to_string(),
                message: "must be a string".to_string(),
            });
        }
    }

    if let Some(level_val) = obj.get("level").filter(|v| !v.is_null()) {
        let level = level_val.as_i64().ok_or(ValidationError {
            field: "trait_data.level".to_string(),
            message: "must be an integer".to_string(),
        })?;

        if !(1..=20).contains(&level) {
            return Err(ValidationError {
                field: "trait_data.level".to_string(),
                message: "must be between 1 and 20".to_string(),
            });
        }
    }

    // Validate optional fields
    if let Some(subclass_val) = obj.get("subclass") {
        if !subclass_val.is_string() && !subclass_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.subclass".to_string(),
                message: "must be a string or null".to_string(),
            });
        }
    }

    if let Some(race_val) = obj.get("race") {
        if !race_val.is_string() && !race_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.race".to_string(),
                message: "must be a string or null".to_string(),
            });
        }
    }

    if let Some(bg_val) = obj.get("background") {
        if !bg_val.is_string() && !bg_val.is_null() {
            return Err(ValidationError {
                field: "trait_data.background".to_string(),
                message: "must be a string or null".to_string(),
            });
        }
    }

    // Validate feats array (if present)
    if let Some(feats_val) = obj.get("feats") {
        let _feats = feats_val.as_array().ok_or(ValidationError {
            field: "trait_data.feats".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, feat) in _feats.iter().enumerate() {
            if !feat.is_string() {
                return Err(ValidationError {
                    field: format!("trait_data.feats[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Spec 046 FR-030: a creature's size, one of the categories this system
    // declares in `combat.sizes` — read from the manifest, so the list a
    // validator accepts and the list the grid measures by are one list.
    if let Some(size) = obj.get("size") {
        if !size.is_null() {
            let value = size.as_str().ok_or(ValidationError {
                field: "trait_data.size".to_string(),
                message: "must be a string or null".to_string(),
            })?;
            let declared = declared_size_ids();
            if !declared.iter().any(|id| id == value) {
                return Err(ValidationError {
                    field: "trait_data.size".to_string(),
                    message: format!("must be one of {declared:?}"),
                });
            }
        }
    }

    // Spec 046 FR-050: legendary actions per round, declared in
    // `combat.legendary`. A whole number, never negative; absent or null is a
    // creature with none.
    if let Some(legendary) = obj.get("legendary_actions") {
        if !legendary.is_null() {
            let value = legendary.as_i64().ok_or(ValidationError {
                field: "trait_data.legendary_actions".to_string(),
                message: "must be a whole number or null".to_string(),
            })?;
            if value < 0 {
                return Err(ValidationError {
                    field: "trait_data.legendary_actions".to_string(),
                    message: "cannot be negative".to_string(),
                });
            }
        }
    }

    // A creature's challenge rating. It stands where a character's level
    // does: the rules read the proficiency bonus from it when there is no
    // level (`rules.rs`).
    if let Some(challenge) = obj.get("challenge").filter(|v| !v.is_null()) {
        let value = challenge.as_str().ok_or(ValidationError {
            field: "trait_data.challenge".to_string(),
            message: "must be a string or null".to_string(),
        })?;
        if !CHALLENGE_RATINGS.contains(&value) {
            return Err(ValidationError {
                field: "trait_data.challenge".to_string(),
                message: "must be a challenge rating: 0, 1/8, 1/4, 1/2, or 1 to 30".to_string(),
            });
        }
    }

    // Speeds, senses and a carried light, in feet. The board reads the walk
    // speed, darkvision and the light (`movement` and `vision` in the
    // manifest); the rest are the sheet's.
    for key in TRAIT_DISTANCES {
        optional_distance(obj, "trait_data", key)?;
    }

    optional_whole(obj, "trait_data", "experience", 0, i64::MAX)?;
    for key in ["alignment", "creature_type", "notes"] {
        optional_text(obj, "trait_data", key)?;
    }
    if let Some(inspiration) = obj.get("inspiration").filter(|v| !v.is_null()) {
        if !inspiration.is_boolean() {
            return Err(ValidationError {
                field: "trait_data.inspiration".to_string(),
                message: "must be true or false".to_string(),
            });
        }
    }

    // Spec 084: the facets a character has, each known and listed once, and
    // how many Luck Points it has spent since its last long rest.
    if let Some(facets) = obj.get("facets").filter(|v| !v.is_null()) {
        let refuse = |message: String| ValidationError {
            field: "trait_data.facets".to_string(),
            message,
        };
        let ids = facets
            .as_array()
            .ok_or_else(|| refuse("must be an array of facet ids".to_string()))?;
        let mut seen = Vec::new();
        for id in ids {
            let id = id
                .as_str()
                .filter(|id| SHEET_FACETS.contains(id))
                .ok_or_else(|| refuse(format!("must each be one of {SHEET_FACETS:?}")))?;
            if seen.contains(&id) {
                return Err(refuse(format!("lists '{id}' twice")));
            }
            seen.push(id);
        }
    }
    optional_whole(obj, "trait_data", "luck_points_used", 0, i64::MAX)?;

    // Validate traits array (if present)
    if let Some(traits_val) = obj.get("traits") {
        let _traits = traits_val.as_array().ok_or(ValidationError {
            field: "trait_data.traits".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, trait_) in _traits.iter().enumerate() {
            if !trait_.is_string() {
                return Err(ValidationError {
                    field: format!("trait_data.traits[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    crate::validators_sheet::trait_fields(obj)?;
    Ok(())
}

// ============================================================================
// spell_data Validators
// ============================================================================

/// Validates D&D 5e spellcasting data
pub fn validate_spell_data(data: &serde_json::Value) -> Result<(), ValidationError> {
    let obj = data.as_object().ok_or(ValidationError {
        field: "spell_data".to_string(),
        message: "must be a JSON object".to_string(),
    })?;

    // If any spell data is present, spellcasting_ability should be valid
    if let Some(ability_val) = obj.get("spellcasting_ability") {
        let ability = ability_val.as_str().ok_or(ValidationError {
            field: "spell_data.spellcasting_ability".to_string(),
            message: "must be a string".to_string(),
        })?;

        let valid_abilities = [
            "strength",
            "dexterity",
            "constitution",
            "intelligence",
            "wisdom",
            "charisma",
        ];
        if !valid_abilities.contains(&ability) {
            return Err(ValidationError {
                field: "spell_data.spellcasting_ability".to_string(),
                message: "must be one of the six ability scores".to_string(),
            });
        }
    }

    // Validate spell_save_dc (if present)
    if let Some(dc_val) = obj.get("spell_save_dc") {
        let dc = dc_val.as_i64().ok_or(ValidationError {
            field: "spell_data.spell_save_dc".to_string(),
            message: "must be an integer".to_string(),
        })?;

        // Eight is the floor the formula has; thirty leaves room for a
        // creature whose casting ability is past a character's cap.
        if !(8..=30).contains(&dc) {
            return Err(ValidationError {
                field: "spell_data.spell_save_dc".to_string(),
                message: "must be between 8 and 30".to_string(),
            });
        }
    }

    // Validate spell_attack_bonus (if present)
    if let Some(bonus_val) = obj.get("spell_attack_bonus") {
        if !bonus_val.is_i64() {
            return Err(ValidationError {
                field: "spell_data.spell_attack_bonus".to_string(),
                message: "must be an integer".to_string(),
            });
        }
    }

    // Validate cantrips_known array (if present)
    if let Some(cantrips_val) = obj.get("cantrips_known") {
        let _cantrips = cantrips_val.as_array().ok_or(ValidationError {
            field: "spell_data.cantrips_known".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, cantrip) in _cantrips.iter().enumerate() {
            if !cantrip.is_string() {
                return Err(ValidationError {
                    field: format!("spell_data.cantrips_known[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Validate spells_known array (if present)
    if let Some(spells_val) = obj.get("spells_known") {
        let _spells = spells_val.as_array().ok_or(ValidationError {
            field: "spell_data.spells_known".to_string(),
            message: "must be an array of strings".to_string(),
        })?;

        for (i, spell) in _spells.iter().enumerate() {
            if !spell.is_string() {
                return Err(ValidationError {
                    field: format!("spell_data.spells_known[{}]", i),
                    message: "must be a string".to_string(),
                });
            }
        }
    }

    // Slots per day, and how many of them are spent. The same nine levels and
    // the same rule for both: a whole number, never negative.
    for key in ["spell_slots", "spell_slots_used"] {
        let Some(slots_val) = obj.get(key).filter(|v| !v.is_null()) else {
            continue;
        };
        let slots = slots_val.as_object().ok_or(ValidationError {
            field: format!("spell_data.{key}"),
            message: "must be a JSON object".to_string(),
        })?;

        let valid_levels = [
            "level_1", "level_2", "level_3", "level_4", "level_5", "level_6", "level_7", "level_8",
            "level_9",
        ];

        for (level_key, slot_count_val) in slots {
            if !valid_levels.contains(&level_key.as_str()) {
                return Err(ValidationError {
                    field: format!("spell_data.{key}.{level_key}"),
                    message: "invalid spell level (must be level_1 through level_9)".to_string(),
                });
            }

            let slot_count = slot_count_val.as_i64().ok_or(ValidationError {
                field: format!("spell_data.{key}.{level_key}"),
                message: "must be an integer".to_string(),
            })?;

            if slot_count < 0 {
                return Err(ValidationError {
                    field: format!("spell_data.{key}.{level_key}"),
                    message: "cannot be negative".to_string(),
                });
            }
        }
    }

    crate::validators_sheet::spell_fields(obj)?;
    Ok(())
}

// ============================================================================
// Registry Adapters: Convert ValidationError -> String
// ============================================================================
// These functions wrap the validators to return Result<(), String> for use
// in the generic system registry (crates/thunderforge-server/src/systems/mod.rs)

/// Adapter: validate_ability_data for registry
pub fn validate_ability_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_ability_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_resource_data for registry
pub fn validate_resource_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_resource_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_proficiency_data for registry
pub fn validate_proficiency_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_proficiency_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_trait_data for registry
pub fn validate_trait_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_trait_data(data).map_err(|e| e.to_string())
}

/// Adapter: validate_spell_data for registry
pub fn validate_spell_data_for_registry(data: &serde_json::Value) -> Result<(), String> {
    validate_spell_data(data).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "validators_tests.rs"]
mod tests;
