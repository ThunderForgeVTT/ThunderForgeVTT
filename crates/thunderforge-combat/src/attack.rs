//! What an attack decides (spec 046 contracts/fight.md §2, steps 4 to 8).
//!
//! The server's `combat::attack::record_attack` rolls, reads the target's
//! defence, and writes rows and events; between those it asks these. The demo
//! asks the same ones against the creatures it holds. The order the dice are
//! drawn in is part of the rule — to-hit, then damage only on a hit — so a
//! fight under a fixed RNG is one fight wherever it runs (spec 079 US3).

use rand_core::Rng;
use thunderforge_dice::{DiceFormula, PlaceholderBindings, ResolutionKind, RollResolution};

use crate::records::{
    FLAG_NO_LINE_OF_SIGHT, OUTCOME_HIT, OUTCOME_MISS, OUTCOME_NO_DEFENCE, OUTCOME_NO_TARGET,
};

/// What an attack costs (`world_attacks.action_cost`, research R13).
#[derive(Copy, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "graphql", derive(async_graphql::Enum))]
#[cfg_attr(feature = "graphql", graphql(name = "ActionCost"))]
pub enum ActionCost {
    Action,
    BonusAction,
    Reaction,
    Legendary,
    Free,
}

impl ActionCost {
    pub fn as_db_str(self) -> &'static str {
        match self {
            ActionCost::Action => "action",
            ActionCost::BonusAction => "bonus_action",
            ActionCost::Reaction => "reaction",
            ActionCost::Legendary => "legendary",
            ActionCost::Free => "free",
        }
    }

    /// An unrecognised stored value reads as an action, the default a row
    /// written before the column existed has.
    pub fn from_db_str(value: &str) -> Self {
        match value {
            "bonus_action" => ActionCost::BonusAction,
            "reaction" => ActionCost::Reaction,
            "legendary" => ActionCost::Legendary,
            "free" => ActionCost::Free,
            _ => ActionCost::Action,
        }
    }
}

/// An ability's or item's to-hit and damage formulas, out of its effects
/// (`(kind, formula)`): the first non-empty `attack_roll`, and every
/// non-empty `damage`, trimmed. Something with no attack roll cannot attack,
/// and the sentence says so.
pub fn attack_formulas(
    name: &str,
    effects: &[(String, String)],
) -> Result<(String, Vec<String>), String> {
    let to_hit = effects
        .iter()
        .find(|(kind, formula)| kind == "attack_roll" && !formula.trim().is_empty())
        .map(|(_, formula)| formula.trim().to_string())
        .ok_or_else(|| format!("{name} has no attack roll to make"))?;
    let damage = effects
        .iter()
        .filter(|(kind, formula)| kind == "damage" && !formula.trim().is_empty())
        .map(|(_, formula)| formula.trim().to_string())
        .collect();
    Ok((to_hit, damage))
}

/// Roll a formula. A formula that does not parse, or cannot be resolved, is
/// refused in one sentence ("Roll rejected: …"). Returns the resolution, to be
/// recorded, and its value: a total, or a count of successes.
pub fn roll<R: Rng>(
    source: &str,
    bindings: &PlaceholderBindings,
    rng: &mut R,
) -> Result<(RollResolution, f64), String> {
    let formula = DiceFormula::parse(source).map_err(|e| format!("Roll rejected: {e}"))?;
    let resolution = thunderforge_dice::resolve(&formula, bindings, rng)
        .map_err(|e| format!("Roll rejected: {e}"))?;
    let value = roll_value(&resolution);
    Ok((resolution, value))
}

/// A resolution's value: a total, or a count of successes.
pub fn roll_value(resolution: &RollResolution) -> f64 {
    match resolution.kind {
        ResolutionKind::Total(v) => v,
        ResolutionKind::SuccessCount(n) => n as f64,
    }
}

/// The outcome of a to-hit total against a target's defence: no target, a
/// target with no defence, a hit (total ≥ defence) or a miss.
pub fn judge(has_target: bool, defence: Option<i32>, total: f64) -> &'static str {
    match (has_target, defence) {
        (false, _) => OUTCOME_NO_TARGET,
        (true, None) => OUTCOME_NO_DEFENCE,
        (true, Some(defence)) if total >= defence as f64 => OUTCOME_HIT,
        (true, Some(_)) => OUTCOME_MISS,
    }
}

/// The one formula a part's damage is rolled as: its only formula, or each
/// in brackets, added. `None` with nothing to roll.
pub fn damage_source(damage: &[String]) -> Option<String> {
    match damage {
        [] => None,
        [only] => Some(only.clone()),
        many => Some(
            many.iter()
                .map(|f| format!("({f})"))
                .collect::<Vec<_>>()
                .join("+"),
        ),
    }
}

/// What a damage roll offers: rounded, never negative, within `i32`.
pub fn offered_amount(rolled: f64) -> i32 {
    rolled.round().max(0.0).min(i32::MAX as f64) as i32
}

/// Research R15, less what the caller has already settled (a named target
/// that was hit): the effective setting is on, no player controls the target,
/// and the attack could see it or does not need to.
pub fn auto_apply_holds(
    effective_setting: bool,
    any_player_controls_target: bool,
    flags: &[String],
    needs_line_of_sight: bool,
) -> bool {
    effective_setting
        && !any_player_controls_target
        && (!needs_line_of_sight || !flags.iter().any(|f| f == FLAG_NO_LINE_OF_SIGHT))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tie_hits() {
        assert_eq!(judge(true, Some(15), 15.0), OUTCOME_HIT);
        assert_eq!(judge(true, Some(15), 14.0), OUTCOME_MISS);
        assert_eq!(judge(true, None, 30.0), OUTCOME_NO_DEFENCE);
        assert_eq!(judge(false, Some(1), 30.0), OUTCOME_NO_TARGET);
    }

    #[test]
    fn several_damage_formulas_are_one_roll() {
        assert_eq!(damage_source(&[]), None);
        assert_eq!(damage_source(&["1d6+2".into()]).as_deref(), Some("1d6+2"));
        assert_eq!(
            damage_source(&["1d6".into(), "2d4".into()]).as_deref(),
            Some("(1d6)+(2d4)")
        );
    }

    #[test]
    fn an_ability_without_an_attack_roll_cannot_attack() {
        let effects = vec![("damage".to_string(), "1d6".to_string())];
        assert_eq!(
            attack_formulas("Shove", &effects),
            Err("Shove has no attack roll to make".to_string())
        );
    }

    #[test]
    fn an_offer_is_never_negative() {
        assert_eq!(offered_amount(-3.0), 0);
        assert_eq!(offered_amount(4.5), 5);
    }
}
