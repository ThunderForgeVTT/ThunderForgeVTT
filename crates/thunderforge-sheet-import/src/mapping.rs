//! A system's `sheetImport` declaration (spec 048 FR-010, FR-012).
//!
//! The declaration says, as data, where each neutral path of a reading lands
//! on that system's actor. It is parsed and checked when the pack loads: a
//! target naming a data type or field the pack does not declare is a load
//! failure, not a silent drop at import time.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde_json::Value;

/// Targets on the actor row itself rather than in a data type.
const ACTOR_TARGETS: &[&str] = &["actor.label"];

/// Where one neutral content kind lands.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContentTarget {
    /// A linked ability of this vocabulary type.
    Ability { vocabulary: String },
    /// A linked item.
    Item,
    /// The pack's refine hook decides. Left undecided, it is unmapped.
    Refine,
}

/// A parsed, checked `sheetImport` block.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SheetMapping {
    pub readers: Vec<String>,
    /// Neutral path to target ("abilities.str" to "ability_data.strength").
    /// A path may name a whole part ("classes"): every leaf under it lands
    /// with it, as one value.
    pub fields: BTreeMap<String, String>,
    pub content: BTreeMap<String, ContentTarget>,
    /// Globs over a reading's `derived` keys: never written, only checked.
    pub derived: Vec<String>,
    /// Targets a re-import keeps unless the person names them.
    pub play_state: BTreeSet<String>,
    /// Neutral paths read and deliberately not kept: a player's name.
    pub ignore: Vec<String>,
    /// Where unmapped values are appended, labelled.
    pub notes: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MappingError(pub String);

impl fmt::Display for MappingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sheetImport: {}", self.0)
    }
}

impl std::error::Error for MappingError {}

fn fail<T>(message: impl Into<String>) -> Result<T, MappingError> {
    Err(MappingError(message.into()))
}

impl SheetMapping {
    /// Parse and check the `sheetImport` block of a pack manifest.
    ///
    /// `Ok(None)` when the manifest has no block: the system takes no sheets
    /// (`SYSTEM_HAS_NO_MAPPING`). Targets are checked against the manifest's
    /// own `data_types` and `abilityVocabulary`.
    pub fn from_manifest(manifest: &Value) -> Result<Option<SheetMapping>, MappingError> {
        let Some(block) = manifest.get("sheetImport") else {
            return Ok(None);
        };
        let mapping = Self::parse(block)?;
        mapping.check(manifest)?;
        Ok(Some(mapping))
    }

    /// Parse a block without checking its targets against a manifest.
    pub fn parse(block: &Value) -> Result<SheetMapping, MappingError> {
        let Some(object) = block.as_object() else {
            return fail("must be an object");
        };
        const KNOWN: &[&str] = &[
            "readers",
            "fields",
            "content",
            "derived",
            "playState",
            "ignore",
            "notes",
        ];
        if let Some(unknown) = object.keys().find(|k| !KNOWN.contains(&k.as_str())) {
            return fail(format!("unknown key `{unknown}`"));
        }

        let readers = strings(block.get("readers"), "readers")?;
        if readers.is_empty() {
            return fail("`readers` names no reader");
        }

        let mut fields = BTreeMap::new();
        if let Some(raw) = block.get("fields") {
            let Some(map) = raw.as_object() else {
                return fail("`fields` must be an object");
            };
            for (path, target) in map {
                if !is_path(path, false) {
                    return fail(format!("`fields` path `{path}` does not parse"));
                }
                let Some(target) = target.as_str() else {
                    return fail(format!("`fields.{path}` must be a string"));
                };
                fields.insert(path.clone(), target.to_string());
            }
        }

        let mut content = BTreeMap::new();
        if let Some(raw) = block.get("content") {
            let Some(map) = raw.as_object() else {
                return fail("`content` must be an object");
            };
            for (kind, target) in map {
                content.insert(kind.clone(), content_target(kind, target)?);
            }
        }

        let derived = strings(block.get("derived"), "derived")?;
        for glob in &derived {
            if !is_path(glob, true) {
                return fail(format!("`derived` entry `{glob}` does not parse"));
            }
        }
        let play_state: BTreeSet<String> = strings(block.get("playState"), "playState")?
            .into_iter()
            .collect();
        for target in &play_state {
            if !is_path(target, true) {
                return fail(format!("`playState` entry `{target}` does not parse"));
            }
        }
        let ignore = strings(block.get("ignore"), "ignore")?;
        let Some(notes) = block.get("notes").and_then(Value::as_str) else {
            return fail("`notes` must name where unmapped values go");
        };

        Ok(SheetMapping {
            readers,
            fields,
            content,
            derived,
            play_state,
            ignore,
            notes: notes.to_string(),
        })
    }

    fn check(&self, manifest: &Value) -> Result<(), MappingError> {
        for (path, target) in &self.fields {
            if !target_exists(manifest, target) {
                return fail(format!(
                    "`fields.{path}` targets `{target}`, which the pack does not declare"
                ));
            }
        }
        if !target_exists(manifest, &self.notes) {
            return fail(format!(
                "`notes` targets `{}`, which the pack does not declare",
                self.notes
            ));
        }
        let vocabulary: BTreeSet<&str> = manifest
            .pointer("/abilityVocabulary/types")
            .and_then(Value::as_array)
            .map(|types| types.iter().filter_map(|t| t["id"].as_str()).collect())
            .unwrap_or_default();
        for (kind, target) in &self.content {
            if let ContentTarget::Ability { vocabulary: ty } = target
                && !vocabulary.contains(ty.as_str())
            {
                return fail(format!(
                    "`content.{kind}` names ability type `{ty}`, which the pack does not declare"
                ));
            }
        }
        Ok(())
    }

    /// The declared path that covers `path`: itself, or the nearest part
    /// holding it.
    pub fn field_for(&self, path: &str) -> Option<(&str, &str)> {
        self.fields
            .iter()
            .filter(|(declared, _)| covers(declared, path))
            .max_by_key(|(declared, _)| declared.len())
            .map(|(declared, target)| (declared.as_str(), target.as_str()))
    }

    /// Whether a `derived` key is one the declaration checks.
    pub fn is_derived(&self, key: &str) -> bool {
        self.derived.iter().any(|glob| glob_matches(glob, key))
    }

    pub fn is_ignored(&self, path: &str) -> bool {
        self.ignore.iter().any(|ignored| covers(ignored, path))
    }
}

/// Whether `declared` is `path` or a part holding it.
pub fn covers(declared: &str, path: &str) -> bool {
    path == declared
        || (path.len() > declared.len()
            && path.starts_with(declared)
            && path.as_bytes()[declared.len()] == b'.')
}

/// `*` matches one segment; a trailing `*` matches one or more.
pub fn glob_matches(glob: &str, key: &str) -> bool {
    let pattern: Vec<&str> = glob.split('.').collect();
    let parts: Vec<&str> = key.split('.').collect();
    for (index, segment) in pattern.iter().enumerate() {
        let last = index == pattern.len() - 1;
        if *segment == "*" && last {
            return parts.len() > index;
        }
        match parts.get(index) {
            Some(part) if *segment == "*" || segment == part => {}
            _ => return false,
        }
    }
    parts.len() == pattern.len()
}

fn strings(raw: Option<&Value>, name: &str) -> Result<Vec<String>, MappingError> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let Some(items) = raw.as_array() else {
        return fail(format!("`{name}` must be a list"));
    };
    items
        .iter()
        .map(|item| match item.as_str() {
            Some(text) => Ok(text.to_string()),
            None => fail(format!("`{name}` must hold only strings")),
        })
        .collect()
}

fn content_target(kind: &str, raw: &Value) -> Result<ContentTarget, MappingError> {
    let object = raw.as_object();
    let single = object.filter(|o| o.len() == 1);
    match single.and_then(|o| o.iter().next()) {
        Some((key, Value::String(ty))) if key == "ability" => Ok(ContentTarget::Ability {
            vocabulary: ty.clone(),
        }),
        Some((key, Value::Bool(true))) if key == "item" => Ok(ContentTarget::Item),
        Some((key, Value::Bool(true))) if key == "refine" => Ok(ContentTarget::Refine),
        _ => fail(format!(
            "`content.{kind}` must be {{\"ability\": type}}, {{\"item\": true}} or {{\"refine\": true}}"
        )),
    }
}

/// `segment(.segment)*`, each segment lower-case letters, digits and `_`,
/// optionally ending in `[]`. With `globs`, a segment may be `*`.
fn is_path(path: &str, globs: bool) -> bool {
    !path.is_empty()
        && path.split('.').all(|segment| {
            if globs && segment == "*" {
                return true;
            }
            let name = segment.strip_suffix("[]").unwrap_or(segment);
            !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        })
}

fn target_exists(manifest: &Value, target: &str) -> bool {
    if ACTOR_TARGETS.contains(&target) {
        return true;
    }
    let Some((data_type, field)) = target.split_once('.') else {
        return false;
    };
    if field.contains('.') {
        return false;
    }
    manifest
        .get("data_types")
        .and_then(|types| types.get(data_type))
        .and_then(|ty| ty.get("properties"))
        .and_then(|properties| properties.get(field))
        .is_some()
}
