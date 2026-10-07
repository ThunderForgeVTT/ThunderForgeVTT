//! A system's `settings` block (spec 067 Story 1).
//!
//! What a table may choose about how it plays this ruleset. A pack declares
//! each choice here — its label, its type, its default — and the host stores
//! a world's answers in one shared table, renders the form, and announces a
//! change. The pack writes no migration, no GraphQL type and no panel.
//!
//! # Why a declaration and not a bag
//!
//! ADR-063 rejected a generic key/value store, and ADR-108 said what the
//! generic surface must be instead: keys a registry declares. This is that
//! registry. A row whose key nothing here declares is inert, and a value this
//! declaration does not allow is refused — never coerced, because a setting
//! quietly read as something its Game Master did not choose is a world
//! playing a different game from the one they set up.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A text setting with no `maxLength` of its own is held to this.
pub const DEFAULT_MAX_TEXT_LENGTH: usize = 500;

/// One choice a world may make.
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemSetting {
    /// The key a world's answer is stored under. Unique within the system.
    pub id: String,
    /// What the Game Master reads beside the control.
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: SettingKind,
    /// What a world that has never answered plays by.
    pub default: Value,
    /// Integer settings only: the least and greatest value allowed.
    #[serde(default)]
    pub min: Option<i64>,
    #[serde(default)]
    pub max: Option<i64>,
    /// Choice settings only: what may be chosen.
    #[serde(default)]
    pub options: Vec<SettingOption>,
    /// Text settings only.
    #[serde(default)]
    pub max_length: Option<usize>,
    /// Lower sorts first. Ties keep the manifest's order.
    #[serde(default)]
    pub order: i32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SettingKind {
    Boolean,
    Integer,
    Choice,
    Text,
}

impl SettingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Boolean => "boolean",
            Self::Integer => "integer",
            Self::Choice => "choice",
            Self::Text => "text",
        }
    }
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
}

impl SystemSetting {
    /// Whether `value` is one this setting allows.
    ///
    /// The message names the setting by its label, because it is shown to the
    /// Game Master who tried to set it.
    pub fn check(&self, value: &Value) -> Result<(), String> {
        match self.kind {
            SettingKind::Boolean => value
                .as_bool()
                .map(|_| ())
                .ok_or_else(|| format!("{} must be on or off", self.label)),
            SettingKind::Integer => {
                let number = value
                    .as_i64()
                    .ok_or_else(|| format!("{} must be a whole number", self.label))?;
                if let Some(min) = self.min
                    && number < min
                {
                    return Err(format!("{} must be at least {min}", self.label));
                }
                if let Some(max) = self.max
                    && number > max
                {
                    return Err(format!("{} must be at most {max}", self.label));
                }
                Ok(())
            }
            SettingKind::Choice => {
                let chosen = value
                    .as_str()
                    .ok_or_else(|| format!("{} must be one of its options", self.label))?;
                if self.options.iter().any(|option| option.value == chosen) {
                    Ok(())
                } else {
                    Err(format!("{} has no option \"{chosen}\"", self.label))
                }
            }
            SettingKind::Text => {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("{} must be text", self.label))?;
                let limit = self.max_length.unwrap_or(DEFAULT_MAX_TEXT_LENGTH);
                if text.chars().count() > limit {
                    return Err(format!("{} must be at most {limit} characters", self.label));
                }
                Ok(())
            }
        }
    }
}

/// The settings a manifest declares, in the order a form shows them.
///
/// Lenient on purpose: a block that does not parse declares nothing, the way
/// every other manifest reader here treats an unreadable block. Refusing a
/// bad block is [`validate_settings_content`]'s job, at install and in the
/// bundled-manifest walk, where somebody is looking.
pub fn settings_from_manifest(manifest: &Value) -> Vec<SystemSetting> {
    let Some(block) = manifest.get("settings").filter(|b| !b.is_null()) else {
        return Vec::new();
    };
    let mut settings: Vec<SystemSetting> =
        serde_json::from_value(block.clone()).unwrap_or_default();
    // Stable, so equal `order` keeps the manifest's own sequence.
    settings.sort_by_key(|setting| setting.order);
    settings
}

/// A `settings` block that is present must be one a form can be built from.
pub fn validate_settings_content(instance: &Value) -> Result<(), String> {
    let Some(block) = instance.get("settings").filter(|b| !b.is_null()) else {
        return Ok(());
    };
    let settings: Vec<SystemSetting> =
        serde_json::from_value(block.clone()).map_err(|e| format!("settings: {e}"))?;

    let mut seen = std::collections::HashSet::new();
    for setting in &settings {
        let id = setting.id.trim();
        if id.is_empty() || id != setting.id {
            return Err("settings: an id must be non-empty with no surrounding space".to_string());
        }
        if !seen.insert(id) {
            return Err(format!("settings: the id \"{id}\" is declared twice"));
        }
        if setting.label.trim().is_empty() {
            return Err(format!("settings.{id}: a label is required"));
        }
        if setting.kind == SettingKind::Choice && setting.options.is_empty() {
            return Err(format!("settings.{id}: a choice needs options"));
        }
        if let (Some(min), Some(max)) = (setting.min, setting.max)
            && min > max
        {
            return Err(format!("settings.{id}: min is greater than max"));
        }
        setting
            .check(&setting.default)
            .map_err(|e| format!("settings.{id}: the default is not allowed — {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
