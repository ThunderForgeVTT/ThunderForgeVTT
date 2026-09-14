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
//! Hit points went too (spec 046, ADR-102): a proportion of `Token.health`
//! was computed here and read by nothing, while the bars were drawn from the
//! server's `tokenStatus`. A creature's hit points have one record, and it is
//! not on the engine's token. What is left is the hook, for whatever
//! ruleset-independent value is derived next.

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
    DerivedStats::calculate(token)
}
