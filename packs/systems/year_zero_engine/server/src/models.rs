// yze System Models
// Base data structures, derived from research/system_yze.json.
// Stored in world_actor_system_data JSONB columns.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityData {
    pub strength: i64,
    pub agility: i64,
    pub wits: i64,
    pub empathy: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceData {
    pub health: i64,
    pub resolve: i64,
    pub stress: i64,
    pub experience_points: i64,
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
