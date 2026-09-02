//! Phase 4.9.D.2.3: Conflict Visualization System
//!
//! Provides visual feedback when token conflicts occur:
//! - Red tint overlay
//! - Fade-out animation over 2 seconds
//! - Optional conflict marker/label

use bevy::prelude::*;

use crate::systems::event_dispatcher::WorldEventQueue;

/// Component: Marks a token as having a conflict
#[derive(Component, Clone, Debug)]
pub struct ConflictMarker {
    pub started_at: f64,
    pub duration: f64, // Fade-out duration in seconds
    pub original_color: Color,
}

impl ConflictMarker {
    pub fn new(current_time: f64) -> Self {
        Self {
            started_at: current_time,
            duration: 2.0,
            original_color: Color::srgb(1.0, 1.0, 1.0),
        }
    }

    /// Get alpha value for fade-out (1.0 → 0.0 over duration)
    pub fn get_alpha(&self, current_time: f64) -> f32 {
        let elapsed = current_time - self.started_at;
        let alpha = 1.0 - (elapsed / self.duration).clamp(0.0, 1.0);
        alpha.max(0.0) as f32
    }

    /// Check if conflict indicator should be removed
    pub fn is_expired(&self, current_time: f64) -> bool {
        current_time - self.started_at > self.duration
    }
}

/// System: Detect conflict events and mark tokens
pub fn mark_conflict_tokens(
    mut commands: Commands,
    mut query: Query<(Entity, &crate::components::Token)>,
    mut queue: ResMut<WorldEventQueue>,
    time: Res<Time>,
) {
    let events = queue.drain();

    for event in events {
        // Only process conflict events (code=2)
        if event.event_code != 2 {
            continue;
        }

        let token_id = match &event.token_id {
            Some(id) => id,
            None => continue,
        };

        eprintln!(
            "[Phase4.9.D🎨] Conflict marker: token={}, created_by={:?}",
            token_id, event.created_by
        );

        // Find token and attach conflict marker
        for (entity, token) in query.iter_mut() {
            if token.id == *token_id {
                commands
                    .entity(entity)
                    .insert(ConflictMarker::new(time.elapsed_secs() as f64));

                eprintln!(
                    "[Phase4.9.D⚠️] Attached conflict marker to token: {}",
                    token_id
                );

                break;
            }
        }
    }
}

/// System: Animate conflict indicators (fade-out and removal)
pub fn animate_conflict_indicators(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Sprite, &mut ConflictMarker)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs() as f64;

    for (entity, mut sprite, conflict) in query.iter_mut() {
        let alpha = conflict.get_alpha(current_time);

        if alpha <= 0.0 {
            // Remove expired conflict marker
            commands.entity(entity).remove::<ConflictMarker>();

            // Restore original color
            sprite.color = Color::srgb(1.0, 1.0, 1.0);

            eprintln!("[Phase4.9.D✅] Conflict marker expired, removed");
        } else {
            // Apply red tint with fading alpha
            sprite.color = Color::srgba(1.0, 0.5, 0.5, alpha);

            eprintln!("[Phase4.9.D🎨] Updating conflict color: alpha={:.2}", alpha);
        }
    }
}

/// System: Log all conflict events (for debugging)
pub fn log_conflict_events(query: Query<&ConflictMarker>) {
    for marker in query.iter() {
        eprintln!(
            "[Phase4.9.D🔴] Active conflict marker: started_at={:.2}, duration={:.2}",
            marker.started_at, marker.duration
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_marker_creation() {
        let marker = ConflictMarker::new(1.0);
        assert_eq!(marker.started_at, 1.0);
        assert_eq!(marker.duration, 2.0);
    }

    #[test]
    fn test_conflict_marker_alpha_calculation() {
        let marker = ConflictMarker::new(1.0);

        // At start: alpha = 1.0
        assert!((marker.get_alpha(1.0) - 1.0).abs() < 0.01);

        // At halfway: alpha = 0.5
        assert!((marker.get_alpha(2.0) - 0.5).abs() < 0.01);

        // At end: alpha = 0.0
        assert!((marker.get_alpha(3.0) - 0.0).abs() < 0.01);

        // Beyond end: alpha clamped to 0.0
        assert_eq!(marker.get_alpha(4.0), 0.0);
    }

    #[test]
    fn test_conflict_marker_expiration() {
        let marker = ConflictMarker::new(1.0);

        assert!(!marker.is_expired(2.5)); // 1.5s elapsed (< 2.0s)
        assert!(marker.is_expired(3.1)); // 2.1s elapsed (> 2.0s)
    }
}
