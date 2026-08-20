//! Token ECS Components and Bundles
//!
//! Defines the core ECS components for game tokens on the canvas.
//! All components are designed for WASM compatibility and follow the
//! circular event-driven architecture pattern.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Unique identifier for a token
/// Used to correlate Bevy entities with backend token IDs
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TokenId(pub String);

/// Base token data (transmitted from server)
/// Contains only core game data, never derived/computed values
#[derive(Component, Clone, Debug, Serialize, Deserialize, Default)]
pub struct Token {
    /// Server-side token ID (UUID)
    pub id: String,
    
    /// World this token belongs to
    pub world_id: String,
    
    /// Scene this token is placed on
    pub scene_id: String,
    
    /// Token type (character, npc, object, etc.)
    pub token_type: String,
    
    /// Label or name (e.g., "Goblin #1", "Player Character")
    pub label: Option<String>,
    
    /// Grid X position (logical game board coordinate)
    pub base_x: i32,
    
    /// Grid Y position (logical game board coordinate)
    pub base_y: i32,
    
    /// Width in grid units (e.g., 1 for 1x1, 2 for 2x2)
    pub size_x: i32,
    
    /// Height in grid units
    pub size_y: i32,
    
    /// Color tint for visual distinction (team indicators, etc.)
    pub color: Color,
    
    /// Visibility flag (for fog of war, Phase 4.7.1)
    pub is_visible: bool,
    
    /// Base health value
    pub health: Option<i32>,
    
    /// Maximum health
    pub max_health: Option<i32>,
    
    /// Raw ability scores (from system or server)
    pub abilities: TokenAbilities,
    
    /// Schema version for migrations
    pub schema_version: i32,
    
    /// Selection state (Phase 4.7.E1)
    pub is_selected: bool,
    
    /// Hover state for tooltips/feedback (Phase 4.7.E1)
    pub is_hovered: bool,
}

/// Ability scores (D&D-style or system-specific)
#[derive(Component, Clone, Debug, Serialize, Deserialize, Default)]
pub struct TokenAbilities {
    pub strength: Option<i32>,
    pub dexterity: Option<i32>,
    pub constitution: Option<i32>,
    pub intelligence: Option<i32>,
    pub wisdom: Option<i32>,
    pub charisma: Option<i32>,
}

/// Grid position of the token (x, y coordinates on the game board)
#[derive(Component, Clone, Debug, Copy, PartialEq)]
pub struct GridPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32, // Layer/depth
}

impl GridPosition {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    
    pub fn distance_to(&self, other: GridPosition) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Rollback cache for optimistic updates
/// Stores the last server-approved position and state
/// If a mutation is rejected, we restore from this cache
#[derive(Component, Clone, Debug, Copy)]
pub struct RollbackCache {
    /// Last server-approved position
    pub last_server_position: GridPosition,
    
    /// Last server-approved health
    pub last_server_health: Option<i32>,
    
    /// Whether we're waiting for server confirmation
    pub is_pending: bool,
    
    /// ID of the mutation we're waiting for (used for matching responses)
    pub pending_mutation_id: Option<i64>,
}

impl RollbackCache {
    pub fn new(position: GridPosition, health: Option<i32>) -> Self {
        Self {
            last_server_position: position,
            last_server_health: health,
            is_pending: false,
            pending_mutation_id: None,
        }
    }
}

/// Derived stats computed from base token data
/// Never transmitted over network; calculated locally to save bandwidth
#[derive(Component, Clone, Debug, Default)]
pub struct DerivedStats {
    /// Calculated armor class (from system hook or base computation)
    pub armor_class: Option<i32>,
    
    /// Initiative modifier
    pub initiative: Option<i32>,
    
    /// Health as percentage (0-100)
    pub health_percentage: Option<f32>,
    
    /// Is token dead/unconscious
    pub is_dead: bool,
    
    /// Is token at full health
    pub is_full_health: bool,
    
    /// Movement speed (tiles per round)
    pub movement_speed: Option<i32>,
    
    /// Proficiency bonus (D&D-style)
    pub proficiency_bonus: Option<i32>,
    
    /// System-specific computed values (JSON)
    /// For extensibility with game systems
    pub custom: Option<serde_json::Value>,
}

impl DerivedStats {
    /// Calculate derived stats from base token data
    /// This is called after receiving token data from server
    pub fn calculate(token: &Token) -> Self {
        let mut stats = DerivedStats::default();
        
        // Health percentage
        if let (Some(health), Some(max_health)) = (token.health, token.max_health) {
            if max_health > 0 {
                stats.health_percentage = Some((health as f32 / max_health as f32) * 100.0);
            }
            stats.is_dead = health <= 0;
            stats.is_full_health = health >= max_health;
        }
        
        // Base AC calculation (can be overridden by system hooks)
        // Default: 10 + DEX modifier
        if let Some(dex) = token.abilities.dexterity {
            let dex_mod = (dex - 10) / 2;
            stats.armor_class = Some(10 + dex_mod);
            stats.initiative = Some(dex_mod);
        }
        
        // Default movement speed
        stats.movement_speed = Some(30); // 30 feet = 5 tiles per round (D&D default)
        
        stats
    }
}

/// Complete token entity bundle
/// Combines all components needed for a renderable, interactive token
#[derive(Bundle)]
pub struct TokenBundle {
    /// Visual representation
    pub sprite: Sprite,
    
    /// Position and orientation
    pub transform: Transform,
    
    /// Global visibility
    pub visibility: Visibility,
    
    /// Backend token metadata
    pub token: Token,
    
    /// Unique ID
    pub token_id: TokenId,
    
    /// Position on the game board
    pub grid_position: GridPosition,
    
    /// Rollback cache for optimistic updates
    pub rollback_cache: RollbackCache,
    
    /// Computed statistics
    pub derived_stats: DerivedStats,
}

impl TokenBundle {
    /// Create a new token bundle with default values
    pub fn new(
        token: Token,
        position: GridPosition,
        color: Color,
        size: Vec2,
    ) -> Self {
        let token_id = TokenId(token.id.clone());
        
        // Create rollback cache from current state
        let rollback_cache = RollbackCache::new(position, token.health);
        
        // Calculate initial derived stats
        let derived_stats = DerivedStats::calculate(&token);
        
        // Create sprite
        let sprite = Sprite::from_color(color, size);
        
        // Create transform from grid position
        let transform = Transform::from_xyz(position.x, position.y, position.z);
        
        Self {
            sprite,
            transform,
            visibility: Visibility::Visible,
            token,
            token_id,
            grid_position: position,
            rollback_cache,
            derived_stats,
        }
    }
}

/// Marker component for player-controlled tokens
#[derive(Component)]
pub struct PlayerControlled;

/// Marker component for tokens that are currently selected
#[derive(Component)]
pub struct Selected;

/// Marker component for tokens that are performing an action
#[derive(Component)]
pub struct IsActing;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_position_distance() {
        let pos1 = GridPosition::new(0.0, 0.0, 0.0);
        let pos2 = GridPosition::new(3.0, 4.0, 0.0);
        
        assert_eq!(pos1.distance_to(pos2), 5.0);
    }

    #[test]
    fn test_derived_stats_calculation() {
        let token = Token {
            id: "test".to_string(),
            world_id: "world".to_string(),
            label: Some("Test Token".to_string()),
            health: Some(50),
            max_health: Some(100),
            abilities: TokenAbilities {
                dexterity: Some(14),
                ..Default::default()
            },
            schema_version: 1,
            ..Default::default()
        };

        let stats = DerivedStats::calculate(&token);
        
        assert_eq!(stats.health_percentage, Some(50.0));
        assert_eq!(stats.is_dead, false);
        assert_eq!(stats.is_full_health, false);
        assert_eq!(stats.armor_class, Some(12)); // 10 + 2 (DEX mod)
        assert_eq!(stats.initiative, Some(2));
    }

    #[test]
    fn test_rollback_cache() {
        let pos = GridPosition::new(10.0, 20.0, 0.0);
        let cache = RollbackCache::new(pos, Some(100));
        
        assert_eq!(cache.last_server_position, pos);
        assert_eq!(cache.last_server_health, Some(100));
        assert_eq!(cache.is_pending, false);
    }
}
