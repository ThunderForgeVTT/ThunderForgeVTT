//! A skill roll, judged on the server (spec 067 Story 3).
//!
//! Until this module the sheet rolled a pool whose size it named, compared
//! the total with a number it held, and wrote the experience a failure earns
//! itself. All three were the browser's word. Here the server reads the
//! skill's level, the statuses, what has to be beaten and the tie rule, rolls
//! through the host, judges, and pays a failure its experience in the same
//! transaction as the roll's record.
//!
//! It is in two parts, as the host's contract asks. [`adjudicate`] is pure:
//! a roll and a context value in, a verdict out, and it is what the pack
//! registers. `graphql` is the pack's own mutation, which gathers that
//! context and is the only caller that knows where it comes from.

pub mod graphql;

#[cfg(test)]
mod db_tests;

use thunderforge_canvas_core::system_contribution::{RollFacts, RollOutcome, Verdict};

/// The check id this pack's own mutation rolls under.
pub const SKILL_CHECK: &str = "skill";

/// What a skill roll is judged against, as the mutation gathered it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Judging {
    /// The statuses' modifiers, summed. Nought in a world without statuses.
    pub modifier: i64,
    /// What has to be beaten. `None` is "nobody said", and the roll is not
    /// judged.
    pub opposition: Option<f64>,
    /// Whether matching the opposition is enough in this world.
    pub tie_succeeds: bool,
}

impl Judging {
    /// The context value [`adjudicate`] reads.
    pub fn context(&self) -> serde_json::Value {
        serde_json::json!({
            "modifier": self.modifier,
            "opposition": self.opposition,
            "tieSucceeds": self.tie_succeeds,
        })
    }
}

/// What the roll came to once the statuses are counted.
pub fn modified_total(facts: &RollFacts<'_>, context: &serde_json::Value) -> f64 {
    let modifier = context
        .get("modifier")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    facts.total + modifier as f64
}

/// Roll for Shoes' adjudicator: beat the opposition, or fail.
///
/// A tie is a failure, and earns its experience like any other, unless the
/// world has said ties succeed. The game has two results and no criticals —
/// what a six earns is advancement, which is the sheet's business and not a
/// verdict. With no opposition there is nothing to judge against, and the
/// answer is `None` rather than either result.
pub fn adjudicate(facts: &RollFacts<'_>, context: &serde_json::Value) -> Option<RollOutcome> {
    let opposition = context
        .get("opposition")
        .and_then(serde_json::Value::as_f64)?;
    let tie_succeeds = context
        .get("tieSucceeds")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let total = modified_total(facts, context);

    let beaten = if tie_succeeds {
        total >= opposition
    } else {
        total > opposition
    };
    Some(if beaten {
        RollOutcome {
            verdict: Verdict::Success,
            label: "Success".to_string(),
        }
    } else {
        RollOutcome {
            verdict: Verdict::Failure,
            label: "Failure".to_string(),
        }
    })
}

/// One skill as the server reads it off a character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSkill {
    pub id: String,
    pub level: i64,
}

/// The skills a character has: what is stored, or what they were handed.
///
/// The same rule the sheet's `skillsOf` applies, and it has to be the same,
/// because the sheet names a skill by the id this function gives it. A
/// character with stored skills never reads the world's starting skills; one
/// with none has the world's, or the manifest's when the world declares none,
/// under the ids `starting-skill`, `starting-skill-1`, and so on.
pub fn skills_of(
    trait_data: Option<&serde_json::Value>,
    world_starting_skills: &[(String, i64)],
) -> Vec<StoredSkill> {
    let stored: Vec<StoredSkill> = trait_data
        .and_then(|data| data.get("skills"))
        .and_then(serde_json::Value::as_array)
        .map(|skills| {
            skills
                .iter()
                .filter_map(|skill| {
                    Some(StoredSkill {
                        id: skill.get("id")?.as_str()?.to_string(),
                        level: skill.get("level")?.as_i64()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if !stored.is_empty() {
        return stored;
    }

    let handed = if world_starting_skills.is_empty() {
        crate::starting_skills()
    } else {
        world_starting_skills.to_vec()
    };
    handed
        .into_iter()
        .enumerate()
        .map(|(index, (_, level))| StoredSkill {
            id: if index == 0 {
                "starting-skill".to_string()
            } else {
                format!("starting-skill-{index}")
            },
            level: level.max(1),
        })
        .collect()
}

/// The statuses' modifiers, summed; nothing stored is nought.
pub fn status_modifier(trait_data: Option<&serde_json::Value>) -> i64 {
    trait_data
        .and_then(|data| data.get("statuses"))
        .and_then(serde_json::Value::as_array)
        .map(|statuses| {
            statuses
                .iter()
                .filter_map(|status| status.get("modifier")?.as_i64())
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn judged(total: f64, judging: Judging) -> Option<Verdict> {
        let facts = RollFacts {
            check: SKILL_CHECK,
            dice: &[],
            total,
        };
        adjudicate(&facts, &judging.context()).map(|outcome| outcome.verdict)
    }

    fn against(opposition: f64) -> Judging {
        Judging {
            modifier: 0,
            opposition: Some(opposition),
            tie_succeeds: false,
        }
    }

    #[test]
    fn beating_the_opposition_succeeds_and_anything_less_fails() {
        assert_eq!(judged(8.0, against(7.0)), Some(Verdict::Success));
        assert_eq!(judged(6.0, against(7.0)), Some(Verdict::Failure));
    }

    #[test]
    fn a_tie_fails_unless_the_world_says_ties_succeed() {
        assert_eq!(judged(7.0, against(7.0)), Some(Verdict::Failure));
        let lenient = Judging {
            tie_succeeds: true,
            ..against(7.0)
        };
        assert_eq!(judged(7.0, lenient), Some(Verdict::Success));
        assert_eq!(judged(6.0, lenient), Some(Verdict::Failure));
    }

    #[test]
    fn with_nothing_to_beat_the_roll_is_not_judged() {
        let unopposed = Judging {
            modifier: 0,
            opposition: None,
            tie_succeeds: true,
        };
        assert_eq!(judged(12.0, unopposed), None);
    }

    #[test]
    fn the_statuses_move_the_total_before_it_is_judged() {
        let hindered = Judging {
            modifier: -2,
            ..against(7.0)
        };
        assert_eq!(judged(8.0, hindered), Some(Verdict::Failure));
        let helped = Judging {
            modifier: 2,
            ..against(7.0)
        };
        assert_eq!(judged(6.0, helped), Some(Verdict::Success));
    }

    #[test]
    fn the_label_is_the_games_own_word_for_the_verdict() {
        let facts = RollFacts {
            check: SKILL_CHECK,
            dice: &[3, 3],
            total: 6.0,
        };
        let outcome = adjudicate(&facts, &against(7.0).context()).expect("judged");
        assert_eq!(outcome.label, "Failure");
    }

    #[test]
    fn stored_skills_are_read_and_the_starting_skills_are_not() {
        let trait_data = serde_json::json!({
            "skills": [{ "id": "a", "name": "Climb", "level": 3, "parentId": null }]
        });
        let skills = skills_of(Some(&trait_data), &[("Fight".to_string(), 2)]);
        assert_eq!(
            skills,
            vec![StoredSkill {
                id: "a".to_string(),
                level: 3
            }]
        );
    }

    #[test]
    fn a_character_with_nothing_stored_has_what_the_world_hands_out() {
        let world = [("Fight".to_string(), 2), ("Talk".to_string(), 1)];
        let skills = skills_of(None, &world);
        assert_eq!(skills[0].id, "starting-skill");
        assert_eq!(skills[0].level, 2);
        assert_eq!(skills[1].id, "starting-skill-1");

        let core = skills_of(Some(&serde_json::json!({ "skills": [] })), &[]);
        assert_eq!(
            core,
            vec![StoredSkill {
                id: "starting-skill".to_string(),
                level: 1
            }]
        );
    }

    #[test]
    fn status_modifiers_sum_and_none_stored_is_nought() {
        let trait_data = serde_json::json!({
            "statuses": [
                { "id": "a", "name": "Soaked", "modifier": -2 },
                { "id": "b", "name": "Furious", "modifier": 1 }
            ]
        });
        assert_eq!(status_modifier(Some(&trait_data)), -1);
        assert_eq!(status_modifier(None), 0);
    }
}
