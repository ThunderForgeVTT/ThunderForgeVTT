//! Spec 048: the 5e fields a whole character sheet adds (data-model.md,
//! "5e pack: fields added"). Kept beside `validators.rs`, which calls these
//! from the slot validators.
//!
//! Every field is optional; absent or null is a sheet that does not record
//! it. The damage types and conditions are read from the manifest, so the
//! list a validator accepts is the list the pack declares.

use std::sync::OnceLock;

use serde_json::{Map, Value};

use crate::validators::ValidationError;

const HIT_DICE: [&str; 4] = ["d6", "d8", "d10", "d12"];
const ABILITIES: [&str; 6] = [
    "strength",
    "dexterity",
    "constitution",
    "intelligence",
    "wisdom",
    "charisma",
];
const PERSONA_SHORT: [&str; 8] = [
    "age", "height", "weight", "eyes", "skin", "hair", "gender", "faith",
];
const PERSONA_LONG: [&str; 6] = [
    "personality_traits",
    "ideals",
    "bonds",
    "flaws",
    "backstory",
    "allies_and_organizations",
];
const DEFENCES: [&str; 3] = ["resistances", "immunities", "vulnerabilities"];
const COINS: [&str; 5] = ["cp", "sp", "ep", "gp", "pp"];

fn refuse(field: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError {
        field: field.into(),
        message: message.into(),
    }
}

fn manifest_ids(pointer: &str) -> Vec<String> {
    serde_json::from_str::<Value>(include_str!("../../system.json"))
        .ok()
        .and_then(|manifest| {
            manifest
                .pointer(pointer)
                .and_then(Value::as_array)
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|e| e.as_str().or_else(|| e["id"].as_str()))
                        .map(str::to_string)
                        .collect()
                })
        })
        .unwrap_or_default()
}

/// The damage types this pack declares under `damageTypes`.
pub fn damage_type_ids() -> &'static [String] {
    static IDS: OnceLock<Vec<String>> = OnceLock::new();
    IDS.get_or_init(|| manifest_ids("/damageTypes"))
}

/// The condition ids this pack declares under `conditions`.
pub fn condition_ids() -> &'static [String] {
    static IDS: OnceLock<Vec<String>> = OnceLock::new();
    IDS.get_or_init(|| manifest_ids("/conditions"))
}

fn present<'a>(obj: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    obj.get(key).filter(|v| !v.is_null())
}

fn array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>, ValidationError> {
    value
        .as_array()
        .ok_or_else(|| refuse(field, "must be a list"))
}

fn object<'a>(value: &'a Value, field: &str) -> Result<&'a Map<String, Value>, ValidationError> {
    value
        .as_object()
        .ok_or_else(|| refuse(field, "must be an object"))
}

fn only_keys(
    obj: &Map<String, Value>,
    field: &str,
    allowed: &[&str],
) -> Result<(), ValidationError> {
    match obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(key) => Err(refuse(field, format!("has no field '{key}'"))),
        None => Ok(()),
    }
}

fn whole(value: Option<&Value>, field: &str, min: i64, max: i64) -> Result<i64, ValidationError> {
    let number = value
        .and_then(Value::as_i64)
        .ok_or_else(|| refuse(field, "must be a whole number"))?;
    if number < min {
        return Err(refuse(
            field,
            if min == 0 {
                "cannot be negative".to_string()
            } else {
                format!("must be at least {min}")
            },
        ));
    }
    if number > max {
        return Err(refuse(field, format!("must be at most {max}")));
    }
    Ok(number)
}

fn text(value: Option<&Value>, field: &str) -> Result<String, ValidationError> {
    value
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| refuse(field, "must be a string"))
}

fn one_of(value: Option<&Value>, field: &str, allowed: &[&str]) -> Result<(), ValidationError> {
    let value = text(value, field)?;
    if allowed.contains(&value.as_str()) {
        Ok(())
    } else {
        Err(refuse(field, format!("must be one of {allowed:?}")))
    }
}

pub(crate) fn trait_fields(obj: &Map<String, Value>) -> Result<(), ValidationError> {
    if let Some(classes) = present(obj, "classes") {
        let classes = array(classes, "trait_data.classes")?;
        let mut total = 0;
        for (index, entry) in classes.iter().enumerate() {
            let field = format!("trait_data.classes[{index}]");
            let entry = object(entry, &field)?;
            only_keys(entry, &field, &["name", "subclass", "level", "hit_die"])?;
            let name = text(entry.get("name"), &format!("{field}.name"))?;
            if name.trim().is_empty() {
                return Err(refuse(format!("{field}.name"), "cannot be empty"));
            }
            if present(entry, "subclass").is_some_and(|s| !s.is_string()) {
                return Err(refuse(format!("{field}.subclass"), "must be a string"));
            }
            total += whole(entry.get("level"), &format!("{field}.level"), 1, 20)?;
            one_of(entry.get("hit_die"), &format!("{field}.hit_die"), &HIT_DICE)?;
        }
        if total > 20 {
            return Err(refuse(
                "trait_data.classes",
                "levels add up to more than 20",
            ));
        }
        if !classes.is_empty() {
            if present(obj, "level")
                .and_then(Value::as_i64)
                .is_some_and(|level| level != total)
            {
                return Err(refuse(
                    "trait_data.level",
                    format!("must be the sum of the class levels, {total}"),
                ));
            }
            let first = classes[0]["name"].as_str().unwrap_or_default();
            if present(obj, "class")
                .and_then(Value::as_str)
                .is_some_and(|class| class != first)
            {
                return Err(refuse(
                    "trait_data.class",
                    format!("must be the first class listed, '{first}'"),
                ));
            }
        }
    }

    for (keys, limit) in [(&PERSONA_SHORT[..], 100), (&PERSONA_LONG[..], 8000)] {
        for key in keys {
            if let Some(value) = present(obj, key) {
                let field = format!("trait_data.{key}");
                let value = text(Some(value), &field)?;
                if value.chars().count() > limit {
                    return Err(refuse(field, format!("must be at most {limit} characters")));
                }
            }
        }
    }

    let damage: Vec<&str> = damage_type_ids().iter().map(String::as_str).collect();
    for key in DEFENCES {
        list_of(obj, key, &damage)?;
    }
    let conditions: Vec<&str> = condition_ids().iter().map(String::as_str).collect();
    list_of(obj, "condition_immunities", &conditions)?;
    Ok(())
}

fn list_of(obj: &Map<String, Value>, key: &str, allowed: &[&str]) -> Result<(), ValidationError> {
    let Some(value) = present(obj, key) else {
        return Ok(());
    };
    for (index, entry) in array(value, &format!("trait_data.{key}"))?
        .iter()
        .enumerate()
    {
        one_of(Some(entry), &format!("trait_data.{key}[{index}]"), allowed)?;
    }
    Ok(())
}

pub(crate) fn resource_fields(obj: &Map<String, Value>) -> Result<(), ValidationError> {
    if let Some(pools) = present(obj, "hit_dice_pools") {
        for (index, pool) in array(pools, "resource_data.hit_dice_pools")?
            .iter()
            .enumerate()
        {
            let field = format!("resource_data.hit_dice_pools[{index}]");
            let pool = object(pool, &field)?;
            only_keys(pool, &field, &["die", "total", "used"])?;
            one_of(pool.get("die"), &format!("{field}.die"), &HIT_DICE)?;
            let total = whole(pool.get("total"), &format!("{field}.total"), 0, 20)?;
            whole(pool.get("used"), &format!("{field}.used"), 0, total)?;
        }
    }
    if let Some(coins) = present(obj, "coins") {
        let coins = object(coins, "resource_data.coins")?;
        for (key, value) in coins {
            let field = format!("resource_data.coins.{key}");
            if !COINS.contains(&key.as_str()) {
                return Err(refuse(field, format!("must be one of {COINS:?}")));
            }
            if !value.is_null() {
                whole(Some(value), &field, 0, i64::MAX)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn spell_fields(obj: &Map<String, Value>) -> Result<(), ValidationError> {
    if let Some(classes) = present(obj, "spellcasting_classes") {
        for (index, entry) in array(classes, "spell_data.spellcasting_classes")?
            .iter()
            .enumerate()
        {
            let field = format!("spell_data.spellcasting_classes[{index}]");
            let entry = object(entry, &field)?;
            only_keys(
                entry,
                &field,
                &["class", "ability", "save_dc", "attack_bonus"],
            )?;
            text(entry.get("class"), &format!("{field}.class"))?;
            one_of(
                entry.get("ability"),
                &format!("{field}.ability"),
                &ABILITIES,
            )?;
            // The same bounds the single spell_save_dc has.
            whole(entry.get("save_dc"), &format!("{field}.save_dc"), 8, 30)?;
            whole(
                entry.get("attack_bonus"),
                &format!("{field}.attack_bonus"),
                -10,
                30,
            )?;
        }
    }
    if let Some(pact) = present(obj, "pact_slots") {
        let pact = object(pact, "spell_data.pact_slots")?;
        only_keys(pact, "spell_data.pact_slots", &["level", "total", "used"])?;
        whole(pact.get("level"), "spell_data.pact_slots.level", 1, 5)?;
        let total = whole(pact.get("total"), "spell_data.pact_slots.total", 0, 4)?;
        whole(pact.get("used"), "spell_data.pact_slots.used", 0, total)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "validators_sheet_tests.rs"]
mod tests;
