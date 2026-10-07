//! A system's `combat` block and its turn budget (spec 046).
//!
//! The platform holds the concepts — hit points, a defence, a size, a
//! legendary pool, what a turn affords — and the pack says which of its own
//! fields they are. Shared code reads these declarations and never names a
//! system's fields itself (contract M5, `check-system-registry.mjs`).
//!
//! Every block is optional (M1). A pack that declares no `hitPoints` has no
//! damage operation; that is a correct answer for a ruleset without hit
//! points, not a gap to fill with a default.
//!
//! The schema types the shapes. What it cannot say is that a declaration
//! *points at something*: a `current` of `"curent_hp"` validates as a string
//! and then reads nothing, for ever, at the moment somebody is hit. So
//! [`validate_combat_content`] refuses a declaration whose slot or field is
//! absent from the pack's own `data_types` (M2), a size smaller than the
//! canvas can draw or a duplicated size id (M3), and a budget whose movement
//! names no declared speed (M4).
//!
//! The shapes themselves live in `thunderforge_combat::manifest` (spec 079,
//! ADR-113), because reading a manifest is the same answer on the server and
//! in the browser demo; they are re-exported here, with their `JsonSchema`,
//! so this crate's schema and everything that names them is unchanged.

pub use thunderforge_combat::manifest::{
    MIN_SIZE_FOOTPRINT, SystemBudgetMovement, SystemCombat, SystemDefence, SystemFieldRef,
    SystemHitPoints, SystemSizeCategory, SystemSizes, SystemTurnBudget, SystemTurnStructure,
    slot_key,
};

/// M2: `slot` is a data type the pack declares, and `field` one of its
/// properties.
pub(crate) fn require_field(
    data_types: Option<&serde_json::Value>,
    path: &str,
    slot: &str,
    field: &str,
) -> Result<(), String> {
    if field.trim().is_empty() {
        return Err(format!("{path} must name a field"));
    }
    let key = slot_key(slot);
    let Some(data_type) = data_types.and_then(|types| types.get(&key)) else {
        return Err(format!(
            "{path} names slot `{slot}`, which data_types does not declare"
        ));
    };
    let declared = data_type
        .get("properties")
        .and_then(|p| p.get(field))
        .is_some();
    if !declared {
        return Err(format!(
            "{path} names field `{field}`, which data_types.{key} does not declare"
        ));
    }
    Ok(())
}

/// Spec 046 M2–M4: a `combat` block or a turn budget that is present must
/// point at things the pack actually declares.
pub fn validate_combat_content(instance: &serde_json::Value) -> Result<(), String> {
    let data_types = instance.get("data_types");

    if let Some(block) = instance.get("combat").filter(|b| !b.is_null()) {
        let combat: SystemCombat =
            serde_json::from_value(block.clone()).map_err(|e| format!("combat: {e}"))?;

        if let Some(hp) = &combat.hit_points {
            require_field(
                data_types,
                "combat.hitPoints.current",
                &hp.slot,
                &hp.current,
            )?;
            require_field(data_types, "combat.hitPoints.max", &hp.slot, &hp.max)?;
            if let Some(temporary) = &hp.temporary {
                require_field(
                    data_types,
                    "combat.hitPoints.temporary",
                    &hp.slot,
                    temporary,
                )?;
            }
        }
        if let Some(defence) = &combat.defence {
            require_field(data_types, "combat.defence", &defence.slot, &defence.field)?;
        }
        if let Some(legendary) = &combat.legendary {
            require_field(
                data_types,
                "combat.legendary",
                &legendary.slot,
                &legendary.field,
            )?;
        }
        if let Some(sizes) = &combat.sizes {
            require_field(
                data_types,
                "combat.sizes.source",
                &sizes.source.slot,
                &sizes.source.field,
            )?;
            let mut seen = std::collections::HashSet::new();
            for category in &sizes.categories {
                if category.id.trim().is_empty() {
                    return Err("combat.sizes.categories[].id must not be empty".to_string());
                }
                if !seen.insert(category.id.as_str()) {
                    return Err(format!(
                        "combat.sizes.categories[].id `{}` is declared twice",
                        category.id
                    ));
                }
                if !(category.footprint.is_finite() && category.footprint >= MIN_SIZE_FOOTPRINT) {
                    return Err(format!(
                        "combat.sizes.categories[{}].footprint must be at least \
                         {MIN_SIZE_FOOTPRINT} (got {})",
                        category.id, category.footprint
                    ));
                }
            }
        }
    }

    let budget = instance
        .get("turnStructure")
        .and_then(|t| t.get("budget"))
        .filter(|b| !b.is_null());
    if let Some(budget) = budget {
        let budget: SystemTurnBudget = serde_json::from_value(budget.clone())
            .map_err(|e| format!("turnStructure.budget: {e}"))?;
        if let Some(movement) = &budget.movement {
            let declared = instance
                .get("movement")
                .and_then(|m| m.get(&movement.speed))
                .is_some();
            if !declared {
                return Err(format!(
                    "turnStructure.budget.movement.speed names `{}`, which the \
                     movement block does not declare",
                    movement.speed
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "combat_tests.rs"]
mod tests;
