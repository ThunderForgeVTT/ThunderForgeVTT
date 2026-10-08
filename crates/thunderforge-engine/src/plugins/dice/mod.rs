//! A roll, thrown on the board (spec 083).
//!
//! The server has decided every value. `thunderforge_canvas_core::dice_throw`
//! decides how the decided values are shown, and is tested on the host; this
//! plugin only draws what it computes:
//!
//! - [`throw`]: the queue, and each throw's tumble, hold and fade;
//! - [`mesh`]: each die as a `Mesh2d`, rotated, culled and shaded on the CPU
//!   (research R1);
//! - [`readout`]: the face numbers, the struck values, the line of arithmetic
//!   and the `+N more` chip;
//! - [`probe`]: what the e2e reads back (`dice_landed()`).
//!
//! Every throw is a child of one [`DiceStage`], which follows the camera, so
//! a throw stays fixed on screen at any pan or zoom (research R8). The dice
//! carry none of the components the interaction systems hit-test, so a click
//! goes through them.

pub mod mesh;
pub mod probe;
pub mod readout;
pub mod throw;

use bevy::camera::visibility::VisibilitySystems;
use bevy::prelude::*;
use thunderforge_canvas_core::dice_throw::ThrowSpec;
use thunderforge_canvas_core::dice_throw::queue::ThrowQueue;

/// The stage's z: above the darkness sheet and the fog layer.
pub const STAGE_Z: f32 = 950.0;

pub struct DicePlugin;

impl Plugin for DicePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DiceInbox>()
            .init_resource::<DiceQueue>()
            .init_resource::<DiceMotion>()
            .init_resource::<DiceViewport>()
            .add_systems(Startup, spawn_stage)
            .add_systems(
                PostUpdate,
                (
                    follow_camera,
                    throw::take_inbox,
                    throw::play,
                    throw::draw,
                    probe::count_entities,
                )
                    .chain()
                    // Not at all while no throw is waiting, playing or
                    // fading. Even idle, these systems cost a 3200-token
                    // world its first load in the engine-limits sweep
                    // (2 to 9 fps there against 60 without them).
                    .run_if(dice_active)
                    // After every camera system in `Update`, and before
                    // transforms and visibility propagate, so the stage
                    // reaches the screen in the same frame as the camera.
                    .before(TransformSystems::Propagate)
                    .before(VisibilitySystems::VisibilityPropagate),
            );
    }
}

/// What the web sent since the last frame: throws to queue, and the
/// viewer's motion preference if it changed. `app.rs` writes it; the plugin
/// drains it.
#[derive(Resource, Default)]
pub struct DiceInbox {
    pub throws: Vec<ThrowSpec>,
    pub reduced: Option<bool>,
}

/// The playing, waiting and fading throws (research R7).
#[derive(Resource, Default)]
pub struct DiceQueue(pub ThrowQueue<throw::Throw>);

/// Whether the viewer prefers reduced motion. It applies to the next throw
/// that starts.
#[derive(Resource, Default)]
pub struct DiceMotion {
    pub reduced: bool,
}

/// The viewport's size in logical pixels, read from the camera each frame.
#[derive(Resource)]
pub struct DiceViewport(pub Vec2);

impl Default for DiceViewport {
    fn default() -> Self {
        Self(Vec2::new(1280.0, 720.0))
    }
}

/// The one entity every throw hangs from.
#[derive(Component)]
pub struct DiceStage;

/// On every entity a throw spawns, so `dice_entity_count()` can count them.
#[derive(Component)]
pub struct DiceThrowEntity;

/// Whether the dice systems have anything to do this frame: a command in
/// the inbox, a throw in the queue, or throw entities the probe has not yet
/// counted down to none.
fn dice_active(inbox: Res<DiceInbox>, queue: Res<DiceQueue>) -> bool {
    !inbox.throws.is_empty()
        || inbox.reduced.is_some()
        || !queue.0.is_empty()
        || probe::entity_count() > 0
}

fn spawn_stage(mut commands: Commands) {
    commands.spawn((
        DiceStage,
        Transform::from_xyz(0.0, 0.0, STAGE_Z),
        Visibility::Inherited,
    ));
}

/// Copies the camera's x and y into the stage, and its zoom into the stage's
/// scale, so the stage's children are in screen pixels.
fn follow_camera(
    cameras: Query<(&Camera, &Transform, &Projection), (With<Camera2d>, Without<DiceStage>)>,
    mut stage: Query<&mut Transform, With<DiceStage>>,
    mut viewport: ResMut<DiceViewport>,
) {
    let Some((camera, camera_transform, projection)) = cameras.iter().next() else {
        return;
    };
    let scale = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };
    if let Some(size) = camera.logical_viewport_size() {
        viewport.0 = size;
    }
    for mut transform in &mut stage {
        transform.translation = Vec3::new(
            camera_transform.translation.x,
            camera_transform.translation.y,
            STAGE_Z,
        );
        transform.scale = Vec3::new(scale, scale, 1.0);
    }
}
