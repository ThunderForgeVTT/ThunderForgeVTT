// cypher System Models
// Base data structures, derived from research/system_cypher.json.
// Stored in world_actor_system_data JSONB columns.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityData {
    pub might: i64,
    pub speed: i64,
    pub intellect: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceData {
    pub might_pool: i64,
    pub speed_pool: i64,
    pub intellect_pool: i64,
    pub effort: i64,
    pub xp: i64,
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
