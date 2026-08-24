// pathfinder2e System Models
// Base data structures, derived from research/system_pathfinder2e.json.
// Stored in world_actor_system_data JSONB columns.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbilityData {
    pub strength: i64,
    pub dexterity: i64,
    pub constitution: i64,
    pub intelligence: i64,
    pub wisdom: i64,
    pub charisma: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceData {
    pub current_hp: i64,
    pub max_hp: i64,
    pub focus_points: i64,
    pub hero_points: i64,
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
