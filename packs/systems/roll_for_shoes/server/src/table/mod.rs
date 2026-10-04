//! What has to be beaten, said once for the whole table.
//!
//! # Why the server holds it
//!
//! Roll for Shoes is played mostly in the theatre of the mind: the Game
//! Master says how hard a thing is, and the player rolls against that. Until
//! this module the number lived on the player's own sheet — they typed it, or
//! in the "rolled" difficulty mode they rolled the Game Master's dice
//! themselves, and could roll them again until they liked the answer. Nothing
//! was dishonest about the sheet; it simply had nowhere else to get the
//! number from.
//!
//! So the number is now a row the Game Master writes and every member reads.
//! While a row exists a sheet uses it and offers its player nothing to type;
//! when the Game Master clears it, the row is deleted and the sheet is back
//! to free entry. *No row is not an error* — it is how every world that has
//! never used this plays, exactly as before.
//!
//! # How the number is arrived at
//!
//! Three ways, and [`plan`] is the one place that chooses between them:
//!
//! - The Game Master **names a number**. Always allowed, whatever the world's
//!   difficulty mode: a Game Master who wants "beat a 7" should not have to
//!   change a setting first.
//! - The Game Master **names a band** in a world whose mode is `target`: the
//!   band's fixed number is stored.
//! - The Game Master **names a band** in a world whose mode is `rolled`: the
//!   band's dice are rolled **here, once**, and both the faces and their sum
//!   are stored. Nobody at the table can roll them again but the Game Master,
//!   by setting the difficulty again.
//!
//! A band in a `free` world is refused: that world has said it does not use
//! bands, and guessing which of the two meanings was wanted would be a
//! coercion.
//!
//! The bands' dice and targets are the same four pairs the pack's web half
//! states in `game.ts` (`BAND_DICE`, `BAND_TARGET`). They are written twice
//! because the two halves share no code; the tests below pin this half.

#[cfg(test)]
mod db_tests;
pub mod graphql;
pub mod schema;

use diesel::prelude::*;
use uuid::Uuid;

use crate::settings::DifficultyMode;
use schema::world_roll_for_shoes_difficulty;

/// The world event that says the table's difficulty changed.
///
/// Reserved in the server's `world_events.rs`, where every code is listed so
/// that two features cannot take the same one. Payload:
/// `{"action": "set" | "cleared"}` — deliberately not the number. A client
/// that hears this asks again, and the server answers per caller.
pub const EVENT_CODE_TABLE_DIFFICULTY: i32 =
    thunderforge_server::world_events::EVENT_CODE_TABLE_DIFFICULTY_CHANGED;

/// The lowest and highest number a Game Master may name.
///
/// One is the least any roll can total. The ceiling is not a rule of the
/// game — it only keeps a slipped key from storing a number no pool of d6
/// this game hands out could ever reach by a factor of ten.
pub const LOWEST_TARGET: i32 = 1;
pub const HIGHEST_TARGET: i32 = 999;

/// How hard the Game Master called it, by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    Easy,
    Moderate,
    Hard,
    VeryHard,
}

impl Band {
    /// The spelling stored, sent, and used by the pack's web half.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Easy => "easy",
            Self::Moderate => "moderate",
            Self::Hard => "hard",
            Self::VeryHard => "veryHard",
        }
    }

    /// `None` for anything else. An unknown band is a mistake, not "easy".
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "easy" => Some(Self::Easy),
            "moderate" => Some(Self::Moderate),
            "hard" => Some(Self::Hard),
            "veryHard" => Some(Self::VeryHard),
            _ => None,
        }
    }

    /// How many d6 the Game Master rolls for this band in a `rolled` world.
    pub fn dice(self) -> u32 {
        match self {
            Self::Easy => 1,
            Self::Moderate => 2,
            Self::Hard => 3,
            Self::VeryHard => 4,
        }
    }

    /// The fixed number this band stands for in a `target` world.
    pub fn fixed_target(self) -> i32 {
        match self {
            Self::Easy => 3,
            Self::Moderate => 6,
            Self::Hard => 9,
            Self::VeryHard => 12,
        }
    }
}

/// The standing difficulty, as a caller sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableDifficulty {
    pub target: i32,
    pub band: Option<Band>,
    /// The faces the server rolled, when it rolled. Their sum is `target`.
    pub gm_dice: Option<Vec<i32>>,
}

#[derive(Debug, Clone, Queryable, Selectable, Identifiable)]
#[diesel(table_name = world_roll_for_shoes_difficulty)]
#[diesel(primary_key(world_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct DifficultyRow {
    pub world_id: Uuid,
    pub target: i32,
    pub band: Option<String>,
    pub gm_dice: Option<serde_json::Value>,
    pub set_by: Option<Uuid>,
    pub set_at: chrono::NaiveDateTime,
}

impl From<DifficultyRow> for TableDifficulty {
    fn from(row: DifficultyRow) -> Self {
        Self {
            target: row.target,
            // The column's CHECK constraint admits only the four spellings,
            // so a failed parse can only be a row written around it. Such a
            // row still has its number, which is the part a roll needs.
            band: row.band.as_deref().and_then(Band::parse),
            gm_dice: row
                .gm_dice
                .and_then(|value| serde_json::from_value::<Vec<i32>>(value).ok()),
        }
    }
}

#[derive(Debug, Insertable)]
#[diesel(table_name = world_roll_for_shoes_difficulty)]
pub struct SetDifficulty {
    pub world_id: Uuid,
    pub target: i32,
    pub band: Option<String>,
    pub gm_dice: Option<serde_json::Value>,
    pub set_by: Option<Uuid>,
}

/// The standing difficulty, or `None` when the Game Master has set none.
///
/// Unlike the settings read this keeps its `Option`: whether a row exists
/// *is* the question — it is the difference between a sheet that shows the
/// Game Master's number and one that lets its player type their own.
pub fn read(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<Option<TableDifficulty>> {
    let row = world_roll_for_shoes_difficulty::table
        .find(world_id)
        .select(DifficultyRow::as_select())
        .first(conn)
        .optional()?;
    Ok(row.map(TableDifficulty::from))
}

/// Set the difficulty, replacing whatever stood before.
///
/// A whole-row replacement: a new difficulty owes nothing to the old one, and
/// a band or a set of dice left over from the last call would describe a
/// number that is no longer the one stored.
pub fn set(conn: &mut PgConnection, values: SetDifficulty) -> QueryResult<TableDifficulty> {
    let row = diesel::insert_into(world_roll_for_shoes_difficulty::table)
        .values(&values)
        .on_conflict(world_roll_for_shoes_difficulty::world_id)
        .do_update()
        .set((
            world_roll_for_shoes_difficulty::target.eq(values.target),
            world_roll_for_shoes_difficulty::band.eq(&values.band),
            world_roll_for_shoes_difficulty::gm_dice.eq(&values.gm_dice),
            world_roll_for_shoes_difficulty::set_by.eq(values.set_by),
            world_roll_for_shoes_difficulty::set_at.eq(diesel::dsl::now),
        ))
        .returning(DifficultyRow::as_select())
        .get_result(conn)?;
    Ok(row.into())
}

/// Clear the difficulty. `true` when there was one to clear.
pub fn clear(conn: &mut PgConnection, world_id: Uuid) -> QueryResult<bool> {
    let removed =
        diesel::delete(world_roll_for_shoes_difficulty::table.find(world_id)).execute(conn)?;
    Ok(removed > 0)
}

/// What the server has to do to honour a request to set the difficulty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// Store this number; no band, no dice.
    Named(i32),
    /// Store the band and its fixed number.
    Fixed(Band),
    /// Roll the band's dice once, and store the band, the faces and their sum.
    Rolled(Band),
}

/// What a caller got wrong, stated so the message can be shown to them.
#[derive(Debug, PartialEq, Eq)]
pub enum DifficultyError {
    /// Neither a number nor a band, or both at once.
    NeedsExactlyOne,
    NumberOutOfRange(i32),
    UnknownBand(String),
    /// A band in a world whose difficulty mode is `free`.
    BandsNotInUse,
}

impl std::fmt::Display for DifficultyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedsExactlyOne => {
                write!(f, "Set the difficulty as a number or as a band, not both")
            }
            Self::NumberOutOfRange(n) => write!(
                f,
                "A difficulty must be between {LOWEST_TARGET} and {HIGHEST_TARGET}, not {n}"
            ),
            Self::UnknownBand(band) => write!(f, "\"{band}\" is not a difficulty band"),
            Self::BandsNotInUse => write!(
                f,
                "This world does not use difficulty bands; name a number instead"
            ),
        }
    }
}

/// Decide how a request becomes a number. Pure: no database, no dice.
///
/// Refusals, never coercions — the same stance `settings::validate` takes. A
/// request with both a number and a band is not resolved in favour of either,
/// because whichever was dropped is something a Game Master believed they
/// had said.
pub fn plan(
    mode: DifficultyMode,
    target: Option<i32>,
    band: Option<&str>,
) -> Result<Plan, DifficultyError> {
    match (target, band) {
        (Some(number), None) => {
            if (LOWEST_TARGET..=HIGHEST_TARGET).contains(&number) {
                Ok(Plan::Named(number))
            } else {
                Err(DifficultyError::NumberOutOfRange(number))
            }
        }
        (None, Some(spelling)) => {
            let band = Band::parse(spelling)
                .ok_or_else(|| DifficultyError::UnknownBand(spelling.to_string()))?;
            match mode {
                DifficultyMode::Free => Err(DifficultyError::BandsNotInUse),
                DifficultyMode::Target => Ok(Plan::Fixed(band)),
                DifficultyMode::Rolled => Ok(Plan::Rolled(band)),
            }
        }
        _ => Err(DifficultyError::NeedsExactlyOne),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_number_is_allowed_in_every_mode() {
        for mode in [
            DifficultyMode::Free,
            DifficultyMode::Rolled,
            DifficultyMode::Target,
        ] {
            assert_eq!(plan(mode, Some(7), None), Ok(Plan::Named(7)));
        }
    }

    #[test]
    fn a_number_outside_the_range_is_refused_not_clamped() {
        assert_eq!(
            plan(DifficultyMode::Free, Some(0), None),
            Err(DifficultyError::NumberOutOfRange(0))
        );
        assert_eq!(
            plan(DifficultyMode::Free, Some(-3), None),
            Err(DifficultyError::NumberOutOfRange(-3))
        );
        assert_eq!(
            plan(DifficultyMode::Free, Some(1000), None),
            Err(DifficultyError::NumberOutOfRange(1000))
        );
        assert_eq!(
            plan(DifficultyMode::Free, Some(1), None),
            Ok(Plan::Named(1))
        );
        assert_eq!(
            plan(DifficultyMode::Free, Some(999), None),
            Ok(Plan::Named(999))
        );
    }

    #[test]
    fn a_band_means_a_fixed_number_or_a_roll_depending_on_the_world() {
        assert_eq!(
            plan(DifficultyMode::Target, None, Some("hard")),
            Ok(Plan::Fixed(Band::Hard))
        );
        assert_eq!(
            plan(DifficultyMode::Rolled, None, Some("hard")),
            Ok(Plan::Rolled(Band::Hard))
        );
    }

    #[test]
    fn a_band_in_a_world_that_does_not_use_bands_is_refused() {
        assert_eq!(
            plan(DifficultyMode::Free, None, Some("easy")),
            Err(DifficultyError::BandsNotInUse)
        );
    }

    #[test]
    fn an_unknown_band_is_refused_not_read_as_easy() {
        assert_eq!(
            plan(DifficultyMode::Target, None, Some("impossible")),
            Err(DifficultyError::UnknownBand("impossible".into()))
        );
        // Spelled as the web half spells it, exactly.
        assert_eq!(Band::parse("veryhard"), None);
        assert_eq!(Band::parse("veryHard"), Some(Band::VeryHard));
    }

    #[test]
    fn neither_or_both_is_refused() {
        assert_eq!(
            plan(DifficultyMode::Target, None, None),
            Err(DifficultyError::NeedsExactlyOne)
        );
        assert_eq!(
            plan(DifficultyMode::Target, Some(6), Some("moderate")),
            Err(DifficultyError::NeedsExactlyOne)
        );
    }

    /// The four pairs the web half states as `BAND_DICE` and `BAND_TARGET`.
    #[test]
    fn the_bands_carry_the_dice_and_targets_the_sheet_shows() {
        let bands = [Band::Easy, Band::Moderate, Band::Hard, Band::VeryHard];
        assert_eq!(bands.map(Band::dice), [1, 2, 3, 4]);
        assert_eq!(bands.map(Band::fixed_target), [3, 6, 9, 12]);
        for band in bands {
            assert_eq!(Band::parse(band.as_str()), Some(band));
        }
    }
}
