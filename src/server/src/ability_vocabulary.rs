//! What a game system calls its abilities.
//!
//! The companion to `attributes::attribute_declarations_for_system` and
//! `sheet::declarations_for_system`, and deliberately the same shape: the
//! manifest is the authority, this file only parses it, and every function
//! here is **total** — no manifest, a malformed one, or a system that declares
//! nothing all yield a complete, correctly-labelled vocabulary rather than an
//! error or a blank.
//!
//! # Why this is assembled here and not in the browser
//!
//! It was in the browser, six times. `WorldCompendiumPage`,
//! `AbilityCompendiumTab`, `AbilityPreviewPanel`, `AbilityDetailPage`,
//! `ActorAbilitiesPanel` and the shared-ability page each fetched the manifest
//! and cast `abilityFacets` themselves. Spec 033's FR-006 requires every
//! surface naming an ability type to use the system's word for it, and six
//! independent readers is six chances to disagree about something they are
//! required to agree on.
//!
//! The server needs it regardless: FR-013 (a type not offered in the wrong
//! world), FR-019 (a binding refused) and FR-023 (a grade out of range) are all
//! refusals only the server can make. Assembling it twice, in two languages,
//! to two possibly different answers, is the thing to avoid.
//!
//! See ADR-064 and `specs/033-abilities-vocabulary/contracts/ability-vocabulary.md`.

use async_graphql::{Enum, SimpleObject};
use serde::Serialize;

/// What an ability of a type may be attached to.
///
/// **Exactly one**, never a set. A type binds to characters or to items or to
/// nothing, so FR-019's refusal is a comparison rather than a set membership
/// test, and a Spell stays a thing characters have while an Enchantment stays
/// a thing items carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Enum)]
#[serde(rename_all = "camelCase")]
pub enum Binds {
    Character,
    Item,
    Nothing,
}

impl Binds {
    /// Absent means `character`, which is what every ability does today.
    fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("item") => Self::Item,
            Some("nothing") => Self::Nothing,
            _ => Self::Character,
        }
    }
}

/// An ordered value a type's abilities carry — 5e's spell Level, another
/// system's Rank or Circle. One shape, many words.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct GradeFacet {
    /// The system's word for it. Never empty.
    pub label: String,
    pub min: i32,
    pub max: i32,
}

/// One type of ability, built in or declared by a system. A GM cannot tell
/// which is which, and that is the point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct AbilityTypeDeclaration {
    /// Stable identity. Matches what is stored in
    /// `world_abilities.classification`.
    pub id: String,
    /// What a person reads. Never empty — falls back to the id.
    pub label: String,
    pub plural_label: String,
    pub order: i32,
    /// True for the four the application ships. Carried for the presence rule
    /// (FR-011a) and for diagnostics, never to decide what a GM may author.
    pub builtin: bool,
    pub binds: Binds,
    pub grade: Option<GradeFacet>,
}

/// The system's word for the concept itself, replacing "Ability"/"Abilities".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct UmbrellaTerm {
    pub label: String,
    pub plural_label: String,
}

impl Default for UmbrellaTerm {
    fn default() -> Self {
        Self {
            label: "Ability".to_string(),
            plural_label: "Abilities".to_string(),
        }
    }
}

/// Everything a world needs to name and group its abilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
pub struct AbilityVocabulary {
    pub umbrella: UmbrellaTerm,
    /// In display order. Built-ins first, then declared types.
    pub types: Vec<AbilityTypeDeclaration>,
}

/// The four the application ships, in their established order.
///
/// **Permanently authorable** (FR-017). Existing worlds and existing abilities
/// require no migration, no re-typing and no GM action, whatever a system says.
/// A declaration matching one of these ids re-labels it; it never creates a
/// second type and never removes the built-in.
pub const BUILTIN_TYPE_IDS: [&str; 4] = ["spell", "feat", "power", "talent"];

fn builtin_label(id: &str) -> (&'static str, &'static str) {
    match id {
        "spell" => ("Spell", "Spells"),
        "feat" => ("Feat", "Feats"),
        "power" => ("Power", "Powers"),
        "talent" => ("Talent", "Talents"),
        _ => ("Ability", "Abilities"),
    }
}

/// A non-empty string, or nothing. Used everywhere a label is read, so that a
/// declaration carrying `""` falls back rather than rendering blank (FR-016).
fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The built-in vocabulary, before any system has spoken.
fn builtins() -> Vec<AbilityTypeDeclaration> {
    BUILTIN_TYPE_IDS
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let (label, plural) = builtin_label(id);
            AbilityTypeDeclaration {
                id: (*id).to_string(),
                label: label.to_string(),
                plural_label: plural.to_string(),
                order: index as i32,
                builtin: true,
                binds: Binds::Character,
                grade: None,
            }
        })
        .collect()
}

/// Read one `types` entry, or skip it.
///
/// A malformed entry loses only itself (FR-016): a pack does not forfeit its
/// whole vocabulary to one typo, and a missing label falls back to the id
/// rather than to blank.
fn read_type_entry(entry: &serde_json::Value, order: i32) -> Option<AbilityTypeDeclaration> {
    let id = non_empty(entry.get("id").and_then(serde_json::Value::as_str))?;
    let label = non_empty(entry.get("label").and_then(serde_json::Value::as_str))
        .unwrap_or_else(|| id.clone());
    let plural_label = non_empty(entry.get("pluralLabel").and_then(serde_json::Value::as_str))
        .unwrap_or_else(|| label.clone());

    let grade = entry.get("grade").and_then(|grade| {
        let label = non_empty(grade.get("label").and_then(serde_json::Value::as_str))?;
        let min = i32::try_from(grade.get("min")?.as_i64()?).ok()?;
        let max = i32::try_from(grade.get("max")?.as_i64()?).ok()?;
        // A range that cannot contain anything is not a range. Skipping the
        // facet leaves the type ungraded, which is a state the product
        // already handles, rather than refusing every value a GM enters.
        (min <= max).then_some(GradeFacet { label, min, max })
    });

    Some(AbilityTypeDeclaration {
        id,
        label,
        plural_label,
        order: entry
            .get("order")
            .and_then(serde_json::Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(order),
        builtin: false,
        binds: Binds::parse(entry.get("binds").and_then(serde_json::Value::as_str)),
        grade,
    })
}

/// Read the legacy `abilityFacets` block as a labels-only declaration.
///
/// `packs/systems/genie/system.json` ships it, relabelling `spell` to Scroll
/// and `talent` to Knack. Breaking a shipped pack to tidy a key name would be
/// this feature failing its own premise, so it keeps working: it carries
/// labels, and nothing else.
fn read_legacy_facets(manifest: &serde_json::Value) -> Vec<AbilityTypeDeclaration> {
    let Some(facets) = manifest.get("abilityFacets").and_then(|f| f.as_object()) else {
        return Vec::new();
    };

    facets
        .iter()
        .filter_map(|(id, entry)| {
            let id = non_empty(Some(id))?;
            let label = non_empty(entry.get("label").and_then(serde_json::Value::as_str))
                .unwrap_or_else(|| id.clone());
            let plural_label =
                non_empty(entry.get("pluralLabel").and_then(serde_json::Value::as_str))
                    .unwrap_or_else(|| label.clone());
            Some(AbilityTypeDeclaration {
                id,
                label,
                plural_label,
                order: i32::MAX,
                builtin: false,
                binds: Binds::Character,
                grade: None,
            })
        })
        .collect()
}

/// Assemble a system's vocabulary from its manifest.
///
/// Split from the filesystem so it can be tested without one, exactly as
/// `attributes::declarations_from_manifest` is.
///
/// `in_use` is the set of classifications the world actually holds abilities
/// of. It decides **presence**, not availability: a built-in the system
/// neither declares nor re-labels, and of which the world holds none, is not
/// shown (FR-011a). That is why this is assembled per world rather than cached
/// per system.
pub fn from_manifest(manifest: &serde_json::Value, in_use: &[String]) -> AbilityVocabulary {
    let umbrella = manifest
        .get("abilityVocabulary")
        .and_then(|v| v.get("umbrella"))
        .and_then(|umbrella| {
            let label = non_empty(umbrella.get("label").and_then(serde_json::Value::as_str))?;
            let plural_label = non_empty(
                umbrella
                    .get("pluralLabel")
                    .and_then(serde_json::Value::as_str),
            )
            .unwrap_or_else(|| label.clone());
            Some(UmbrellaTerm {
                label,
                plural_label,
            })
        })
        .unwrap_or_default();

    let declared: Vec<AbilityTypeDeclaration> = manifest
        .get("abilityVocabulary")
        .and_then(|v| v.get("types"))
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .enumerate()
                .filter_map(|(index, entry)| read_type_entry(entry, index as i32))
                .collect()
        })
        .unwrap_or_default();

    // The newer block wins for the ids it covers; the older one fills in the
    // rest, so a pack shipping both is not punished for having been early.
    let mut all = declared;
    for legacy in read_legacy_facets(manifest) {
        if !all.iter().any(|declared| declared.id == legacy.id) {
            all.push(legacy);
        }
    }

    let mut types = builtins();

    // Which built-ins the system spoke about at all.
    //
    // Declaring one *is* the statement of use, whether or not the label
    // differs from ours. Inferring it from "the label changed" was the first
    // shape of this and it was wrong: a system declaring `spell` as "Spell",
    // which is a perfectly ordinary thing to do, lost its tab.
    let mut declared_ids: Vec<String> = Vec::new();

    for declaration in all {
        declared_ids.push(declaration.id.clone());
        match types
            .iter_mut()
            .find(|builtin| builtin.id == declaration.id)
        {
            // Re-labelling a built-in (FR-014): one type, not two. The
            // built-in flag survives, because what a GM may author has not
            // changed — only what it is called.
            Some(existing) => {
                existing.label = declaration.label;
                existing.plural_label = declaration.plural_label;
                existing.binds = declaration.binds;
                existing.grade = declaration.grade;
                if declaration.order != i32::MAX {
                    existing.order = declaration.order;
                }
            }
            None => types.push(declaration),
        }
    }

    // FR-011a. A built-in stays when the system declared it, or when the world
    // holds one. Nothing a system declared itself is ever dropped.
    types.retain(|declaration| {
        !declaration.builtin
            || declared_ids.contains(&declaration.id)
            || in_use.iter().any(|held| held == &declaration.id)
    });

    types.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));

    AbilityVocabulary { umbrella, types }
}

/// A system's vocabulary, read from disk.
///
/// An unreadable or absent manifest yields the built-in vocabulary — correct
/// rather than defensive. A world whose system pack is missing still has
/// abilities, and they still have names (SC-013).
pub fn for_system(
    systems_dir: &str,
    system_id: Option<&str>,
    in_use: &[String],
) -> AbilityVocabulary {
    let Some(system_id) = system_id else {
        return from_manifest(&serde_json::Value::Null, in_use);
    };

    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");

    let Ok(text) = std::fs::read_to_string(path) else {
        return from_manifest(&serde_json::Value::Null, in_use);
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return from_manifest(&serde_json::Value::Null, in_use);
    };
    from_manifest(&manifest, in_use)
}

impl AbilityVocabulary {
    /// Whether this vocabulary recognises a stored classification.
    ///
    /// The question FR-013 and FR-034 both turn on, asked in one place.
    pub fn recognises(&self, classification: &str) -> bool {
        self.types.iter().any(|kind| kind.id == classification)
    }

    /// The declaration for a stored classification, if it is recognised.
    pub fn get(&self, classification: &str) -> Option<&AbilityTypeDeclaration> {
        self.types.iter().find(|kind| kind.id == classification)
    }
}

#[cfg(test)]
#[path = "ability_vocabulary_tests.rs"]
mod tests;
