//! How many squares a creature fills (spec 046 FR-030, research R10).
//!
//! A pack declares its sizes in `combat.sizes`: where a creature's size is
//! kept (`source`) and how many squares a side each size fills. The server's
//! `combat::size` finds which record a token's size is read from; this is what
//! the record's value means.

use crate::manifest::SystemSizes;

/// A creature of no known size fills one square.
pub const DEFAULT_FOOTPRINT: f32 = 1.0;

/// The footprint a slot's value names, by a system's declared sizes.
pub fn footprint_from(sizes: &SystemSizes, slot: Option<&serde_json::Value>) -> f32 {
    slot.and_then(|slot| slot.get(&sizes.source.field))
        .and_then(|value| value.as_str())
        .and_then(|id| sizes.categories.iter().find(|c| c.id == id))
        .map(|category| category.footprint)
        .filter(|footprint| footprint.is_finite() && *footprint > 0.0)
        .unwrap_or(DEFAULT_FOOTPRINT)
}
