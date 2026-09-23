//! A world's answers to the five optional rules (spec 062).
//!
//! # Why this exists at all
//!
//! Roll for Shoes publishes six core rules and a handful of "Extras" it
//! explicitly leaves to the table. Spec 061 shipped the six. These are the
//! Extras, and every one of them is a *world* decision — this table plays with
//! statuses, that one does not — so they need somewhere per-world to live.
//!
//! Nothing in the product offered that. There is no generic per-world,
//! per-system configuration surface: `worlds` has no JSON column, the settings
//! it does carry are hand-written columns with a mutation each, and the one
//! existing per-system setting is `worlds.genie_resource_carryover_enabled` —
//! a column named for one ruleset on a table every system shares, which that
//! pack's own source files as a known defect.
//!
//! So: a table this pack owns, per ADR-063. See `research.md` D1 for the four
//! candidates weighed, and ADR-108 for why the generic surface — now justified
//! by ADR-063's own "one pack is a case, two is a shape" test — is still a
//! spec of its own rather than a paragraph of this one.
//!
//! # The one invariant everything here rests on
//!
//! **A world with no row is not an error.** It reads as every default, which
//! is the core game exactly as spec 061 shipped it. That is why nothing seeds
//! a row at world creation, why the read never fails on a missing row, and why
//! a world that predates this table is indistinguishable from one whose Game
//! Master has never opened the settings panel.

#[cfg(test)]
mod db_tests;
pub mod graphql;
mod play_pause_surface;
pub mod schema;

use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use schema::world_roll_for_shoes_settings;

/// One starting skill a world hands a new character.
///
/// The level is `i64` to match what the trait validator reads out of JSON;
/// nothing here caps it, because Roll for Shoes caps no skill's level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartingSkill {
    pub name: String,
    pub level: i64,
}

/// A world's settings, as the rest of the pack sees them.
///
/// Distinct from the row: this is what a world *has*, whether or not anybody
/// has written a row for it. `Default` is the core game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSettings {
    pub difficulty_mode: DifficultyMode,
    pub tie_succeeds: bool,
    pub statuses_enabled: bool,
    pub skill_slots_enabled: bool,
    /// Empty means the core default, "Do Anything 1" — never "this world's
    /// characters start with no skills".
    pub starting_skills: Vec<StartingSkill>,
}

impl Default for WorldSettings {
    fn default() -> Self {
        Self {
            difficulty_mode: DifficultyMode::Free,
            tie_succeeds: false,
            statuses_enabled: false,
            skill_slots_enabled: false,
            starting_skills: Vec::new(),
        }
    }
}

/// How the Game Master states the opposition.
///
/// `Free` is what the core game has always done and stays available in every
/// mode — a Game Master who wants to say "beat 7" may always say it. The other
/// two add a named way to arrive at the number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DifficultyMode {
    Free,
    Rolled,
    Target,
}

impl DifficultyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Rolled => "rolled",
            Self::Target => "target",
        }
    }

    /// Refuses an unknown mode rather than falling back to `Free`.
    ///
    /// A silent fallback would turn a typo in a mutation into a world that
    /// quietly plays a different game from the one its Game Master chose.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "free" => Some(Self::Free),
            "rolled" => Some(Self::Rolled),
            "target" => Some(Self::Target),
            _ => None,
        }
    }
}

/// The stored row.
#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = world_roll_for_shoes_settings)]
#[diesel(primary_key(world_id))]
pub struct SettingsRow {
    pub world_id: Uuid,
    pub difficulty_mode: String,
    pub tie_succeeds: bool,
    pub statuses_enabled: bool,
    pub skill_slots_enabled: bool,
    pub starting_skills: serde_json::Value,
    pub updated_by: Option<Uuid>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<SettingsRow> for WorldSettings {
    fn from(row: SettingsRow) -> Self {
        Self {
            // A row holding a mode this build does not know reads as `Free`.
            // The CHECK constraint makes that unreachable going forward; it is
            // handled anyway because the alternative is a panic on a read.
            difficulty_mode: DifficultyMode::parse(&row.difficulty_mode)
                .unwrap_or(DifficultyMode::Free),
            tie_succeeds: row.tie_succeeds,
            statuses_enabled: row.statuses_enabled,
            skill_slots_enabled: row.skill_slots_enabled,
            starting_skills: serde_json::from_value(row.starting_skills).unwrap_or_default(),
        }
    }
}

#[derive(Debug, Insertable, AsChangeset)]
#[diesel(table_name = world_roll_for_shoes_settings)]
pub struct UpsertSettings {
    pub world_id: Uuid,
    pub difficulty_mode: String,
    pub tie_succeeds: bool,
    pub statuses_enabled: bool,
    pub skill_slots_enabled: bool,
    pub starting_skills: serde_json::Value,
    pub updated_by: Option<Uuid>,
}

/// This world's settings, or the defaults if nobody has written a row.
///
/// The `Option` from the query is collapsed here deliberately: no caller ever
/// needs to know whether a row exists, and one that branched on it would be
/// asking a question the feature does not have an answer for.
pub fn read_or_default(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<WorldSettings> {
    let row = world_roll_for_shoes_settings::table
        .find(world_id)
        .select(SettingsRow::as_select())
        .first(conn)
        .optional()?;
    Ok(row.map(WorldSettings::from).unwrap_or_default())
}

/// Write all five settings, creating the row if this is the first change.
///
/// A whole-row upsert, not a patch. The caller reads, changes one, and sends
/// all five back — see `contracts/graphql.md` for why: an input of optionals
/// makes "absent" ambiguous between "leave it" and "clear it", and the
/// guarantee that enabling one setting never silently changes another is
/// easiest to hold when every call states all five.
pub fn upsert(conn: &mut PgConnection, values: UpsertSettings) -> QueryResult<WorldSettings> {
    let row = diesel::insert_into(world_roll_for_shoes_settings::table)
        .values(&values)
        .on_conflict(world_roll_for_shoes_settings::world_id)
        .do_update()
        .set((
            world_roll_for_shoes_settings::difficulty_mode.eq(&values.difficulty_mode),
            world_roll_for_shoes_settings::tie_succeeds.eq(values.tie_succeeds),
            world_roll_for_shoes_settings::statuses_enabled.eq(values.statuses_enabled),
            world_roll_for_shoes_settings::skill_slots_enabled.eq(values.skill_slots_enabled),
            world_roll_for_shoes_settings::starting_skills.eq(&values.starting_skills),
            world_roll_for_shoes_settings::updated_by.eq(values.updated_by),
            world_roll_for_shoes_settings::updated_at.eq(diesel::dsl::now),
        ))
        .returning(SettingsRow::as_select())
        .get_result(conn)?;
    Ok(row.into())
}

/// What a caller got wrong, stated so the message can be shown to them.
///
/// Refusals, never coercions: a starting skill named `"   "` is a mistake
/// worth reporting, not a blank to silently accept.
#[derive(Debug, PartialEq, Eq)]
pub enum SettingsError {
    UnknownDifficultyMode(String),
    StartingSkillNeedsName,
    StartingSkillLevelTooLow(i64),
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownDifficultyMode(given) => {
                write!(f, "Unknown difficulty mode: {given}")
            }
            Self::StartingSkillNeedsName => write!(f, "A starting skill needs a name"),
            Self::StartingSkillLevelTooLow(level) => write!(
                f,
                "A starting skill's level must be at least 1, not {level}"
            ),
        }
    }
}

/// Check a proposed set of settings before it reaches the database.
pub fn validate(
    difficulty_mode: &str,
    starting_skills: &[StartingSkill],
) -> Result<DifficultyMode, SettingsError> {
    let mode = DifficultyMode::parse(difficulty_mode)
        .ok_or_else(|| SettingsError::UnknownDifficultyMode(difficulty_mode.to_string()))?;

    for skill in starting_skills {
        if skill.name.trim().is_empty() {
            return Err(SettingsError::StartingSkillNeedsName);
        }
        if skill.level < 1 {
            return Err(SettingsError::StartingSkillLevelTooLow(skill.level));
        }
    }

    Ok(mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_the_core_game() {
        let settings = WorldSettings::default();
        assert_eq!(settings.difficulty_mode, DifficultyMode::Free);
        assert!(!settings.tie_succeeds);
        assert!(!settings.statuses_enabled);
        assert!(!settings.skill_slots_enabled);
        assert!(settings.starting_skills.is_empty());
    }

    #[test]
    fn an_unknown_difficulty_mode_is_refused_not_defaulted() {
        let err = validate("hard-mode", &[]).unwrap_err();
        assert_eq!(
            err,
            SettingsError::UnknownDifficultyMode("hard-mode".into())
        );
    }

    #[test]
    fn the_three_modes_parse() {
        for mode in ["free", "rolled", "target"] {
            assert_eq!(validate(mode, &[]).unwrap().as_str(), mode);
        }
    }

    #[test]
    fn a_starting_skill_needs_a_name_that_is_not_only_spaces() {
        let skills = vec![StartingSkill {
            name: "   ".into(),
            level: 1,
        }];
        assert_eq!(
            validate("free", &skills).unwrap_err(),
            SettingsError::StartingSkillNeedsName
        );
    }

    #[test]
    fn a_starting_skill_starts_at_level_one_or_higher() {
        let skills = vec![StartingSkill {
            name: "Do Anything".into(),
            level: 0,
        }];
        assert_eq!(
            validate("free", &skills).unwrap_err(),
            SettingsError::StartingSkillLevelTooLow(0)
        );
    }

    /// An empty list is legal and means the core default. It is not the same
    /// as a world whose characters start with nothing, and no validation rule
    /// may confuse the two.
    #[test]
    fn no_starting_skills_is_legal() {
        assert!(validate("free", &[]).is_ok());
    }
}
