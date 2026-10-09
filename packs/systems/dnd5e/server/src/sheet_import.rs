//! Bringing a D&D Beyond character in (spec 048, contracts/sheet-mapping-5e.md).
//!
//! The declaration under `sheetImport` in `system.json` says where each read
//! value lands. This module is the rest:
//!
//! * the slot the host runs: the `ddb-pdf` reader over bytes, with each
//!   refusal named by its error code;
//! * the refine hook, for what a declaration cannot say: a level that is the
//!   sum of the classes, hit dice folded into the sheet's string, skill marks
//!   turned into proficiency and expertise, the pack's own ability names, and
//!   an attack row that is an item's, a spell's, or an ability of its own.
//!
//! The hook writes through [`ImportPlan::set_field`], so a value the actor
//! already holds is identical rather than a change, exactly as the planner
//! decides for a declared field.

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use thunderforge_canvas_core::sheet_import::{SheetImport, SheetReadFailure, SheetReaderHandle};
use thunderforge_pdf::{Document, Limits, PdfError};
use thunderforge_sheet_import::plan::Unmapped;
use thunderforge_sheet_import::{
    ActorSnapshot, ContentTarget, FieldChange, ImportPlan, ImportedCharacter, ReadError,
    SheetReader,
};
use thunderforge_system_dnd5e_sheet::DdbPdf;

/// What this pack contributes to bringing a character in.
pub static SHEET_IMPORT: SheetImport = SheetImport {
    readers: &[&DDB_PDF],
    refine: Some(refine_json),
};

/// Where a value with no field of its own goes: the declaration's `notes`.
const NOTES: &str = "trait_data.notes";

/// The sheet's short ability names and this pack's.
const ABILITIES: [(&str, &str); 6] = [
    ("str", "strength"),
    ("dex", "dexterity"),
    ("con", "constitution"),
    ("int", "intelligence"),
    ("wis", "wisdom"),
    ("cha", "charisma"),
];

fn ability_name(short: &str) -> Option<&'static str> {
    ABILITIES
        .iter()
        .find(|(s, full)| *s == short || *full == short)
        .map(|(_, full)| *full)
}

struct DdbPdfHandle;

static DDB_PDF: DdbPdfHandle = DdbPdfHandle;

impl SheetReaderHandle for DdbPdfHandle {
    fn id(&self) -> &'static str {
        DdbPdf.id()
    }

    fn version(&self) -> &'static str {
        DdbPdf.version()
    }

    fn read(&self, bytes: &[u8]) -> Result<String, SheetReadFailure> {
        let doc = Document::from_bytes_bounded(bytes, Limits::default()).map_err(pdf_failure)?;
        let reading = DdbPdf.read(&doc).map_err(|error| match error {
            ReadError::Pdf(error) => pdf_failure(error),
            ReadError::NotRecognised(reason) => failure("SHEET_NOT_RECOGNISED", reason),
            error @ ReadError::Page { .. } => failure("SHEET_UNREADABLE", error.to_string()),
        })?;
        serde_json::to_string(&reading).map_err(|e| failure("SHEET_UNREADABLE", e.to_string()))
    }
}

fn failure(code: &'static str, message: impl Into<String>) -> SheetReadFailure {
    SheetReadFailure {
        code,
        message: message.into(),
    }
}

fn pdf_failure(error: PdfError) -> SheetReadFailure {
    let code = match &error {
        PdfError::Encrypted => "SHEET_ENCRYPTED",
        PdfError::TooLarge { .. } => "SHEET_TOO_LARGE",
        PdfError::TooManyPages { .. } => "SHEET_TOO_MANY_PAGES",
        PdfError::Unreadable(_) | PdfError::Page { .. } => "SHEET_UNREADABLE",
    };
    failure(code, error.to_string())
}

/// The hook as the host's slot holds it: over JSON. A value that does not
/// parse leaves the plan as it is.
pub fn refine_json(reading: &Value, current: &Value, plan: &mut Value) {
    let parsed = (
        serde_json::from_value::<ImportedCharacter>(reading.clone()),
        serde_json::from_value::<ActorSnapshot>(current.clone()),
        serde_json::from_value::<ImportPlan>(plan.clone()),
    );
    if let (Ok(reading), Ok(current), Ok(mut refined)) = parsed {
        refine(&reading, &current, &mut refined);
        if let Ok(value) = serde_json::to_value(&refined) {
            *plan = value;
        }
    }
}

/// 5e's last word on a plan (contracts/sheet-mapping-5e.md, "Refine hook
/// rules").
pub fn refine(reading: &ImportedCharacter, current: &ActorSnapshot, plan: &mut ImportPlan) {
    size(current, plan);
    classes(current, plan);
    hit_dice(current, plan);
    skills(current, plan);
    saves(current, plan);
    spellcasting(current, plan);
    spell_slots(current, plan);
    pact_slots(current, plan);
    attacks(plan);
    cross_checks(reading, plan);
}

/// A change to `target` that comes from the same part of the sheet as `from`
/// and is as sure as it.
fn alongside(from: &FieldChange, target: &str, new: Value) -> FieldChange {
    FieldChange {
        path: from.path.clone(),
        target: target.to_string(),
        old: None,
        new: Some(new),
        certainty: from.certainty,
        reason: from.reason.clone(),
        source: from.source.clone(),
        play_state: false,
    }
}

fn current_object(current: &ActorSnapshot, target: &str) -> Map<String, Value> {
    current
        .values
        .get(target)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

fn note(plan: &mut ImportPlan, path: String, label: String, value: Value) {
    plan.unmapped.push(Unmapped {
        path,
        label,
        value,
        goes_to: NOTES.to_string(),
    });
}

/// The sheet prints "Medium"; the pack's sizes are lower case. A size the
/// pack does not declare is kept in the notes rather than refused.
fn size(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let Some(mut change) = plan.take_field("trait_data.size") else {
        return;
    };
    if let Some(Value::String(printed)) = change.new.clone() {
        let id = printed.trim().to_lowercase();
        if !crate::validators::declared_size_ids().contains(&id) {
            note(
                plan,
                change.path.clone(),
                "Size".to_string(),
                json!(printed),
            );
            return;
        }
        change.new = Some(json!(id));
    }
    plan.set_field(current, change);
}

/// Rule 1: `level` is the sum of the classes, and `class` and `subclass` are
/// the first class's. Each class's hit die is the pack's "d10".
fn classes(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let Some(mut change) = plan.take_field("trait_data.classes") else {
        return;
    };
    if let Some(Value::Array(items)) = change.new.as_mut() {
        for item in items.iter_mut() {
            if let Some(sides) = item.get("hit_die").and_then(Value::as_i64) {
                item["hit_die"] = json!(format!("d{sides}"));
            }
        }
    }
    let items = change.new.clone();
    plan.set_field(current, change.clone());
    let Some(Value::Array(items)) = items else {
        return;
    };
    let levels: Option<Vec<i64>> = items
        .iter()
        .map(|item| item.get("level").and_then(Value::as_i64))
        .collect();
    if let Some(levels) = levels.filter(|levels| !levels.is_empty()) {
        let level = json!(levels.iter().sum::<i64>());
        plan.set_field(current, alongside(&change, "trait_data.level", level));
    }
    let Some(first) = items.first() else {
        return;
    };
    for (key, target) in [
        ("name", "trait_data.class"),
        ("subclass", "trait_data.subclass"),
    ] {
        if let Some(value) = first.get(key).filter(|v| v.is_string()) {
            plan.set_field(current, alongside(&change, target, value.clone()));
        }
    }
}

/// Rule 2: the pools take the pack's die names, used dice are the table's,
/// and the pools fold into the sheet's string, largest die first.
fn hit_dice(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let target = "resource_data.hit_dice_pools";
    let Some(mut change) = plan.take_field(target) else {
        return;
    };
    let before: Vec<Value> = current
        .values
        .get(target)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let used_before = |die: &str| {
        before
            .iter()
            .find(|pool| pool["die"] == die)
            .and_then(|pool| pool["used"].as_i64())
    };
    if let Some(Value::Array(pools)) = change.new.as_mut() {
        for pool in pools.iter_mut() {
            if let Some(sides) = pool.get("die").and_then(Value::as_i64) {
                pool["die"] = json!(format!("d{sides}"));
            }
            let die = pool["die"].as_str().unwrap_or_default().to_string();
            let printed = pool.get("used").and_then(Value::as_i64);
            // Spent dice are play state: a re-import keeps the table's.
            let used = match (current.is_reimport, used_before(&die), printed) {
                (true, Some(table), _) => table,
                (_, _, Some(printed)) => printed,
                (_, table, None) => table.unwrap_or(0),
            };
            pool["used"] = json!(used);
        }
    }
    let pools = change.new.clone();
    plan.set_field(current, change.clone());
    let Some(Value::Array(pools)) = pools else {
        return;
    };
    let mut dice: Vec<(i64, i64, String)> = pools
        .iter()
        .filter_map(|pool| {
            let die = pool["die"].as_str()?;
            let sides = die.strip_prefix('d')?.parse().ok()?;
            Some((sides, pool["total"].as_i64()?, die.to_string()))
        })
        .collect();
    dice.sort_by_key(|die| std::cmp::Reverse(die.0));
    let folded: Vec<String> = dice
        .iter()
        .map(|(_, total, die)| format!("{total}{die}"))
        .collect();
    let used: i64 = pools.iter().filter_map(|p| p["used"].as_i64()).sum();
    plan.set_field(
        current,
        alongside(&change, "resource_data.hit_dice", json!(folded.join(" + "))),
    );
    plan.set_field(
        current,
        alongside(&change, "resource_data.hit_dice_used", json!(used)),
    );
}

/// A mark becomes proficiency, and expertise its own list. Half proficiency
/// (Jack of All Trades) is not held: it is a note, and the feature that
/// grants it arrives as content. A mark the reader could not tell keeps what
/// the actor has, and invents nothing.
fn skills(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let target = "proficiency_data.skill_proficiencies";
    let Some(mut change) = plan.take_field(target) else {
        return;
    };
    let Some(Value::Object(marks)) = change.new.clone() else {
        plan.set_field(current, change);
        return;
    };
    let before = current_object(current, target);
    let expert_before: BTreeSet<String> = current
        .values
        .get("proficiency_data.skill_expertise")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    let mut skills = Map::new();
    let mut expertise = BTreeSet::new();
    for (skill, mark) in marks {
        match mark.as_str() {
            Some("none") => {
                skills.insert(skill, json!(false));
            }
            Some("half") => {
                let label = format!("Half proficiency: {}", skill.replace('_', " "));
                note(
                    plan,
                    format!("{}.{skill}", change.path),
                    label,
                    json!("half"),
                );
                skills.insert(skill, json!(false));
            }
            Some("proficient") => {
                skills.insert(skill, json!(true));
            }
            Some("expertise") => {
                skills.insert(skill.clone(), json!(true));
                expertise.insert(skill);
            }
            _ => {
                if let Some(held) = before.get(&skill) {
                    skills.insert(skill.clone(), held.clone());
                }
                if expert_before.contains(&skill) {
                    expertise.insert(skill);
                }
            }
        }
    }
    change.new = Some(Value::Object(skills));
    plan.set_field(current, change.clone());
    let expertise: Vec<String> = expertise.into_iter().collect();
    plan.set_field(
        current,
        alongside(
            &change,
            "proficiency_data.skill_expertise",
            json!(expertise),
        ),
    );
}

/// Saves under the pack's ability names. A mark the reader could not tell
/// keeps what the actor has.
fn saves(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let Some(mut change) = plan.take_field("proficiency_data.saving_throw_proficiencies") else {
        return;
    };
    if let Some(Value::Object(marks)) = change.new.as_ref() {
        let mut saves: Map<String, Value> = marks
            .iter()
            .filter(|(key, _)| ABILITIES.iter().any(|(_, full)| full == key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        for (short, full) in ABILITIES {
            if let Some(mark) = marks.get(short).filter(|v| v.is_boolean()) {
                saves.insert(full.to_string(), mark.clone());
            }
        }
        change.new = Some(Value::Object(saves));
    }
    plan.set_field(current, change);
}

/// One entry per casting class, and the first is the actor's own.
fn spellcasting(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let Some(mut change) = plan.take_field("spell_data.spellcasting_classes") else {
        return;
    };
    if let Some(Value::Array(items)) = change.new.as_mut() {
        for item in items.iter_mut() {
            let mut kept = Map::new();
            for key in ["class", "ability", "save_dc", "attack_bonus"] {
                if let Some(value) = item.get(key).filter(|v| !v.is_null()) {
                    kept.insert(key.to_string(), value.clone());
                }
            }
            if let Some(full) = kept
                .get("ability")
                .and_then(Value::as_str)
                .and_then(ability_name)
            {
                kept.insert("ability".to_string(), json!(full));
            }
            *item = Value::Object(kept);
        }
    }
    let first = change.new.as_ref().and_then(|items| items.get(0)).cloned();
    plan.set_field(current, change.clone());
    let Some(first) = first else {
        return;
    };
    for (key, target) in [
        ("ability", "spell_data.spellcasting_ability"),
        ("save_dc", "spell_data.spell_save_dc"),
        ("attack_bonus", "spell_data.spell_attack_bonus"),
    ] {
        if let Some(value) = first.get(key) {
            plan.set_field(current, alongside(&change, target, value.clone()));
        }
    }
}

/// The sheet's slots by level ("1") become the pack's ("level_1"). The sheet
/// lists every level it has, so the sheet's slots are the whole of them.
fn spell_slots(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let target = "spell_data.spell_slots";
    let Some(mut change) = plan.take_field(target) else {
        return;
    };
    // The planner composes the numbered leaves as a list, with the level as
    // the index; a reading built by hand may still be an object.
    let printed: Option<Vec<(String, Value)>> = match change.new.as_ref() {
        Some(Value::Object(printed)) => Some(
            printed
                .iter()
                .map(|(level, count)| (level.clone(), count.clone()))
                .collect(),
        ),
        Some(Value::Array(printed)) => Some(
            printed
                .iter()
                .enumerate()
                .skip(1)
                .map(|(level, count)| (level.to_string(), count.clone()))
                .collect(),
        ),
        _ => None,
    };
    if let Some(printed) = printed {
        let before = current_object(current, target);
        let mut slots = Map::new();
        for (level, count) in printed {
            if level.parse::<u8>().is_err() {
                continue;
            }
            let key = format!("level_{level}");
            match (count.is_null(), before.get(&key)) {
                (false, _) => {
                    slots.insert(key, count);
                }
                (true, Some(held)) => {
                    slots.insert(key, held.clone());
                }
                (true, None) => {}
            }
        }
        change.new = Some(Value::Object(slots));
    }
    plan.set_field(current, change);
}

/// A Warlock's slots; how many are spent is the table's.
fn pact_slots(current: &ActorSnapshot, plan: &mut ImportPlan) {
    let target = "spell_data.pact_slots";
    let Some(mut change) = plan.take_field(target) else {
        return;
    };
    if let Some(Value::Object(slots)) = change.new.as_mut() {
        let used = current_object(current, target)
            .get("used")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        slots.insert("used".to_string(), json!(used));
    }
    plan.set_field(current, change);
}

/// Rule 3: an attack row rides on the equipment item of the same name, or
/// else on another piece of content of that name (a cantrip's row is the
/// spell's). With nothing to ride on it is an ability of its own. Either way
/// it carries spec 046's attack: the effects the attack flow rolls, and its
/// reach or ranges.
fn attacks(plan: &mut ImportPlan) {
    let (rows, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut plan.content)
        .into_iter()
        .partition(|change| change.kind == "attack" && !change.removed);
    plan.content = rest;
    for mut row in rows {
        let named = |kind: Option<&str>| {
            plan.content.iter().position(|c| {
                !c.removed
                    && c.normalised == row.normalised
                    && kind.map_or(c.kind != "attack", |kind| c.kind == kind)
            })
        };
        let host = named(Some("item")).or_else(|| named(None));
        let range = host
            .and_then(|i| plan.content[i].fields.get("range"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let attack = attack(&row.fields, range.as_deref());
        match host {
            Some(index) => {
                let host = &mut plan.content[index];
                set_attack(&mut host.fields, attack);
                host.from.extend(row.from);
            }
            None => {
                row.target = ContentTarget::Ability {
                    vocabulary: "feature".to_string(),
                };
                set_attack(&mut row.fields, attack);
                plan.content.push(row);
            }
        }
    }
}

fn set_attack(fields: &mut Value, attack: Value) {
    if !fields.is_object() {
        *fields = Value::Object(Map::new());
    }
    fields["attack"] = attack;
}

/// An attack row as spec 046 holds an attack, the way `stat_blocks.rs` maps
/// a monster's: the printed to-hit as an `ATTACK_ROLL` effect, the printed
/// damage as a `DAMAGE` effect, and reach 5 unless the row (or the spell it
/// rides on) gives a range. The printed numbers are kept beside them.
fn attack(row: &Value, spell_range: Option<&str>) -> Value {
    let to_hit = row.get("to_hit").and_then(Value::as_i64);
    let damage = row.get("damage").and_then(Value::as_str);
    let notes = row.get("notes").and_then(Value::as_str).unwrap_or_default();

    let mut effects = Vec::new();
    if let Some(bonus) = to_hit {
        effects.push(json!({ "effect_type": "ATTACK_ROLL", "formula": format!("1d20{bonus:+}") }));
    }
    if let Some(damage) = damage {
        effects.push(json!({ "effect_type": "DAMAGE", "formula": damage }));
    }

    let (reach, ranges) = if let Some(ranges) = parenthesised_range(notes, "Thrown") {
        (Some(5), Some(ranges))
    } else if let Some(ranges) = parenthesised_range(notes, "Range") {
        (None, Some(ranges))
    } else if let Some(feet) = spell_range.and_then(feet) {
        (None, Some((feet, feet)))
    } else {
        (Some(5), None)
    };

    json!({
        "to_hit": to_hit,
        "damage": damage,
        "damage_type": row.get("damage_type"),
        "notes": (!notes.is_empty()).then_some(notes),
        "effects": effects,
        "reach": reach,
        "range_normal": ranges.map(|r| r.0),
        "range_long": ranges.map(|r| r.1),
        "save_ability": row.get("save_ability").and_then(Value::as_str).and_then(ability_name),
        "save_dc": row.get("save_dc"),
    })
}

/// "Range (80/320)" in a row's notes.
fn parenthesised_range(notes: &str, word: &str) -> Option<(i64, i64)> {
    let after = notes.split(&format!("{word} (")).nth(1)?;
    let (inside, _) = after.split_once(')')?;
    let (normal, long) = inside.split_once('/')?;
    Some((normal.trim().parse().ok()?, long.trim().parse().ok()?))
}

/// A spell's "120 ft.".
fn feet(range: &str) -> Option<i64> {
    range.strip_suffix(" ft.")?.trim().parse().ok()
}

/// What the sheet printed that the rules derive. A modifier is checked
/// against its score, and the proficiency bonus against the level; skills,
/// saves and passives are not, because a feat or an item moves them and the
/// rules here would cry wolf.
fn cross_checks(reading: &ImportedCharacter, plan: &mut ImportPlan) {
    for (short, _) in ABILITIES {
        let printed = reading
            .derived
            .get(&format!("modifier.{short}"))
            .and_then(|f| f.value);
        let score = reading.abilities.get(short).and_then(|f| f.value);
        if let (Some(printed), Some(score)) = (printed, score) {
            let base = format!("abilities.{short}");
            plan.cross_check(
                &format!("derived.modifier.{short}"),
                json!(printed),
                json!(crate::rules::ability_modifier(score)),
                &[base.as_str()],
            );
        }
    }
    let levels: Option<Vec<i32>> = reading.classes.iter().map(|c| c.level.value).collect();
    let level = levels
        .filter(|levels| !levels.is_empty())
        .map(|levels| levels.iter().sum::<i32>());
    let printed = reading
        .derived
        .get("proficiency_bonus")
        .and_then(|f| f.value);
    if let (Some(printed), Some(bonus)) = (printed, level.and_then(crate::rules::proficiency_bonus))
    {
        plan.cross_check(
            "derived.proficiency_bonus",
            json!(printed),
            json!(bonus),
            &["classes"],
        );
    }
}

#[cfg(test)]
#[path = "sheet_import_tests.rs"]
mod tests;
