//! What a content pattern cannot say, and this system can (spec 049 FR-016).
//!
//! The shared anchored reader finds a creature by its armour class and reads
//! the labelled fields the manifest declares. That covers most of a statblock
//! and all of what shared code is allowed to know.
//!
//! It cannot reach **per-attack reach**. A reach lives inside an attack's free
//! prose — `Melee Weapon Attack: +7 to hit, reach 10 ft., one target.` — which
//! is not a labelled field and has no declarative form worth inventing. Spec
//! 045's playtest established that reach is per attack rather than per creature
//! size, so it is load-bearing and cannot simply be dropped.
//!
//! ADR-096 records the decision: rather than bend the declaration around one
//! system's prose, or keep a whole second reader alongside the generic one and
//! let the two disagree, the pack contributes a refinement. Shared code still
//! names no system; this file owns the one thing only this system knows.

use serde::{Deserialize, Serialize};
use thunderforge_canvas_core::content_entry::{Entry, SourceLine};

/// One thing a creature can do, and how far away it can do it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub name: String,
    pub text: String,
    pub reach_feet: Option<f64>,
}

/// The headings that end a statblock's header and begin the things it does.
const SECTION_HEADINGS: &[&str] = &[
    "actions",
    "reactions",
    "legendary actions",
    "lair actions",
    "regional effects",
    "bonus actions",
    "villain actions",
];

/// Add this system's actions to an entry the shared reader produced.
///
/// Registered through `SystemContribution::refine_content`, so nothing has to
/// call it by name.
pub fn refine(entry: &mut Entry, lines: &[SourceLine]) {
    if entry.kind != "creature" {
        return;
    }
    let actions = actions_in(lines);
    if actions.is_empty() {
        return;
    }
    entry.extras = serde_json::to_value(serde_json::json!({ "actions": actions })).ok();
}

fn actions_in(lines: &[SourceLine]) -> Vec<Action> {
    let mut out = Vec::new();
    let mut in_actions = false;
    for line in lines {
        let text = line.text.trim();
        let lowered = text.to_lowercase();
        if SECTION_HEADINGS.contains(&lowered.trim_end_matches(':')) {
            in_actions = lowered.starts_with("action")
                || lowered.starts_with("legendary")
                || lowered.starts_with("bonus")
                || lowered.starts_with("reaction")
                || lowered.starts_with("villain");
            continue;
        }
        if in_actions {
            if let Some(action) = action_of(text) {
                out.push(action);
            }
        }
    }
    out
}

fn action_of(text: &str) -> Option<Action> {
    let (name, rest) = text.split_once('.')?;
    let name = name.trim();
    // An action's name is short and titled. A sentence that merely contains a
    // full stop is prose continuing from the line before.
    if name.is_empty() || name.chars().count() > 48 {
        return None;
    }
    if !name.chars().next()?.is_uppercase() {
        return None;
    }
    Some(Action {
        name: name.to_string(),
        text: rest.trim().to_string(),
        reach_feet: reach_of(text),
    })
}

fn reach_of(text: &str) -> Option<f64> {
    let lowered = text.to_lowercase();
    let at = lowered.find("reach")? + "reach".len();
    let rest = lowered[at..].trim_start();
    let number: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let number = number.trim_end_matches('.');
    let value: f64 = number.parse().ok()?;
    // Guard against "reach" used in prose followed by an unrelated number.
    let tail = rest[number.len()..].trim_start();
    (tail.starts_with("ft") || tail.starts_with("feet") || tail.starts_with("'")).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str) -> SourceLine {
        SourceLine {
            text: text.to_string(),
            size: 9.0,
            bold: false,
            page: 1,
            suspect: false,
            heading: false,
        }
    }

    #[test]
    fn reach_is_read_per_attack_not_per_creature() {
        // The property spec 045's playtest established: two attacks on one
        // creature can reach different distances, and both must survive.
        let lines = vec![
            line("Actions"),
            line("Bite. Melee Weapon Attack: +7 to hit, reach 10 ft., one target."),
            line("Claw. Melee Weapon Attack: +7 to hit, reach 5 ft., one target."),
        ];
        let actions = actions_in(&lines);
        assert_eq!(actions.len(), 2);
        assert_eq!(actions[0].reach_feet, Some(10.0));
        assert_eq!(actions[1].reach_feet, Some(5.0));
    }

    #[test]
    fn prose_before_the_actions_heading_is_not_an_action() {
        let lines = vec![line("Amphibious. The dragon can breathe air and water.")];
        assert!(actions_in(&lines).is_empty());
    }

    #[test]
    fn the_word_reach_in_ordinary_prose_does_not_become_a_reach() {
        let lines = vec![
            line("Actions"),
            line("Roar. Creatures within reach 3 of its lair are frightened."),
        ];
        assert_eq!(actions_in(&lines)[0].reach_feet, None);
    }

    #[test]
    fn a_refinement_only_touches_the_kind_it_understands() {
        let mut entry = Entry {
            kind: "spell".into(),
            name: "Fireball".into(),
            name_state: thunderforge_canvas_core::content_entry::NameState::Clear,
            page: 1,
            values: Default::default(),
            text: None,
            suspect: false,
            extras: None,
        };
        refine(&mut entry, &[line("Actions"), line("Bite. reach 10 ft.")]);
        assert!(entry.extras.is_none(), "a spell has no actions to refine");
    }
}
