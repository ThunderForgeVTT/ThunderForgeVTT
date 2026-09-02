//! Reading the rest of a character sheet out of a system's manifest.
//!
//! `abilities`, `skills`, `resources` and `movement` were the whole
//! vocabulary, and between them they describe about a third of a Cypher sheet
//! and two numbers of a Fate one. This is the block that carries everything
//! else: the text a player writes, the slots they name themselves, the tracks
//! they tick, and the ladders they slide down.
//!
//! Same shape as `crate::attributes` and `crate::status_display`, and for the
//! same reason: the manifest is the authority on what a system has, this file
//! only parses it, and the rules for turning stored data into values live in
//! `thunderforge_canvas_core` where tests execute.

use thunderforge_canvas_core::system_rules::{DeclaredValue, DeclaredValueKind, Origin};

/// One thing on a sheet that is not a score, a skill, a pool or a speed.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetDeclaration {
    pub id: String,
    pub label: String,
    /// Which stored slot to read — `traitData`, `resourceData`, and so on.
    pub slot: String,
    /// The field inside that slot. Defaults to `id`.
    pub source: String,
    pub kind: SheetKind,
    /// The group this belongs to, when it is part of one (FR-033).
    pub group: Option<String>,
    pub order: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SheetKind {
    /// A line a player writes: an aspect, a descriptor, a focus.
    Text,
    /// A list a player adds to: equipment, cyphers, stunts.
    List,
    /// A bounded run of marks. Fate's eight stress boxes; 5e's three
    /// death-save successes.
    Track { of: u32 },
    /// An ordered ladder of named states. Cypher's impaired, debilitated,
    /// dead.
    State { options: Vec<String> },
    /// `count` entries the **player** names — Fate's twenty-six skill slots,
    /// Cypher's seven.
    ///
    /// A format that models only fixed lists turns those into twenty-six wrong
    /// labels, which is why this is its own kind rather than a list with a
    /// length (FR-032).
    Slots { count: u32 },
    /// A plain number the other blocks do not claim: a tier, an experience
    /// total, an armour rating.
    Number,
}

/// What a system says about a group of its values (T019g).
///
/// A group used to be nothing but an identifier shared by its members, which
/// left a renderer with two questions the format could not answer: what is
/// this group called, and which member is the one to show when there is room
/// for one. It answered both by taking the first member — right by luck for
/// Cypher's `might` group and wrong the moment a manifest is reordered.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetGroup {
    pub id: String,
    /// The group's own name. Absent means "call it after its first member",
    /// which is the old behaviour and stays available: a group whose members
    /// already read well together does not need naming twice.
    pub label: Option<String>,
    /// The id of the member to show when there is room for one.
    ///
    /// A Cypher stat shows its current value, not its edge. Absent means the
    /// renderer falls back to the first member.
    pub headline: Option<String>,
}

/// Read a system's group declarations.
///
/// Declared **once**, in the manifest's own `groups` block, and stamped onto
/// each member by [`apply_groups`]. The alternative — a `groupLabel` beside
/// every member — is the same fact written four times for a Fate consequence
/// set, and four places to disagree.
pub fn groups_from_manifest(manifest: &serde_json::Value) -> Vec<SheetGroup> {
    let Some(entries) = manifest.get("groups").and_then(|g| g.as_array()) else {
        return Vec::new();
    };

    entries
        .iter()
        .filter_map(|entry| {
            Some(SheetGroup {
                id: entry.get("id")?.as_str()?.to_string(),
                label: entry
                    .get("label")
                    .and_then(|l| l.as_str())
                    .map(str::to_string),
                headline: entry
                    .get("headline")
                    .and_then(|h| h.as_str())
                    .map(str::to_string),
            })
        })
        .collect()
}

/// Attach each group's name and headline to its members.
///
/// A value in no group, or in one the manifest never declared, is left
/// untouched: the renderer's first-member fallback is what it had before and
/// is still a reasonable answer. An undeclared group is not an error — a
/// system may group values purely to keep them together on the sheet, with
/// nothing to add about the grouping itself.
pub fn apply_groups(values: &mut [DeclaredValue], groups: &[SheetGroup]) {
    if groups.is_empty() {
        return;
    }
    for value in values.iter_mut() {
        let Some(group_id) = value.group.as_deref() else {
            continue;
        };
        let Some(group) = groups.iter().find(|g| g.id == group_id) else {
            continue;
        };
        value.group_label.clone_from(&group.label);
        value.headline = group.headline.as_deref() == Some(value.id.as_str());
    }
}

/// Read a system's sheet declarations.
///
/// A system that declares none yields an empty list, which is correct rather
/// than defensive: a ruleset whose sheet is scores and pools has nothing else
/// to say, and inventing entries for it would put fields on a character sheet
/// that no book supports.
pub fn declarations_for_system(systems_dir: &str, system_id: &str) -> Vec<SheetDeclaration> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    declarations_from_manifest(&manifest)
}

/// Split out so it can be tested without a filesystem.
pub fn declarations_from_manifest(manifest: &serde_json::Value) -> Vec<SheetDeclaration> {
    let Some(entries) = manifest.get("sheet").and_then(|s| s.as_array()) else {
        return Vec::new();
    };

    entries
        .iter()
        .enumerate()
        .filter_map(|(order, entry)| {
            let id = entry.get("id")?.as_str()?.to_string();
            let kind = match entry.get("kind")?.as_str()? {
                "text" => SheetKind::Text,
                "list" => SheetKind::List,
                "number" => SheetKind::Number,
                "track" => SheetKind::Track {
                    of: u32::try_from(entry.get("of")?.as_u64()?).ok()?,
                },
                "state" => SheetKind::State {
                    options: entry
                        .get("options")?
                        .as_array()?
                        .iter()
                        .filter_map(|o| o.as_str().map(str::to_string))
                        .collect(),
                },
                "slots" => SheetKind::Slots {
                    count: u32::try_from(entry.get("count")?.as_u64()?).ok()?,
                },
                // A kind this build does not know is skipped here rather than
                // guessed at. FR-035's "render it as text anyway" applies to a
                // value that arrived, not to a declaration nothing can read.
                _ => return None,
            };

            Some(SheetDeclaration {
                label: entry
                    .get("label")
                    .and_then(|l| l.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                slot: entry
                    .get("slot")
                    .and_then(|s| s.as_str())
                    .unwrap_or("traitData")
                    .to_string(),
                source: entry
                    .get("source")
                    .and_then(|s| s.as_str())
                    .unwrap_or(&id)
                    .to_string(),
                group: entry
                    .get("group")
                    .and_then(|g| g.as_str())
                    .map(str::to_string),
                kind,
                id,
                order,
            })
        })
        .collect()
}

/// Turn one declaration and its stored data into values.
///
/// Returns more than one only for [`SheetKind::Slots`], where a declaration is
/// a template for entries the player names.
///
/// Omission over defaulting throughout, for the reason it is applied
/// everywhere else in this feature: a field nobody filled in is not a field
/// whose value is nothing.
pub fn values_from(declaration: &SheetDeclaration, slot: &serde_json::Value) -> Vec<DeclaredValue> {
    let raw = slot.get(&declaration.source);

    let one = |value: DeclaredValueKind| DeclaredValue {
        id: declaration.id.clone(),
        label: declaration.label.clone(),
        abbreviation: None,
        value,
        group: declaration.group.clone(),
        // Stamped later by `apply_groups`, from the manifest's one `groups`
        // block — never written per member, so members cannot disagree.
        group_label: None,
        headline: false,
        origin: Origin::Stored,
    };

    match &declaration.kind {
        SheetKind::Text => match raw.and_then(|v| v.as_str()) {
            Some(text) if !text.is_empty() => vec![one(DeclaredValueKind::Text(text.to_string()))],
            _ => Vec::new(),
        },
        SheetKind::Number => match raw
            .and_then(|v| v.as_i64())
            .and_then(|n| i32::try_from(n).ok())
        {
            Some(number) => vec![one(DeclaredValueKind::Integer(number))],
            None => Vec::new(),
        },
        SheetKind::List => match raw.and_then(|v| v.as_array()) {
            Some(items) => {
                let items: Vec<String> = items
                    .iter()
                    .filter_map(|i| i.as_str().map(str::to_string))
                    .collect();
                if items.is_empty() {
                    Vec::new()
                } else {
                    vec![one(DeclaredValueKind::List(items))]
                }
            }
            None => Vec::new(),
        },
        SheetKind::Track { of } => {
            // A track with nothing ticked is still a track: the boxes exist
            // whether or not any are filled, unlike a text field nobody wrote
            // in. Showing an empty stress track is showing the truth.
            let filled = raw
                .and_then(|v| v.as_u64())
                .and_then(|n| u32::try_from(n).ok())
                .unwrap_or(0);
            vec![one(DeclaredValueKind::Track {
                filled: filled.min(*of),
                of: *of,
            })]
        }
        SheetKind::State { options } => {
            // Likewise: the ladder exists even when a character is on none of
            // it, and `current: None` is what "unharmed" looks like.
            let current = raw.and_then(|v| v.as_str()).map(str::to_string);
            vec![one(DeclaredValueKind::State {
                current: current.filter(|c| !c.is_empty()),
                options: options.clone(),
            })]
        }
        SheetKind::Slots { count } => {
            // Entries the player named. Stored as a list of `{name, value}`
            // or of plain strings; an entry they have not filled in yields
            // nothing rather than an empty row, so an unfilled sheet does not
            // arrive as twenty-six blanks.
            let Some(items) = raw.and_then(|v| v.as_array()) else {
                return Vec::new();
            };
            items
                .iter()
                .take(*count as usize)
                .enumerate()
                .filter_map(|(index, item)| {
                    let (name, value) = match item {
                        serde_json::Value::String(name) => (name.clone(), None),
                        serde_json::Value::Object(_) => {
                            let name = item.get("name")?.as_str()?.to_string();
                            let value = item
                                .get("value")
                                .and_then(|v| v.as_i64())
                                .and_then(|n| i32::try_from(n).ok());
                            (name, value)
                        }
                        _ => return None,
                    };
                    if name.is_empty() {
                        return None;
                    }
                    Some(DeclaredValue {
                        id: format!("{}{}", declaration.id, index + 1),
                        // The player's own words, which is the whole point of
                        // a slot: the system declares that there are some, and
                        // says nothing about what they are called.
                        label: name,
                        abbreviation: None,
                        value: match value {
                            Some(value) => DeclaredValueKind::Integer(value),
                            None => DeclaredValueKind::Text(String::new()),
                        },
                        group: declaration.group.clone(),
                        group_label: None,
                        headline: false,
                        origin: Origin::Stored,
                    })
                })
                .collect()
        }
    }
}

#[cfg(test)]
#[path = "sheet_tests.rs"]
mod tests;
