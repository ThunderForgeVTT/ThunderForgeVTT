//! Spec 084 research R6: the item properties a pack declares, such as 5e's
//! Two-Handed.
//!
//! The vocabulary is the pack's (`itemProperties` in its `system.json`).
//! Shared code stores and checks the ids, and never reads one: only the
//! pack's own `roll_facets` knows what `two_handed` does. A manifest without
//! the key declares none, so the other packs need no change.

/// One property an item may be marked with.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct ItemPropertyDeclaration {
    pub id: String,
    pub label: String,
}

/// The refusal for properties on an ability: only an item has them.
pub const ONLY_AN_ITEM: &str = "Only an item has properties.";

/// The properties a system declares, read from its own pack, in the
/// pack's order. An absent or unreadable manifest declares none.
pub fn item_properties_for_system(
    systems_dir: &str,
    system_id: &str,
) -> Vec<ItemPropertyDeclaration> {
    let path = std::path::Path::new(systems_dir)
        .join(system_id)
        .join("system.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    manifest
        .get("itemProperties")
        .cloned()
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default()
}

/// `given`, without repeats, when every id is one `declared` names;
/// otherwise the contract's sentence for the first that is not.
pub fn checked_properties(
    declared: &[ItemPropertyDeclaration],
    given: &[String],
) -> Result<Vec<String>, String> {
    let mut kept: Vec<String> = Vec::new();
    for id in given {
        if !declared.iter().any(|property| &property.id == id) {
            return Err(format!("This system has no item property \"{id}\"."));
        }
        if !kept.contains(id) {
            kept.push(id.clone());
        }
    }
    Ok(kept)
}

#[cfg(test)]
#[path = "item_properties_tests.rs"]
mod tests;
