//! Spec 084: 5e's roll facets. Advantage and disadvantage, Halfling Luck,
//! Great Weapon Fighting, and the resources a d20 test can be rerolled by.
//!
//! The host asks; this decides. Formulas are rewritten through the dice
//! crate's AST, never by string edits (research R2).

use thunderforge_canvas_core::roll_facets::{
    Advantage, FacetLabel, RerollInput, RerollPlan, RollFacets, RollKind, ShapeInput, Shaped,
};
use thunderforge_dice::{rewrite_dice_terms, AddModifier, TermEdit};

/// The refusal for advantage on a roll with no plain d20 in it.
pub const NO_D20: &str = "This roll has no d20 to roll twice.";

/// The refusal for advantage on damage, which only a host bug can send.
pub const NO_DAMAGE_ADVANTAGE: &str = "A damage roll is never rolled with advantage.";

/// What a roll can be rerolled by, in the order the table is offered it.
pub const SPENDS: &[FacetLabel] = &[
    FacetLabel {
        id: "inspiration",
        label: "Heroic Inspiration",
    },
    FacetLabel {
        id: "luck_point",
        label: "Luck Point",
    },
];

/// Every facet id this pack names (data-model.md).
pub const LABELS: &[FacetLabel] = &[
    FacetLabel {
        id: "advantage",
        label: "Advantage",
    },
    FacetLabel {
        id: "disadvantage",
        label: "Disadvantage",
    },
    FacetLabel {
        id: "halfling_luck",
        label: "Halfling Luck",
    },
    FacetLabel {
        id: "great_weapon_fighting",
        label: "Great Weapon Fighting",
    },
    FacetLabel {
        id: "lucky",
        label: "Lucky",
    },
    FacetLabel {
        id: "inspiration",
        label: "Heroic Inspiration",
    },
    FacetLabel {
        id: "luck_point",
        label: "Luck Point",
    },
];

/// 5e's slot, registered from `lib.rs`.
pub static ROLL_FACETS: RollFacets = RollFacets {
    shape,
    reroll,
    spends: SPENDS,
    labels: LABELS,
};

/// Whether the sheet's `trait_data.facets` lists `id`.
pub fn has_facet(trait_data: &serde_json::Value, id: &str) -> bool {
    trait_data
        .get("facets")
        .and_then(|facets| facets.as_array())
        .is_some_and(|facets| facets.iter().any(|f| f.as_str() == Some(id)))
}

/// The formula to roll and the facets that shaped it, or `None` to roll the
/// formula as declared.
pub fn shape(input: &ShapeInput<'_>) -> Result<Option<Shaped>, String> {
    match input.kind {
        RollKind::Check | RollKind::ToHit => shape_d20(input),
        RollKind::Damage if input.advantage != Advantage::Normal => {
            Err(NO_DAMAGE_ADVANTAGE.to_string())
        }
        RollKind::Damage => Ok(None),
    }
}

/// The first d20 term that keeps nothing takes the choice, and Halfling Luck
/// rerolls its natural ones once.
fn shape_d20(input: &ShapeInput<'_>) -> Result<Option<Shaped>, String> {
    let keep = match input.advantage {
        Advantage::Normal => None,
        Advantage::Advantage => Some(("advantage", AddModifier::KeepHighest(1))),
        Advantage::Disadvantage => Some(("disadvantage", AddModifier::KeepLowest(1))),
    };
    let lucky = has_facet(input.trait_data, "halfling_luck");
    if keep.is_none() && !lucky {
        return Ok(None);
    }
    let mut facets = Vec::new();
    let mut found = false;
    let formula = rewrite_dice_terms(input.formula, |term| {
        if found || term.sides != Some(20) || term.keeps {
            return None;
        }
        found = true;
        let mut edit = TermEdit::default();
        if let Some((id, modifier)) = keep {
            edit.count = Some(term.count.unwrap_or(1).max(2));
            edit.add.push(modifier);
            facets.push(id.to_string());
        }
        if lucky && !term.rerolls {
            edit.add.push(AddModifier::RerollOnceEq(1));
            facets.push("halfling_luck".to_string());
        }
        Some(edit)
    })
    .map_err(|error| error.to_string())?;
    if !found {
        // A halfling's roll with no plain d20 is simply rolled as written.
        return match keep {
            Some(_) => Err(NO_D20.to_string()),
            None => Ok(None),
        };
    }
    if facets.is_empty() {
        return Ok(None);
    }
    Ok(Some(Shaped { formula, facets }))
}

/// The rerolls arrive with US3; until then every spend is refused.
pub fn reroll(input: &RerollInput<'_>) -> Result<RerollPlan, String> {
    Err(format!(
        "This system has no reroll called \"{}\".",
        input.spend
    ))
}

#[cfg(test)]
#[path = "roll_facets_tests.rs"]
mod tests;
