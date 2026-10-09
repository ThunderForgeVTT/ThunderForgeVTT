//! The marks D&D Beyond prints beside a save or a skill.
//!
//! A mark this table does not know is never guessed at: the value is
//! Uncertain, with no value, and the reason names the mark so the person can
//! say what it means.

use thunderforge_sheet_import::SkillMark;

/// What a skill's mark means, or `Err` with the mark when it is unknown.
pub fn skill_mark(raw: &str) -> Result<SkillMark, String> {
    match raw.trim() {
        "" => Ok(SkillMark::None),
        "P" | "\u{2022}" | "\u{25CF}" => Ok(SkillMark::Proficient),
        "E" => Ok(SkillMark::Expertise),
        "H" => Ok(SkillMark::Half),
        other => Err(other.to_string()),
    }
}

/// Whether a save's mark means proficient, or `Err` with the mark.
pub fn save_mark(raw: &str) -> Result<bool, String> {
    match raw.trim() {
        "" => Ok(false),
        "\u{2022}" | "\u{25CF}" | "P" => Ok(true),
        other => Err(other.to_string()),
    }
}

/// The reason shown for a mark the table does not know.
pub fn unknown_mark(what: &str, mark: &str) -> String {
    format!(
        "The mark beside {what} is \u{201C}{mark}\u{201D}, which this reader does not know. \
Check whether the character is proficient."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table() {
        assert_eq!(skill_mark(""), Ok(SkillMark::None));
        assert_eq!(skill_mark("P"), Ok(SkillMark::Proficient));
        assert_eq!(skill_mark("E"), Ok(SkillMark::Expertise));
        assert_eq!(skill_mark("H"), Ok(SkillMark::Half));
        assert_eq!(skill_mark("\u{25A1}"), Err("\u{25A1}".into()));
        assert_eq!(save_mark("\u{2022}"), Ok(true));
        assert_eq!(save_mark(" "), Ok(false));
        assert_eq!(save_mark("x"), Err("x".into()));
        assert!(unknown_mark("Athletics", "x").contains("Athletics"));
    }
}
