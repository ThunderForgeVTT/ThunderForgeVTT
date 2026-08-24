// Genie System Models
// Base data structures for Genie characters and NPCs.
// Stored in world_actor_system_data JSONB columns (per spec 018 data-model.md).

use serde::{Deserialize, Serialize};

/// Genie ability scores (data-model.md: Genie Character / Genie NPC `ability_data`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenieAbilityData {
    pub might: i64,
    pub cunning: i64,
    pub spirit: i64,
}

/// Genie resource pool: Wish Points and Health (data-model.md `resource_data`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenieResourceData {
    pub current_wish_points: i64,
    pub max_wish_points: i64,
    pub current_health: i64,
    pub max_health: i64,
}

/// Genie skill training flags (data-model.md `proficiency_data`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenieProficiencyData {
    #[serde(default)]
    pub trained_skills: Vec<String>,
}

/// Active conditions plus (for player characters only) a Patron/lineage link
/// (data-model.md `condition_data` / `patron_lore_entry_id`) — reused as the
/// registry's `trait_data` slot (see loader.rs), since the shared
/// `SystemValidators` struct has no dedicated condition-data slot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenieTraitData {
    #[serde(default)]
    pub active_conditions: Vec<String>,
    #[serde(default)]
    pub patron_lore_entry_id: Option<String>,
    /// NPC-only (data-model.md `size_category`); absent/None for player characters.
    #[serde(default)]
    pub size_category: Option<String>,
}
