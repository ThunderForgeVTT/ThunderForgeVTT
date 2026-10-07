//! A system's `conditions` block (spec 067 Story 4).
//!
//! The states a character can be in that the table needs to see at a glance:
//! poisoned, prone, bound. A pack declares each one here — its label, what it
//! means, and the marker the board draws — and the host stores which of them
//! a character has, lets the Game Master apply and clear them, and delivers
//! them to whoever may see that character's token.
//!
//! # Why the marker is a name and a token
//!
//! ADR-062: a pack extends the engine with data, not code. The engine is
//! handed a condition's identifier and its marker and nothing else, so it
//! learns no ruleset. The glyph is one of a closed list of shapes the engine
//! can compose and the colour is one of a closed list of tokens, so a pack
//! cannot ship an image, a hex value, or anything else the engine would have
//! to trust.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One state a character may be in.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemCondition {
    /// What is stored against a character. Unique within the system.
    pub id: String,
    /// What the table reads.
    pub label: String,
    /// What it means, in the system's own words.
    #[serde(default)]
    pub description: Option<String>,
    /// What the board draws on the character's token.
    pub marker: ConditionMarker,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConditionMarker {
    pub glyph: ConditionGlyph,
    pub color: ConditionColor,
}

/// The shapes the engine can compose. Named for what is drawn, never for
/// what it means: the meaning is the pack's, and lives in the label.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConditionGlyph {
    Dot,
    Ring,
    Bar,
    Cross,
    Split,
    Corner,
}

impl ConditionGlyph {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Ring => "ring",
            Self::Bar => "bar",
            Self::Cross => "cross",
            Self::Split => "split",
            Self::Corner => "corner",
        }
    }
}

/// The colours a marker may be. Tokens rather than values, so the board
/// decides what "danger" looks like and every system's markers agree.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConditionColor {
    Danger,
    Warning,
    Positive,
    Info,
    Arcane,
    Neutral,
}

impl ConditionColor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Danger => "danger",
            Self::Warning => "warning",
            Self::Positive => "positive",
            Self::Info => "info",
            Self::Arcane => "arcane",
            Self::Neutral => "neutral",
        }
    }
}

/// The conditions a manifest declares, in the manifest's order.
///
/// Lenient on purpose, as `settings_from_manifest` is: a block that does not
/// parse declares nothing. Refusing a bad block is
/// [`validate_conditions_content`]'s job.
pub fn conditions_from_manifest(manifest: &Value) -> Vec<SystemCondition> {
    manifest
        .get("conditions")
        .filter(|block| !block.is_null())
        .and_then(|block| serde_json::from_value(block.clone()).ok())
        .unwrap_or_default()
}

/// A `conditions` block that is present must be one the board can draw.
pub fn validate_conditions_content(instance: &Value) -> Result<(), String> {
    let Some(block) = instance.get("conditions").filter(|b| !b.is_null()) else {
        return Ok(());
    };
    let conditions: Vec<SystemCondition> =
        serde_json::from_value(block.clone()).map_err(|e| format!("conditions: {e}"))?;

    let mut seen = std::collections::HashSet::new();
    for condition in &conditions {
        let id = condition.id.trim();
        if id.is_empty() || id != condition.id {
            return Err(
                "conditions: an id must be non-empty with no surrounding space".to_string(),
            );
        }
        if !seen.insert(id) {
            return Err(format!("conditions: the id \"{id}\" is declared twice"));
        }
        if condition.label.trim().is_empty() {
            return Err(format!("conditions.{id}: a label is required"));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "conditions_tests.rs"]
mod tests;
