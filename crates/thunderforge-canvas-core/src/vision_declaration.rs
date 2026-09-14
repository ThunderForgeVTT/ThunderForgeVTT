//! How a game system says its creatures see (spec 045 US6, owner decision 2).
//!
//! `vision.rs` decides what a token can see given a profile. This decides
//! where that profile comes from: a system declares which of an actor's own
//! fields carry sight in darkness and the reach of a light the character
//! carries, and those numbers are resolved per token.
//!
//! # Why declared rather than built in
//!
//! Darkvision is a D&D word. Building it into the engine would make one
//! ruleset's vocabulary the shape every other system has to fit — and the
//! shared code would then have to be edited to support a system it had never
//! heard of, which is exactly what the pack architecture exists to prevent.
//! A system that declares nothing gets ordinary sight, which is correct
//! rather than a fallback.
//!
//! # Units are the system's, and the scene converts them
//!
//! A declaration says "60", in the system's own units. What that reaches on a
//! board depends on the scene's grid, and the conversion happens once, in
//! [`GridUnits::cells`]. A declaration that carried pixels would be a
//! declaration that only worked on one map.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::measure::GridUnits;
use crate::vision::VisionProfile;

/// Where one distance is read from an actor, and in what units.
///
/// `slot` and `source` together, exactly as a `sheet` entry names them: which
/// of the actor's stored blobs, and which key inside it. One way to say "read
/// this field", not two.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DistanceSource {
    /// Which of the actor's stored blobs to read — `traitData`,
    /// `abilityData`, and so on. Absent means `traitData`, where a creature's
    /// senses live in every system that has bothered to record them.
    #[serde(default)]
    pub slot: Option<String>,
    /// The key inside that blob.
    pub source: String,
    /// The system's own unit name — `feet`, `metres`. Recorded so a manifest
    /// says what it means; the scene's own `per_cell` does the conversion,
    /// because a scene that measures in metres measures *everything* in
    /// metres.
    #[serde(default)]
    pub unit: Option<String>,
    /// What to assume when the actor's sheet says nothing.
    ///
    /// Almost always absent. A default darkvision would give every creature
    /// that failed to mention it the ability to see in the dark, which is the
    /// same mistake a default flying speed would be.
    #[serde(default)]
    pub default: Option<f32>,
}

/// The bright and dim reach of a light a character carries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CarriedLightDeclaration {
    pub bright: Option<DistanceSource>,
    pub dim: Option<DistanceSource>,
}

/// A system's whole `vision` block. Every part optional; absent means
/// ordinary sight.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VisionDeclaration {
    /// How many of the system's own units one grid square is worth — 5, for
    /// D&D 5e's five-foot square.
    ///
    /// Declared here because it exists nowhere else yet. A scene's `grid_size`
    /// is **pixels** per cell, and no manifest records feet per square: the
    /// `movement` block declares "30" and leaves the unit implicit, which
    /// works only because nothing ever converts it. A distance that must be
    /// drawn cannot be left implicit, so this block says what its numbers
    /// mean. When `movement` needs the same answer, this is the declaration
    /// to lift out and share rather than duplicate.
    ///
    /// Absent falls back to five, which is right for the systems that
    /// measure in feet and harmless for the ones that declare no distances.
    #[serde(default)]
    pub units_per_cell: Option<f32>,
    /// The unit's own name, for anything that shows a number to a person.
    #[serde(default)]
    pub unit_label: Option<String>,
    pub darkvision: Option<DistanceSource>,
    pub carried_light: Option<CarriedLightDeclaration>,
}

impl VisionDeclaration {
    /// The scale this block's distances are quoted in.
    pub fn grid_units(&self) -> GridUnits {
        GridUnits::new(
            self.units_per_cell.unwrap_or(GridUnits::default().per_cell),
            self.unit_label
                .clone()
                .unwrap_or_else(|| GridUnits::default().label),
        )
    }
}

/// What one actor's sheet resolves to, in cells of the scene's grid.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ResolvedVision {
    /// Sight in darkness, in cells. Zero means none.
    pub darkvision: f32,
    /// The bright reach of a light this character carries, in cells.
    pub carried_bright: f32,
    /// Its dim reach, in cells. Zero for both means no carried light.
    pub carried_dim: f32,
}

impl ResolvedVision {
    /// Whether this is worth telling the engine about at all.
    ///
    /// A creature with no darkvision and no carried light sees by the default
    /// rules, and saying so explicitly is the same as saying nothing.
    pub fn is_ordinary(&self) -> bool {
        self.darkvision <= 0.0 && self.carried_bright <= 0.0 && self.carried_dim <= 0.0
    }

    /// As the engine's own profile, leaving facing and range alone.
    ///
    /// [`VisionProfile`] already carries a darkvision range and is already
    /// what `set_token_vision` takes — this fills in the one field a system
    /// declaration speaks to, rather than introducing a second shape for the
    /// same idea.
    pub fn profile(&self) -> VisionProfile {
        VisionProfile {
            darkvision: self.darkvision,
            ..VisionProfile::default()
        }
    }
}

/// Read a distance out of an actor's stored system data.
///
/// Accepts a bare number and the `{ "value": n }` shape a real sheet uses,
/// matching `movement_budget`'s reader — the sheets are the same sheets.
fn read_distance(slot: &serde_json::Value, source: &str) -> Option<f32> {
    let raw = slot.get(source)?;
    let raw = if raw.is_object() {
        raw.get("value")?
    } else {
        raw
    };
    let value = raw.as_f64()?;
    // Negative or non-finite is broken data, not a creature that sees
    // backwards. Treated as absent so the declaration's default can apply.
    (value.is_finite() && value >= 0.0).then_some(value as f32)
}

/// The slot a declaration reads when it does not say.
pub const DEFAULT_SLOT: &str = "traitData";

/// An actor's stored system data, keyed by slot name.
///
/// A map rather than one blob because a declaration says which slot it reads,
/// and a system is free to keep its senses somewhere other than its traits.
pub type ActorSlotData = std::collections::BTreeMap<String, serde_json::Value>;

fn resolve_one(slots: &ActorSlotData, declared: Option<&DistanceSource>, units: &GridUnits) -> f32 {
    let Some(declared) = declared else {
        return 0.0;
    };
    let in_system_units = slots
        .get(declared.slot.as_deref().unwrap_or(DEFAULT_SLOT))
        .and_then(|slot| read_distance(slot, &declared.source))
        .or(declared.default)
        .unwrap_or(0.0);
    units.cells(in_system_units)
}

/// Resolve one actor's vision from its stored system data and its system's
/// declaration, in cells.
///
/// Returns cells rather than pixels because a cell is what both sides of this
/// already agree on: the caller multiplies by the scene's cell size to draw.
/// Putting pixels here would bake one zoom into a stored answer.
pub fn vision_from(
    slots: &ActorSlotData,
    declaration: &VisionDeclaration,
    units: &GridUnits,
) -> ResolvedVision {
    let carried = declaration.carried_light.as_ref();
    ResolvedVision {
        darkvision: resolve_one(slots, declaration.darkvision.as_ref(), units),
        carried_bright: resolve_one(slots, carried.and_then(|c| c.bright.as_ref()), units),
        carried_dim: resolve_one(slots, carried.and_then(|c| c.dim.as_ref()), units),
    }
}

#[cfg(test)]
#[path = "vision_declaration_tests.rs"]
mod tests;
