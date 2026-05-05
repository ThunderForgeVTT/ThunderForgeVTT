use bevy::prelude::*;
use crate::resources::{SceneData, GridType};

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
    scene: Res<SceneData>,
) {
    match scene.grid_type {
        GridType::Gridless => return,
        GridType::Hexagonal => return, // TODO: Phase 4.8
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
