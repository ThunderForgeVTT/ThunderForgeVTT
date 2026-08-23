use crate::resources::{GridType, SceneData};
use bevy::prelude::*;

pub struct GridPlugin;

impl Plugin for GridPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_grid_lines);
    }
}

#[derive(Component)]
pub struct GridLine;

fn spawn_grid_lines(
    mut commands: Commands,
    // Pre-existing, unrelated bug fixed in passing (specs/002-canvas-
    // authoring-asset-storage T014): nothing in this crate ever inserts
    // `SceneData` (its own doc comment says it's meant to be "inserted at
    // startup" by something else, which was never wired up), so this
    // system — registered unconditionally in `Startup` — panicked with
    // "Resource does not exist" on every single page load, before any
    // canvas element was ever created. That blocked the entire canvas
    // from rendering at all, which in turn blocked every e2e scenario in
    // this feature (verified: reproduces identically on this branch with
    // all of today's wall/shape changes stashed out). Graceful
    // `Option`-degradation matches the pattern already used everywhere
    // else in this crate (e.g. `apply_external_commands` in lib.rs) for a
    // plugin/resource that may not exist yet.
    scene: Option<Res<SceneData>>,
) {
    let Some(scene) = scene else {
        return;
    };

    match scene.grid_type {
        GridType::Gridless => (),
        GridType::Hexagonal => (), // TODO: Phase 4.8
        GridType::Square => {
            // Draw vertical lines
            for x in 0..=scene.width {
                let px = x as f32 * scene.grid_size;
                let line_height = scene.pixel_height;

                // Vertical line (thin rectangle)
                commands.spawn((
                    Sprite {
                        color: Color::srgb(0.8, 0.8, 0.8),
                        custom_size: Some(Vec2::new(1.0, line_height)),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(px, scene.pixel_height / 2.0, 0.0)),
                    GridLine,
                ));
            }

            // Draw horizontal lines
            for y in 0..=scene.height {
                let py = scene.database_y_to_bevy_y(y as f32);
                let line_width = scene.pixel_width;

                // Horizontal line (thin rectangle)
                commands.spawn((
                    Sprite {
                        color: Color::srgb(0.8, 0.8, 0.8),
                        custom_size: Some(Vec2::new(line_width, 1.0)),
                        ..default()
                    },
                    Transform::from_translation(Vec3::new(line_width / 2.0, py, 0.0)),
                    GridLine,
                ));
            }
        }
    }
}
