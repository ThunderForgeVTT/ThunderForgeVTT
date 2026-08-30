//! Values computed from a token rather than sent for it.
//!
//! # What this deliberately does not do
//!
//! It used to compute armour class as `10 + (dex - 10) / 2`, initiative as
//! the same expression, a proficiency bonus of 2, and a movement speed of 30
//! — D&D 5e's rules, in the engine, applied to every token in every system.
//! Two of the four rulesets that ship have no dexterity at all: Genie has
//! might, cunning and spirit; Blades in the Dark has insight, prowess and
//! resolve. Those systems got `None` for everything, and the two that did fit
//! got numbers nothing ever displayed.
//!
//! It computed at all only because nothing populated the scores it read. Now
//! that attributes are plumbed through, the same code would start producing
//! one ruleset's answers for all of them — which is why it is gone rather
//! than adapted.
//!
//! What remains is what holds regardless of ruleset: a proportion of a pool,
//! and whether that pool is empty or full. Everything genuinely system-
//! specific waits on where system rules should execute, which MVP.md Phase 8
//! records as unsettled.

use crate::components::*;
use bevy::prelude::*;

/// Recompute a token's derived values when the token changes.
pub fn calculate_derived_stats(mut query: Query<(&Token, &mut DerivedStats), Changed<Token>>) {
    for (token, mut derived) in query.iter_mut() {
        *derived = compute_derived_stats(token);
    }
}

/// Everything derivable without knowing the ruleset.
pub fn compute_derived_stats(token: &Token) -> DerivedStats {
    let mut stats = DerivedStats::default();

    if let (Some(health), Some(max_health)) = (token.health, token.max_health) {
        if max_health > 0 {
            stats.health_percentage = Some((health as f32 / max_health as f32) * 100.0);
        }
        stats.is_dead = health <= 0;
        stats.is_full_health = health >= max_health;
    }

    stats
}

/// Health as a percentage of its maximum.
pub fn compute_health_percentage(health: Option<i32>, max_health: Option<i32>) -> Option<f32> {
    match (health, max_health) {
        (Some(h), Some(max)) if max > 0 => Some((h as f32 / max as f32) * 100.0),
        _ => None,
    }
}

/// Whether a token is down.
pub fn is_token_dead(health: Option<i32>) -> bool {
    match health {
        Some(h) => h <= 0,
        // Not every token tracks health, and one that does not is not dead.
        None => false,
    }
}

/// Whether a token is unhurt.
pub fn is_token_full_health(health: Option<i32>, max_health: Option<i32>) -> bool {
    match (health, max_health) {
        (Some(h), Some(max)) => h >= max,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: these compile but do not execute — the engine crate has no test
    // runner for wasm32 (Constitution V). The rules worth covering live in
    // `thunderforge-canvas-core`, where tests run.

    #[test]
    fn health_percentage_needs_a_maximum_above_zero() {
        assert_eq!(compute_health_percentage(Some(5), Some(10)), Some(50.0));
        assert_eq!(compute_health_percentage(Some(5), Some(0)), None);
        assert_eq!(compute_health_percentage(None, Some(10)), None);
    }

    #[test]
    fn a_token_that_tracks_no_health_is_not_dead() {
        assert!(!is_token_dead(None));
        assert!(is_token_dead(Some(0)));
        assert!(!is_token_dead(Some(1)));
    }

    #[test]
    fn full_health_is_assumed_when_it_cannot_be_determined() {
        assert!(is_token_full_health(None, None));
        assert!(is_token_full_health(Some(10), Some(10)));
        assert!(!is_token_full_health(Some(9), Some(10)));
    }
}
