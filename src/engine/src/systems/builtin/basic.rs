/// Basic Tabletop System
/// 
/// Minimal ruleset for generic tabletop gaming.
/// No predefined abilities or skills.
/// All tokens are treated equally.
/// Movement has 1:1 cost (no modifiers).
///
/// Use this as the default system when no game-specific rules are needed.

use crate::components::Token;
use crate::systems::core::{GameSystem, SkillDefinition, DerivedStats};

pub struct BasicSystem;

impl GameSystem for BasicSystem {
    fn id(&self) -> &'static str {
        "basic"
    }
    
    fn name(&self) -> &'static str {
        "Basic Tabletop"
    }
    
    fn ability_names(&self) -> Vec<&'static str> {
        // Basic system has no predefined abilities
        vec![]
    }
    
    fn skill_definitions(&self) -> Vec<SkillDefinition> {
        // Basic system has no predefined skills
        vec![]
    }
    
    fn validate_token(&self, _token: &Token) -> Result<(), String> {
        // Accept any token configuration
        Ok(())
    }
    
    fn calculate_derived_stats(&self, token: &Token) -> DerivedStats {
        // Calculate basic derived stats
        let health = token.health.unwrap_or(10);
        let armor_class = 10; // D&D baseline (neutral starting point)
        let initiative = 0; // No modifier
        
        DerivedStats {
            effective_health: health,
            armor_class,
            initiative,
            proficiency_bonus: None,
        }
    }
    
    fn calculate_movement_cost(&self, distance: f32) -> f32 {
        // 1:1 cost (no modifiers or special rules)
        distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_basic_system_id() {
        let system = BasicSystem;
        assert_eq!(system.id(), "basic");
    }
    
    #[test]
    fn test_basic_system_name() {
        let system = BasicSystem;
        assert_eq!(system.name(), "Basic Tabletop");
    }
    
    #[test]
    fn test_basic_system_no_abilities() {
        let system = BasicSystem;
        assert_eq!(system.ability_names().len(), 0);
    }
    
    #[test]
    fn test_basic_system_no_skills() {
        let system = BasicSystem;
        assert_eq!(system.skill_definitions().len(), 0);
    }
    
    #[test]
    fn test_basic_system_accepts_any_token() {
        let system = BasicSystem;
        let token = Token::default();
        assert!(system.validate_token(&token).is_ok());
    }
    
    #[test]
    fn test_basic_system_derived_stats() {
        let system = BasicSystem;
        let mut token = Token::default();
        token.health = Some(20);
        
        let stats = system.calculate_derived_stats(&token);
        assert_eq!(stats.effective_health, 20);
        assert_eq!(stats.armor_class, 10);
        assert_eq!(stats.initiative, 0);
        assert_eq!(stats.proficiency_bonus, None);
    }
    
    #[test]
    fn test_basic_system_movement_cost() {
        let system = BasicSystem;
        assert_eq!(system.calculate_movement_cost(10.0), 10.0);
        assert_eq!(system.calculate_movement_cost(5.5), 5.5);
        assert_eq!(system.calculate_movement_cost(0.0), 0.0);
    }

    // Phase 4.7.G1: Additional BasicSystem tests

    #[test]
    fn test_basic_system_movement_cost_large_values() {
        let system = BasicSystem;

        // Basic system should handle arbitrarily large values
        assert_eq!(system.calculate_movement_cost(1000.0), 1000.0);
        assert_eq!(system.calculate_movement_cost(10000.5), 10000.5);
    }

    #[test]
    fn test_basic_system_movement_cost_negative() {
        let system = BasicSystem;

        // Negative distances (moving backward)
        assert_eq!(system.calculate_movement_cost(-10.0), -10.0);
    }

    #[test]
    fn test_basic_system_derived_stats_default_token() {
        let system = BasicSystem;
        let token = Token::default();

        let stats = system.calculate_derived_stats(&token);
        assert_eq!(stats.effective_health, 10);  // Default health
        assert_eq!(stats.armor_class, 10);
        assert_eq!(stats.initiative, 0);
        assert_eq!(stats.proficiency_bonus, None);
    }

    #[test]
    fn test_basic_system_derived_stats_zero_health() {
        let system = BasicSystem;
        let mut token = Token::default();
        token.health = Some(0);

        let stats = system.calculate_derived_stats(&token);
        assert_eq!(stats.effective_health, 0);
    }

    #[test]
    fn test_basic_system_derived_stats_high_health() {
        let system = BasicSystem;
        let mut token = Token::default();
        token.health = Some(1000);

        let stats = system.calculate_derived_stats(&token);
        assert_eq!(stats.effective_health, 1000);
    }

    #[test]
    fn test_basic_system_consistency() {
        let system = BasicSystem;

        // Multiple calls should return identical results
        let stats1 = system.calculate_derived_stats(&Token::default());
        let stats2 = system.calculate_derived_stats(&Token::default());

        assert_eq!(stats1.effective_health, stats2.effective_health);
        assert_eq!(stats1.armor_class, stats2.armor_class);
        assert_eq!(stats1.initiative, stats2.initiative);
    }
}
