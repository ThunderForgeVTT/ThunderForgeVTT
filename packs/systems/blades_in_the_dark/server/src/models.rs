// blades System Models
// Base data structures, derived from research/system_blades.json.
// Stored in world_actor_system_data JSONB columns.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityData {
    pub insight: i64,
    pub prowess: i64,
    pub resolve: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceData {
    pub stress: i64,
    pub trauma: i64,
    pub coin: i64,
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
