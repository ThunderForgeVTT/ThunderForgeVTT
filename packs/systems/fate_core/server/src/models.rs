// fate System Models
// Base data structures, derived from research/system_fate.json.
// Stored in world_actor_system_data JSONB columns.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityData {
    // No fixed ability scores for this system (see research digest).
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceData {
    pub fate_points: i64,
    pub refresh: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProficiencyData {
    #[serde(default)]
    pub trained_skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraitData {
    #[serde(default)]
    pub notes: Vec<String>,
}
