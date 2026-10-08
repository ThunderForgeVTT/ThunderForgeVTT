//! Spec 084: the host's side of a system's roll facets.
//!
//! A system that registers [`RollFacets`] decides how its rolls are shaped
//! (advantage, a halfling's luck, a fighting style) and what a roll can be
//! rerolled by. The host asks it, through these functions, and never learns
//! what any facet means.

use thunderforge_canvas_core::roll_facets::{Advantage, RollFacets, RollKind, ShapeInput, Shaped};
use thunderforge_canvas_core::system_contribution::contribution_for;
use uuid::Uuid;

use crate::models::NewRollRecord;

/// What a roll was for, written to its record (data-model.md). The default
/// is a free roll: no sheet, no kind, no facets, not a reroll.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RollMeta {
    pub actor_id: Option<Uuid>,
    pub roll_kind: Option<RollKind>,
    pub check_id: Option<String>,
    pub facets: Vec<String>,
    pub reroll_of: Option<Uuid>,
    pub reroll_spent: Option<String>,
}

impl RollMeta {
    pub fn write_to(self, record: &mut NewRollRecord) {
        record.actor_id = self.actor_id;
        record.roll_kind = self.roll_kind.map(|kind| kind_name(kind).to_string());
        record.check_id = self.check_id;
        record.facets = self.facets.into_iter().map(Some).collect();
        record.reroll_of = self.reroll_of;
        record.reroll_spent = self.reroll_spent;
    }
}

/// A roll kind as `world_roll_records.roll_kind` stores it.
pub fn kind_name(kind: RollKind) -> &'static str {
    match kind {
        RollKind::Check => "check",
        RollKind::ToHit => "to_hit",
        RollKind::Damage => "damage",
    }
}

/// The roll kind a stored `roll_kind` names.
pub fn kind_of(name: &str) -> Option<RollKind> {
    match name {
        "check" => Some(RollKind::Check),
        "to_hit" => Some(RollKind::ToHit),
        "damage" => Some(RollKind::Damage),
        _ => None,
    }
}

/// The refusal for advantage on a system that has no roll facets.
pub const NO_ADVANTAGE: &str = "This system does not roll with advantage.";

/// The roll facets `system_id` registers, if it registers any.
pub fn facets_for(system_id: &str) -> Option<&'static RollFacets> {
    contribution_for(system_id).and_then(|pack| pack.roll_facets)
}

/// The formula to roll and the facets it carries.
///
/// A system without the slot, or `Ok(None)` from it, rolls `formula`
/// untouched. Without the slot, anything but `Normal` is refused.
pub fn shape_roll(system_id: &str, input: ShapeInput<'_>) -> Result<Shaped, String> {
    let untouched = |input: &ShapeInput<'_>| Shaped {
        formula: input.formula.to_string(),
        facets: Vec::new(),
    };
    let Some(facets) = facets_for(system_id) else {
        if input.advantage != Advantage::Normal {
            return Err(NO_ADVANTAGE.to_string());
        }
        return Ok(untouched(&input));
    };
    Ok((facets.shape)(&input)?.unwrap_or_else(|| untouched(&input)))
}

/// Each id with the name the table sees: a facet's label, a spend's label,
/// or the id itself when the system does not name it.
pub fn facet_labels(system_id: &str, ids: &[String]) -> Vec<(String, String)> {
    let facets = facets_for(system_id);
    ids.iter()
        .map(|id| {
            let label = facets
                .and_then(|f| f.labels.iter().chain(f.spends).find(|l| l.id == id))
                .map_or_else(|| id.clone(), |l| l.label.to_string());
            (id.clone(), label)
        })
        .collect()
}

#[cfg(test)]
#[path = "facets_tests.rs"]
mod tests;
