//! Bevy ECS systems for token rendering and interaction in Phase 4.
//!
//! This module provides ECS components and systems for:
//! - Rendering world tokens as 2D sprites on the canvas
//! - Handling token selection and drag-to-move
//! - Optimistic updates with server rollback
//! - Syncing token state from GraphQL subscriptions

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Component: Represents a world token on the scene.
#[derive(Component, Debug, Clone, Serialize, Deserialize)]
pub struct WorldTokenComponent {
    pub token_id: String,
    pub world_id: String,
    pub label: Option<String>,
    pub health: Option<i32>,
    pub max_health: Option<i32>,
    pub created_by: String,
    pub updated_by: String,
    pub schema_version: i32,
    pub created_at: String,
    pub updated_at: String,

    // Optimistic update tracking
    pub is_optimistic: bool,
    pub last_server_x: f64,
    pub last_server_y: f64,
    pub last_server_z: f64,
}

/// Component: Derived stats computed locally (not sent over network).
#[derive(Component, Debug, Clone)]
pub struct DerivedTokenStats {
    pub effective_health: i32,
    pub health_percentage: f32,
}

impl DerivedTokenStats {
    /// Compute derived data from base token stats.
    pub fn prepare(token: &WorldTokenComponent) -> Self {
        let health = token.health.unwrap_or(0);
        let max_health = token.max_health.unwrap_or(1);
        let health_percentage = (health as f32 / max_health as f32) * 100.0;

        Self {
            effective_health: health,
            health_percentage,
        }
    }
}

/// Component: Selection state for tokens (used by UI and input systems).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSelectionState {
    NotSelected,
    Hovering,
    Selected,
    Dragging,
}

impl Default for TokenSelectionState {
    fn default() -> Self {
        Self::NotSelected
    }
}

/// System: Spawn a token entity from GraphQL data.
///
/// This system is called when a new token is created or received from the server.
/// It creates a Bevy entity with Transform, Sprite, and WorldTokenComponent.
pub fn spawn_token_system(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // This is a template function; actual spawning happens via event channels
    // when GraphQL mutations complete or subscriptions deliver new tokens
}

/// System: Update token positions when they change.
///
/// Listens for token update events and applies them to Transform components.
pub fn token_position_update_system(mut query: Query<(&mut Transform, &WorldTokenComponent)>) {
    for (mut transform, token) in query.iter_mut() {
        // Sync Transform.translation from token's (x, y, z) coordinates
        let Vec3 { x, y, z } = transform.translation;
        // x, y, z come from the GraphQL WorldToken model
        // Update transform only if out of sync
    }
}

/// System: Compute derived stats for all tokens.
///
/// Runs after token updates to recalculate derived data locally.
/// This keeps calculations off the network (no base64 encoding, no extra payloads).
pub fn compute_token_stats_system(mut query: Query<(&WorldTokenComponent, &mut DerivedTokenStats)>) {
    for (token, mut stats) in query.iter_mut() {
        *stats = DerivedTokenStats::prepare(token);
    }
}

/// System: Handle token selection via mouse input.
///
/// Uses raycasting to pick tokens under the cursor.
/// Updates TokenSelectionState component.
pub fn token_selection_input_system(
    mut query: Query<(&Transform, &mut TokenSelectionState), With<WorldTokenComponent>>,
    mouse_input: Res<ButtonInput<MouseButton>>,
    cursor_pos: Query<&Window>,
) {
    // Placeholder: In production, use bevy's picking backend or manual raycasting
    // to determine which token (if any) is under the cursor.
    for (transform, mut selection) in query.iter_mut() {
        if mouse_input.just_pressed(MouseButton::Left) {
            *selection = TokenSelectionState::Selected;
        } else if mouse_input.just_released(MouseButton::Left) {
            *selection = TokenSelectionState::NotSelected;
        }
    }
}

/// System: Handle token drag-to-move with grid snapping.
///
/// When a token is selected and the user drags, this system:
/// 1. Updates the token's position locally (optimistic)
/// 2. Emits a moveToken GraphQL mutation
/// 3. Listens for server response (subscription)
/// 4. On rejection, rolls back to last_server_* position
pub fn token_movement_system(
    mut query: Query<(&mut Transform, &mut WorldTokenComponent), With<TokenSelectionState>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
) {
    for (mut transform, mut token) in query.iter_mut() {
        // Placeholder: Implement drag logic here
        // 1. Check if token is in Dragging state
        // 2. Apply mouse_motion delta to transform.translation
        // 3. Snap to grid (grid_size parameter from scene)
        // 4. Store old position in last_server_* for rollback
        // 5. Emit mutation to GraphQL mutation queue
    }
}

/// System: Apply server updates from GraphQL subscriptions.
///
/// When worldEventCreated subscription delivers a token update,
/// this system applies the authoritative server state.
pub fn apply_server_updates_system(
    mut query: Query<&mut WorldTokenComponent>,
    // In production: mut subscription_rx: ResMut<broadcast::Receiver<WorldEvent>>
) {
    // Placeholder: Receive events from GraphQL subscription channel
    // For each worldEventCreated with token_event data:
    // - Find the corresponding entity by token_id
    // - Update position/health/etc.
    // - Set is_optimistic = false
}

/// System: Detect movement rejection and rollback.
///
/// If a moveToken mutation fails (e.g., invalid position), the server sends
/// a rejection event. This system rolls back the token to last_server_*.
pub fn token_rollback_system(
    mut query: Query<&mut Transform, (With<WorldTokenComponent>, With<TokenSelectionState>)>,
    // In production: mut rejection_rx: ResMut<broadcast::Receiver<RejectionEvent>>
) {
    // Placeholder: Listen for rejection events
    // When a moveToken is rejected:
    // - Find the token entity
    // - Restore transform.translation from last_server_x/y/z
    // - Mark as no longer optimistic
}

/// System: Render tokens as colored circles (placeholder).
///
/// In production, this would use sprite rendering or mesh rendering.
pub fn token_render_system(mut gizmos: Gizmos, query: Query<(&Transform, &WorldTokenComponent)>) {
    for (transform, token) in query.iter() {
        // Draw a circle at transform.translation with radius based on token size
        let pos = transform.translation;
        let color = if token.health.unwrap_or(0) <= 0 {
            Color::srgb(0.5, 0.5, 0.5) // Gray if dead
        } else {
            Color::srgb(0.2, 0.8, 0.2) // Green if alive
        };
        gizmos.circle(pos, Dir3::Z, 0.5, color);
    }
}

/// Plugin: Register all token systems and components.
pub struct TokenPlugin;

impl Plugin for TokenPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<WorldTokenComponent>()
            .register_type::<DerivedTokenStats>()
            .register_type::<TokenSelectionState>()
            .add_systems(
                Update,
                (
                    spawn_token_system,
                    token_position_update_system,
                    compute_token_stats_system,
                    token_selection_input_system,
                    token_movement_system,
                    apply_server_updates_system,
                    token_rollback_system,
                    token_render_system,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derived_stats_preparation() {
        let token = WorldTokenComponent {
            token_id: "test-token".to_string(),
            world_id: "world-1".to_string(),
            label: Some("Dragon".to_string()),
            health: Some(50),
            max_health: Some(100),
            created_by: "user-1".to_string(),
            updated_by: "user-1".to_string(),
            schema_version: 1,
            created_at: "2026-05-04".to_string(),
            updated_at: "2026-05-04".to_string(),
            is_optimistic: false,
            last_server_x: 0.0,
            last_server_y: 0.0,
            last_server_z: 0.0,
        };

        let stats = DerivedTokenStats::prepare(&token);
        assert_eq!(stats.effective_health, 50);
        assert!((stats.health_percentage - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_health_percentage_zero_health() {
        let token = WorldTokenComponent {
            token_id: "test".to_string(),
            world_id: "world-1".to_string(),
            label: None,
            health: Some(0),
            max_health: Some(100),
            created_by: "user-1".to_string(),
            updated_by: "user-1".to_string(),
            schema_version: 1,
            created_at: "2026-05-04".to_string(),
            updated_at: "2026-05-04".to_string(),
            is_optimistic: false,
            last_server_x: 0.0,
            last_server_y: 0.0,
            last_server_z: 0.0,
        };

        let stats = DerivedTokenStats::prepare(&token);
        assert_eq!(stats.health_percentage, 0.0);
    }
}
