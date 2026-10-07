use crate::resources::{SceneData, SceneGrid, SelectedToken, TokenGridBehaviour};
use crate::{ActiveWorld, TOKEN_SIZE, TokenIdentity, emit_event};
use bevy::prelude::*;
use serde_json::json;
use thunderforge_canvas_core::grid::Footprint;

/// One full breath of the active token's ring, in seconds.
pub(crate) const RING_PULSE_SECONDS: f32 = 1.4;

/// Warm gold: distinct from the planned route's cyan, the selected light's
/// colour and the grid, so a selection never reads as scene geometry.
const RING_COLOR: Color = Color::srgb(1.0, 0.82, 0.35);

/// How far outside the token's own square the ring sits, as a fraction of
/// half its side. Outside, so the ring never covers the token's art.
const RING_CLEARANCE: f32 = 1.12;

/// How much the active ring grows at the top of its breath.
const RING_SWELL: f32 = 0.08;

/// One selection ring as drawn: where it sits from the token's centre and
/// what colour it is this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectionRing {
    pub radius: f32,
    pub color: Color,
}

/// The ring for a selected token whose square is `side` wide, `seconds`
/// into the app's life.
///
/// The way an MMO marks a target: the active token — the selection's primary,
/// whose properties every panel shows — has a ring that breathes, so the eye
/// finds it among the rest; every other selected token has a steady, fainter
/// ring, so a picked-up stack still shows every member it holds.
pub(crate) fn selection_ring(primary: bool, side: f32, seconds: f32) -> SelectionRing {
    let rest = side * 0.5 * RING_CLEARANCE;
    if !primary {
        return SelectionRing {
            radius: rest,
            color: RING_COLOR.with_alpha(0.45),
        };
    }
    // 0 at the bottom of the breath, 1 at the top.
    let breath = (seconds / RING_PULSE_SECONDS * std::f32::consts::TAU).sin() * 0.5 + 0.5;
    SelectionRing {
        radius: rest * (1.0 + RING_SWELL * breath),
        color: RING_COLOR.with_alpha(0.65 + 0.35 * breath),
    }
}

/// Whether a selected token is drawn with a ring at all.
///
/// Not when the viewer is not shown it. Lighting hides an unlit token from a
/// player by `Visibility::Hidden`, and a ring around that empty patch of
/// darkness would say exactly what the darkness hides — the same reading
/// `camera_focus` and the context menu's hit test make of it.
pub(crate) fn is_ringed(visibility: &Visibility) -> bool {
    *visibility != Visibility::Hidden
}

/// Draws a ring around every selected token the viewer can see.
///
/// Gizmos: immediate-mode lines, nothing spawned and nothing to despawn, and
/// no allocation per frame — the selection is walked in place and each token
/// found through `TokenEntities` rather than by scanning the board.
pub(crate) fn draw_selection_rings(
    selected: Res<SelectedToken>,
    token_entities: Option<Res<crate::TokenEntities>>,
    tokens: Query<(&Transform, &Visibility, Option<&TokenGridBehaviour>)>,
    grid: Option<Res<SceneGrid>>,
    time: Res<Time>,
    mut gizmos: Gizmos,
) {
    let Some(token_entities) = token_entities else {
        return;
    };
    let seconds = time.elapsed_secs();
    for (index, token_id) in selected.selected_ids().iter().enumerate() {
        let Some(&entity) = token_entities.0.get(token_id) else {
            continue;
        };
        let Ok((transform, visibility, behaviour)) = tokens.get(entity) else {
            continue;
        };
        if !is_ringed(visibility) {
            continue;
        }
        let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
        let side = grid
            .as_ref()
            .map_or(TOKEN_SIZE.y, |grid| footprint.world_size(grid.size))
            * transform.scale.x.abs().max(f32::EPSILON);
        let ring = selection_ring(index == 0, side, seconds);
        gizmos
            .circle_2d(transform.translation.truncate(), ring.radius, ring.color)
            .resolution(48);
    }
}

/// Update token visual feedback based on selection state: opacity and a
/// z-order bump here; the ring is `draw_selection_rings`.
pub(crate) fn render_selection_feedback(
    mut sprite_query: Query<(&TokenIdentity, &mut Sprite, &mut Transform)>,
    selected_token: Res<SelectedToken>,
) {
    // On the token layer, where everything else assumes tokens are. They
    // used to be pinned at z 1 and 2 — under shapes, walls, and (playtest
    // 2026-09-10 P9) the darkness, which is drawn over the map and would
    // have covered every token in a dark scene.
    let layer = crate::resources::CanvasLayer::Tokens.z();
    for (identity, mut sprite, mut transform) in sprite_query.iter_mut() {
        // Selected token: opaque, on top. Unselected: slightly transparent.
        let (alpha, z) = if selected_token.is_selected(&identity.0) {
            (1.0, layer + 1.0)
        } else {
            (0.85, layer)
        };
        // Compared before writing. This runs every frame over every token,
        // and an unconditional write marked every `Transform` and `Sprite`
        // changed every frame — so every system keyed on a token having moved
        // (snapping, the move detector, viewport culling) ran for the whole
        // board on every frame, and every sprite was re-extracted to the
        // render world, whether or not anything had happened.
        if sprite.color.alpha() != alpha {
            sprite.color = sprite.color.with_alpha(alpha);
        }
        if transform.translation.z != z {
            transform.translation.z = z;
        }
    }
}

/// Phase 4.7.E2: Keyboard-driven token movement
/// If a token is selected and an arrow key is pressed, move the token 1 grid cell
/// in that direction. Triggers optimistic update (visual feedback immediately)
/// with rollback placeholder for server rejection (Phase 4.6 integration).
pub(crate) fn handle_keyboard_token_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut token_query: Query<(&mut Transform, &TokenIdentity)>,
    selected_token: Res<SelectedToken>,
    scene: Res<SceneData>,
    active_world: Res<ActiveWorld>,
) {
    // Get selected token ID
    let Some(selected_id) = selected_token.get_selected() else {
        return;
    };

    // Determine movement direction (one grid cell per key press)
    let (dx, dy) = if keyboard.just_pressed(KeyCode::ArrowUp) {
        (0.0, 1.0) // Up (Y increases in Bevy world space)
    } else if keyboard.just_pressed(KeyCode::ArrowDown) {
        (0.0, -1.0) // Down
    } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
        (-1.0, 0.0) // Left
    } else if keyboard.just_pressed(KeyCode::ArrowRight) {
        (1.0, 0.0) // Right
    } else {
        return; // No movement key pressed
    };

    // Find and update selected token
    for (mut transform, identity) in token_query.iter_mut() {
        if identity.0 == *selected_id {
            transform.translation.x += dx * scene.grid_size;
            transform.translation.y += dy * scene.grid_size;

            emit_event(json!({
                "type": "upsert_token",
                "token": {
                    "id": identity.0,
                    "x": transform.translation.x,
                    "y": transform.translation.y,
                    "z": transform.translation.z,
                },
                "worldId": active_world.0,
            }));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The active token's ring breathes: its size changes over time.
    #[test]
    fn the_active_token_s_ring_pulses() {
        let a = selection_ring(true, 64.0, 0.0);
        let b = selection_ring(true, 64.0, RING_PULSE_SECONDS / 4.0);
        assert!((a.radius - b.radius).abs() > 0.5, "{a:?} vs {b:?}");
    }

    /// The rest of the selection is marked, but holds still and stays quieter
    /// than the active one at its faintest.
    #[test]
    fn other_selected_tokens_get_a_steady_quieter_ring() {
        let a = selection_ring(false, 64.0, 0.0);
        let b = selection_ring(false, 64.0, RING_PULSE_SECONDS / 4.0);
        assert_eq!(a, b);
        let faintest_primary = (0..16)
            .map(|step| selection_ring(true, 64.0, step as f32 * RING_PULSE_SECONDS / 16.0))
            .map(|ring| ring.color.alpha())
            .fold(f32::MAX, f32::min);
        assert!(a.color.alpha() < faintest_primary);
    }

    /// Both rings sit outside the token, so they never hide its art.
    #[test]
    fn a_ring_sits_outside_the_token() {
        for primary in [true, false] {
            for step in 0..16 {
                let ring = selection_ring(primary, 64.0, step as f32 * RING_PULSE_SECONDS / 16.0);
                assert!(ring.radius > 32.0, "{ring:?}");
            }
        }
    }

    /// A token the viewer is not shown is never ringed: a ring around empty
    /// darkness would say exactly what the darkness hides.
    #[test]
    fn a_token_the_viewer_cannot_see_is_never_ringed() {
        assert!(!is_ringed(&Visibility::Hidden));
        assert!(is_ringed(&Visibility::Inherited));
        assert!(is_ringed(&Visibility::Visible));
    }

    #[test]
    fn test_keyboard_token_movement_basic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.insert_resource(SceneData::new(
            "test".to_string(),
            "world-test".to_string(),
            crate::resources::GridType::Square,
            32.0,
            20,
            20,
            None,
        ));
        app.insert_resource(ActiveWorld("world-test".to_string()));
        app.init_resource::<SelectedToken>();
        app.init_resource::<ButtonInput<KeyCode>>();
        app.add_systems(Update, handle_keyboard_token_movement);

        let token = app
            .world_mut()
            .spawn((
                Transform::from_xyz(0.0, 0.0, 0.0),
                TokenIdentity("token-1".to_string()),
            ))
            .id();

        app.world_mut()
            .resource_mut::<SelectedToken>()
            .select("token-1".to_string());
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::ArrowRight);

        app.update();

        let transform = app.world().get::<Transform>(token).unwrap();
        assert_eq!(transform.translation.x, 32.0);
        assert_eq!(transform.translation.y, 0.0);
    }
}
