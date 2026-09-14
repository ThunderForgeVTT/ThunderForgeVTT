//! Reading a system's `combat` block (spec 046).
//!
//! Shaped like `vision_profiles.rs` and `attributes.rs`: a `_for_system` that
//! reads `system.json` under the systems directory, and a `_from_manifest`
//! that can be tested without a filesystem. The difference is a cache. A hit
//! point changes far more often than a sheet is opened, and the manifest a
//! running server was started with does not change under it, so a system's
//! block is read once per (directory, system) and kept.
//!
//! Absent, unreadable or malformed all read as "no combat block" (M1).
//! Install-time validation (`pack_system_spec::combat`) is where a malformed
//! block is reported to the person who wrote it; at the moment somebody is
//! hit, the honest answer is "this system declares no hit points", said
//! plainly by the caller.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

pub use pack_system_spec::combat::{SystemCombat, SystemHitPoints, SystemTurnBudget, slot_key};

type Key = (String, String);

fn cache() -> &'static Mutex<HashMap<Key, Arc<SystemCombat>>> {
    static CACHE: OnceLock<Mutex<HashMap<Key, Arc<SystemCombat>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// A system's `combat` block, read once and kept.
pub fn combat_for_system(systems_dir: &str, system_id: &str) -> Arc<SystemCombat> {
    let key = (systems_dir.to_string(), system_id.to_string());
    if let Some(found) = cache().lock().ok().and_then(|map| map.get(&key).cloned()) {
        return found;
    }

    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let block = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .map(|manifest| combat_from_manifest(&manifest))
        .unwrap_or_default();
    let block = Arc::new(block);

    if let Ok(mut map) = cache().lock() {
        map.insert(key, block.clone());
    }
    block
}

/// The same, from a manifest already in hand.
pub fn combat_from_manifest(manifest: &serde_json::Value) -> SystemCombat {
    manifest
        .get("combat")
        .and_then(|block| serde_json::from_value::<SystemCombat>(block.clone()).ok())
        .unwrap_or_default()
}

/// A system's turn budget, if it declares one. Not cached: read when a
/// combatant is added or a turn passes, not on every hit.
pub fn turn_budget_from_manifest(manifest: &serde_json::Value) -> Option<SystemTurnBudget> {
    manifest
        .get("turnStructure")
        .and_then(|t| t.get("budget"))
        .and_then(|b| serde_json::from_value::<SystemTurnBudget>(b.clone()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_manifest_without_a_combat_block_declares_nothing() {
        let block = combat_from_manifest(&json!({ "id": "blades" }));
        assert!(block.hit_points.is_none());
        assert!(block.defence.is_none());
    }

    #[test]
    fn a_malformed_block_reads_as_absent_rather_than_failing() {
        let block = combat_from_manifest(&json!({ "combat": { "hitPoints": 7 } }));
        assert!(block.hit_points.is_none());
    }

    #[test]
    fn hit_points_are_read_as_declared() {
        let block = combat_from_manifest(&json!({ "combat": { "hitPoints": {
            "slot": "resourceData", "current": "hp", "max": "hp_max"
        }}}));
        let hp = block.hit_points.expect("declared");
        assert_eq!(slot_key(&hp.slot), "resource_data");
        assert_eq!(hp.current, "hp");
        assert_eq!(hp.max, "hp_max");
        assert_eq!(hp.temporary, None);
    }

    #[test]
    fn the_bundled_dnd5e_pack_declares_its_hit_points() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../packs/systems");
        let block = combat_for_system(dir, "dnd5e");
        let hp = block.hit_points.clone().expect("5e declares hit points");
        assert_eq!(hp.current, "current_hp");
        assert_eq!(hp.max, "max_hp");
        assert_eq!(hp.temporary.as_deref(), Some("temporary_hp"));
        // Read once: the second call is the same allocation.
        assert!(Arc::ptr_eq(&block, &combat_for_system(dir, "dnd5e")));
    }

    #[test]
    fn a_system_that_is_not_there_declares_nothing() {
        let block = combat_for_system("/nonexistent", "nothing");
        assert!(block.hit_points.is_none());
    }

    #[test]
    fn a_budget_is_read_from_turn_structure() {
        let budget = turn_budget_from_manifest(&json!({ "turnStructure": {
            "rounds": true, "budget": { "action": 1, "movement": { "speed": "walk" } }
        }}))
        .expect("declared");
        assert_eq!(budget.action, Some(1));
        assert_eq!(budget.movement.map(|m| m.speed).as_deref(), Some("walk"));
        assert!(
            turn_budget_from_manifest(&json!({ "turnStructure": { "rounds": true } })).is_none()
        );
    }
}
