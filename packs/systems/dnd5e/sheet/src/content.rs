//! The sheet's rows: spells, features and traits, equipment and attacks
//! (spec 048 T031).

use std::collections::BTreeMap;

use serde_json::{Value, json};
use thunderforge_sheet_import::{ContentLink, Field, ImportedCharacter, NamedContent};

use crate::fields::{DAMAGE_TYPES, Entry, Sheet, parse_int};

/// "=== CANTRIPS ===" is 0, "=== 3rd LEVEL ===" is 3.
pub fn spell_level(header: &str) -> Option<i32> {
    let upper = header.to_uppercase();
    if upper.contains("CANTRIP") {
        return Some(0);
    }
    if !upper.contains("LEVEL") {
        return None;
    }
    let digits: String = upper
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

pub fn read_content(sheet: &Sheet, c: &ImportedCharacter) -> Vec<NamedContent> {
    let mut out = features(sheet, c);
    out.extend(spells(sheet));
    out.extend(equipment(sheet));
    out.extend(attacks(sheet));
    out
}

fn item(kind: &str, name: Field<String>) -> NamedContent {
    NamedContent {
        kind: kind.into(),
        name,
        fields: BTreeMap::new(),
        link: ContentLink::default(),
    }
}

fn put(fields: &mut BTreeMap<String, Value>, key: &str, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        fields.insert(key.into(), json!(text));
    }
}

/// "FIGHTER" as "Fighter", "HILL DWARF" as "Hill Dwarf".
fn title(upper: &str) -> String {
    upper
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first
                    .to_uppercase()
                    .chain(chars.flat_map(char::to_lowercase))
                    .collect(),
                None => String::new(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// The kind a FEATURES & TRAITS heading's entries are, and what grants them.
/// `Err` holds the heading when this reader does not know it.
fn section(heading: &str, c: &ImportedCharacter) -> Result<(&'static str, Vec<String>), String> {
    let upper = heading.trim().to_uppercase();
    let spelled = |prefix: &str, known: Option<&String>| {
        known
            .filter(|k| k.to_uppercase() == prefix)
            .cloned()
            .unwrap_or_else(|| title(prefix))
    };
    if upper == "FEATS" {
        return Ok(("feat", Vec::new()));
    }
    if upper == "BACKGROUND FEATURE" || upper == "BACKGROUND FEATURES" {
        return Ok((
            "feature",
            c.identity.background.value.iter().cloned().collect(),
        ));
    }
    if let Some(prefix) = upper.strip_suffix(" FEATURES") {
        let class = c
            .classes
            .iter()
            .filter_map(|k| k.name.value.as_ref())
            .find(|name| name.to_uppercase() == prefix);
        return Ok(("feature", vec![spelled(prefix, class)]));
    }
    if let Some(prefix) = upper.strip_suffix(" TRAITS") {
        return Ok((
            "species_trait",
            vec![spelled(prefix, c.identity.species.value.as_ref())],
        ));
    }
    Err(heading.trim().to_string())
}

/// "1 / Short Rest • 1 Bonus Action".
fn usage(line: &str, entry: &mut NamedContent) {
    let mut parts = line.split('\u{2022}').map(str::trim);
    let first = parts.next().unwrap_or_default();
    let mut activation: Vec<&str> = parts.collect();
    match first.split_once('/') {
        Some((uses, recharge)) if uses.trim().parse::<i32>().is_ok() => {
            entry.link.uses = uses.trim().parse().ok();
            entry.link.recharge = Some(recharge.trim().to_lowercase().replace(' ', "_"));
        }
        _ => activation.insert(0, first),
    }
    put(
        &mut entry.fields,
        "activation",
        &activation.join(" \u{2022} "),
    );
}

fn features(sheet: &Sheet, c: &ImportedCharacter) -> Vec<NamedContent> {
    let mut out: Vec<NamedContent> = Vec::new();
    let mut description: Vec<String> = Vec::new();
    let mut current: Result<(&'static str, Vec<String>), String> = Err(String::new());
    let finish = |out: &mut Vec<NamedContent>, description: &mut Vec<String>| {
        if let Some(last) = out.last_mut()
            && !description.is_empty()
        {
            put(&mut last.fields, "description", &description.join("\n"));
        }
        description.clear();
    };
    for column in 1..=6 {
        let Some(entry) = sheet.filled(&format!("FeaturesTraits{column}")) else {
            continue;
        };
        for line in entry.value.lines() {
            let trimmed = line.trim();
            if let Some(heading) = trimmed
                .strip_prefix("===")
                .and_then(|rest| rest.strip_suffix("==="))
            {
                finish(&mut out, &mut description);
                current = section(heading, c);
            } else if let Some(rest) = trimmed.strip_prefix("* ") {
                finish(&mut out, &mut description);
                let (name, reference) = match rest.split_once('\u{2022}') {
                    Some((name, reference)) => (name.trim(), reference.trim()),
                    None => (rest.trim(), ""),
                };
                let source = Some(
                    Entry {
                        value: trimmed.to_string(),
                        ..entry.clone()
                    }
                    .source(),
                );
                let (kind, granted_by, field) = match &current {
                    Ok((kind, by)) => (*kind, by.clone(), Field::read(name.to_string(), source)),
                    Err(heading) => (
                        "feature",
                        Vec::new(),
                        Field::uncertain(
                            Some(name.to_string()),
                            if heading.is_empty() {
                                format!(
                                    "{name} is listed under no heading, so what grants it is not known."
                                )
                            } else {
                                format!(
                                    "{name} is listed under \u{201C}{heading}\u{201D}, a heading this reader does not know."
                                )
                            },
                            source,
                        ),
                    ),
                };
                let mut feature = item(kind, field);
                feature.link.granted_by = granted_by;
                put(&mut feature.fields, "source", reference);
                out.push(feature);
            } else if let Some(rest) = trimmed.strip_prefix('|') {
                if let Some(last) = out.last_mut() {
                    usage(rest.trim(), last);
                }
            } else if !trimmed.is_empty() && !out.is_empty() {
                description.push(trimmed.to_string());
            }
        }
    }
    finish(&mut out, &mut description);
    out
}

/// The spell table, top to bottom: a header row sets the level, and each
/// spell row under it is one spell.
fn spells(sheet: &Sheet) -> Vec<NamedContent> {
    enum Row<'a> {
        Header(&'a Entry),
        Spell(&'a str, &'a Entry),
    }
    let mut rows: Vec<Row> = Vec::new();
    for (name, entry) in sheet.iter() {
        if let Some(n) = name.strip_prefix("spellHeader")
            && !n.is_empty()
            && n.chars().all(|c| c.is_ascii_digit())
        {
            rows.push(Row::Header(entry));
        } else if let Some(n) = name.strip_prefix("spellName")
            && !n.is_empty()
            && n.chars().all(|c| c.is_ascii_digit())
            && !entry.text().is_empty()
        {
            rows.push(Row::Spell(n, entry));
        }
    }
    let top = |row: &Row| match row {
        Row::Header(e) | Row::Spell(_, e) => (e.page, e.rect.y0),
    };
    rows.sort_by(|a, b| {
        let (pa, ya) = top(a);
        let (pb, yb) = top(b);
        pa.cmp(&pb).then(ya.total_cmp(&yb))
    });

    let mut out = Vec::new();
    let mut level = None;
    for row in rows {
        match row {
            Row::Header(entry) => level = spell_level(entry.text()),
            Row::Spell(n, entry) => {
                let cell = |column: &str| {
                    sheet
                        .get(&format!("spell{column}{n}"))
                        .map(|e| e.text().to_string())
                        .unwrap_or_default()
                };
                let name = entry.text().to_string();
                let prepared = cell("Prepared");
                let (prepared, doubt) = match prepared.as_str() {
                    "" => (None, None),
                    "P" => (Some(true), None),
                    "O" => (Some(false), None),
                    other => (
                        None,
                        Some(format!(
                            "The prepared mark beside {name} is \u{201C}{other}\u{201D}, which this reader does not know."
                        )),
                    ),
                };
                let doubt = doubt.or_else(|| {
                    level
                        .is_none()
                        .then(|| format!("{name} is not under a spell level this reader can read."))
                });
                let field = match doubt {
                    Some(reason) => {
                        Field::uncertain(Some(name.clone()), reason, Some(entry.source()))
                    }
                    None => Field::read(name.clone(), Some(entry.source())),
                };
                let mut spell = item("spell", field);
                if let Some(level) = level {
                    spell.fields.insert("level".into(), json!(level));
                }
                for (key, column) in [
                    ("save_hit", "SaveHit"),
                    ("casting_time", "CastingTime"),
                    ("range", "Range"),
                    ("components", "Components"),
                    ("duration", "Duration"),
                    ("source", "Page"),
                    ("notes", "Notes"),
                ] {
                    put(&mut spell.fields, key, &cell(column));
                }
                spell.link.prepared = prepared;
                let by = cell("Source");
                if !by.is_empty() {
                    spell.link.granted_by = vec![by];
                }
                out.push(spell);
            }
        }
    }
    out
}

/// "55 lb." is 55; "--" is no weight.
fn weight(text: &str) -> Option<f64> {
    text.trim()
        .trim_end_matches('.')
        .trim_end_matches("lb")
        .trim_end_matches("lbs")
        .trim()
        .replace(',', "")
        .parse()
        .ok()
}

fn quantity(text: Option<&Entry>) -> Option<i32> {
    text.and_then(|e| parse_int(e.text())).map(|n| n as i32)
}

fn equipment(sheet: &Sheet) -> Vec<NamedContent> {
    let mut out: Vec<NamedContent> = Vec::new();
    let mut add = |name: &Entry, qty: Option<&Entry>, wt: Option<&Entry>, attuned: bool| {
        let key = thunderforge_sheet_import::normalise_name(name.text());
        if attuned
            && let Some(held) = out.iter_mut().find(|item| {
                item.name
                    .value
                    .as_deref()
                    .is_some_and(|n| thunderforge_sheet_import::normalise_name(n) == key)
            })
        {
            held.link.attuned = Some(true);
            return;
        }
        let mut entry = item(
            "item",
            Field::read(name.text().to_string(), Some(name.source())),
        );
        entry.link.quantity = quantity(qty);
        if attuned {
            entry.link.attuned = Some(true);
        }
        if let Some(weight) = wt.and_then(|w| weight(w.text())) {
            entry.fields.insert("weight".into(), json!(weight));
        }
        out.push(entry);
    };
    for row in 0.. {
        let Some(name) = sheet.get(&format!("Eq Name{row}")) else {
            break;
        };
        if !name.text().is_empty() {
            add(
                name,
                sheet.get(&format!("Eq Qty{row}")),
                sheet.get(&format!("Eq Weight{row}")),
                false,
            );
        }
    }
    for row in 1..=3 {
        if let Some(name) = sheet.filled(&format!("Attuned Name{row}")) {
            let wt = sheet
                .get(&format!("Attuned Weight{row}"))
                .or_else(|| sheet.get(&format!("AttunedWeight{row}")));
            add(name, sheet.get(&format!("Attuned Qty{row}")), wt, true);
        }
    }
    out
}

/// The weapon rows, as the form names them.
const WEAPON_ROWS: [(&str, &str, &str, &str); 6] = [
    ("Wpn Name", "Wpn1 AtkBonus", "Wpn1 Damage", "Wpn Notes 1"),
    ("Wpn Name 2", "Wpn2 AtkBonus", "Wpn2 Damage", "Wpn Notes 2"),
    ("Wpn Name 3", "Wpn3 AtkBonus", "Wpn3 Damage", "Wpn Notes 3"),
    ("Wpn Name 4", "Wpn4 AtkBonus", "Wpn4 Damage", "Wpn Notes 4"),
    ("Wpn Name 5", "Wpn5 AtkBonus", "Wpn5 Damage", "Wpn Notes 5"),
    ("Wpn Name 6", "Wpn6 AtkBonus", "Wpn6 Damage", "Wpn Notes 6"),
];

fn attacks(sheet: &Sheet) -> Vec<NamedContent> {
    let mut out = Vec::new();
    for (name, hit, damage, notes) in WEAPON_ROWS {
        let Some(name) = sheet.filled(name) else {
            continue;
        };
        let mut attack = item(
            "attack",
            Field::read(name.text().to_string(), Some(name.source())),
        );
        let text = |field: &str| {
            sheet
                .get(field)
                .map(|e| e.text().to_string())
                .unwrap_or_default()
        };
        let hit = text(hit);
        let words: Vec<&str> = hit.split_whitespace().collect();
        match (parse_int(&hit), words.as_slice()) {
            (Some(bonus), _) if hit.starts_with(['+', '-']) => {
                attack.fields.insert("to_hit".into(), json!(bonus));
            }
            (_, [ability, dc]) if dc.parse::<i32>().is_ok() => {
                attack
                    .fields
                    .insert("save_ability".into(), json!(ability.to_lowercase()));
                attack
                    .fields
                    .insert("save_dc".into(), json!(dc.parse::<i32>().unwrap_or(0)));
            }
            _ => put(&mut attack.fields, "hit", &hit),
        }
        let damage = text(damage);
        match damage.rsplit_once(' ') {
            Some((roll, kind)) if DAMAGE_TYPES.contains(&kind.to_lowercase().as_str()) => {
                put(&mut attack.fields, "damage", roll);
                attack
                    .fields
                    .insert("damage_type".into(), json!(kind.to_lowercase()));
            }
            _ => put(&mut attack.fields, "damage", &damage),
        }
        put(&mut attack.fields, "notes", &text(notes));
        out.push(attack);
    }
    out
}
