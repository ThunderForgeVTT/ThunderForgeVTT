//! Keyboard token movement, with a plan-then-commit path for turn-based play.
//!
//! # The gesture
//!
//! - **WASD / arrows** — move the owned token one cell immediately.
//! - **Shift + WASD / arrows** — extend a planned route instead of moving.
//!   The route draws as a line with its cost in the scene's own units.
//! - **Space** — commit the plan: the token moves to the end of the route.
//! - **Escape** — discard it.
//!
//! Planning exists because a move costs a resource the player is budgeting.
//! Seeing the route and its price before paying is the entire point; a token
//! that teleports on keypress gives them nothing to reason about.
//!
//! # Whose token
//!
//! Only the token this client owns. Movement is authored from an ownership
//! position, not a selection one — a player nudging the arrow keys must never
//! move the monster they happen to have clicked on.

use bevy::prelude::*;

use crate::movement::PlayerControlled;
use crate::resources::{SceneGrid, TokenGridBehaviour};
use crate::{ActiveWorld, TokenIdentity, emit_event};
use serde_json::json;
use thunderforge_canvas_core::grid::GridKind;
use thunderforge_canvas_core::measure::GridUnits;
use thunderforge_canvas_core::movement::{PlannedPath, Step};

/// The scene's distance vocabulary — 5 ft, 1.5 m, 1 Unit.
#[derive(Resource, Clone, Debug, Default, Deref, DerefMut)]
pub struct SceneUnits(pub GridUnits);

/// The route currently being planned, if any.
#[derive(Resource, Default, Debug)]
pub struct MovementPlan {
    pub path: Option<PlannedPath>,
}

/// Marks the entities drawing the current plan's distance label.
#[derive(Component)]
pub(crate) struct PlanLabel;

fn pressed_step(keyboard: &ButtonInput<KeyCode>) -> Option<Step> {
    // `just_pressed`, not `pressed`: a movement step is a discrete action. Held
    // keys repeating every frame would run a token off the board in under a
    // second and make a planned route impossible to author.
    if keyboard.just_pressed(KeyCode::KeyW) || keyboard.just_pressed(KeyCode::ArrowUp) {
        Some(Step::North)
    } else if keyboard.just_pressed(KeyCode::KeyS) || keyboard.just_pressed(KeyCode::ArrowDown) {
        Some(Step::South)
    } else if keyboard.just_pressed(KeyCode::KeyA) || keyboard.just_pressed(KeyCode::ArrowLeft) {
        Some(Step::West)
    } else if keyboard.just_pressed(KeyCode::KeyD) || keyboard.just_pressed(KeyCode::ArrowRight) {
        Some(Step::East)
    } else {
        None
    }
}

fn shift_held(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ShiftLeft) || keyboard.pressed(KeyCode::ShiftRight)
}

/// Handles movement input for the owned token.
pub(crate) fn handle_token_movement_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    grid: Res<SceneGrid>,
    mut plan: ResMut<MovementPlan>,
    active_world: Res<ActiveWorld>,
    mut owned: Query<
        (&mut Transform, &TokenIdentity, Option<&TokenGridBehaviour>),
        With<PlayerControlled>,
    >,
) {
    let Ok((mut transform, identity, behaviour)) = owned.single_mut() else {
        // No owned token — nothing to move. A spectator or GM view lands here.
        return;
    };

    let footprint = behaviour.copied().unwrap_or_default().footprint;
    let current = transform.translation.truncate();

    if keyboard.just_pressed(KeyCode::Escape) {
        plan.path = None;
        return;
    }

    // Commit.
    if keyboard.just_pressed(KeyCode::Space) {
        if let Some(path) = plan.path.take() {
            if !path.is_empty() {
                let destination = grid.cell_center(path.head());
                let snapped = grid.snap_footprint(destination, footprint);
                transform.translation.x = snapped.x;
                transform.translation.y = snapped.y;

                emit_event(json!({
                    "type": "update_token",
                    "tokenId": identity.0,
                    "changes": { "x": snapped.x, "y": snapped.y },
                    "worldId": active_world.0,
                    // The route, so a server that cares can validate the path
                    // rather than only the endpoint — teleporting through a
                    // wall and walking around it end in the same place.
                    "pathCells": path
                        .steps
                        .iter()
                        .map(|cell| json!([cell.q, cell.r]))
                        .collect::<Vec<_>>(),
                }));
            }
        }
        return;
    }

    let Some(step) = pressed_step(&keyboard) else {
        return;
    };

    // Gridless scenes have no cells to step between, so keyboard movement
    // falls back to nudging by the nominal cell size rather than doing nothing.
    if grid.kind == GridKind::Gridless {
        let nudge = match step {
            Step::North => Vec2::new(0.0, grid.size),
            Step::South => Vec2::new(0.0, -grid.size),
            Step::East => Vec2::new(grid.size, 0.0),
            Step::West => Vec2::new(-grid.size, 0.0),
        };
        transform.translation += nudge.extend(0.0);
        return;
    }

    if shift_held(&keyboard) {
        let path = plan
            .path
            .get_or_insert_with(|| PlannedPath::new(grid.world_to_cell(current)));
        path.push(step, grid.kind);
        return;
    }

    // Unmodified: move now, and abandon any plan — the player has clearly
    // stopped planning.
    plan.path = None;
    let next = step.apply(grid.world_to_cell(current), grid.kind);
    let snapped = grid.snap_footprint(grid.cell_center(next), footprint);
    transform.translation.x = snapped.x;
    transform.translation.y = snapped.y;

    emit_event(json!({
        "type": "update_token",
        "tokenId": identity.0,
        "changes": { "x": snapped.x, "y": snapped.y },
        "worldId": active_world.0,
    }));
}

/// Colour of the planned route. Distinct from the grid and from light colours
/// so a plan never reads as scene geometry.
const PLAN_COLOR: Color = Color::srgba(0.45, 0.85, 1.0, 0.95);

/// Draws the planned route and its cost.
pub(crate) fn draw_movement_plan(
    plan: Res<MovementPlan>,
    grid: Res<SceneGrid>,
    units: Res<SceneUnits>,
    mut commands: Commands,
    labels: Query<Entity, With<PlanLabel>>,
    mut gizmos: Gizmos,
) {
    // The label is a spawned entity rather than a gizmo because gizmos cannot
    // draw text. Despawned and respawned on change, which is cheap for one
    // entity and avoids tracking its state.
    for entity in labels.iter() {
        commands.entity(entity).despawn();
    }

    let Some(path) = plan.path.as_ref() else {
        return;
    };
    if path.is_empty() {
        return;
    }

    let points = path.world_points(&grid);
    gizmos.linestrip_2d(points.clone(), PLAN_COLOR);

    // A marker at every cell the route passes through, so a doubled-back path
    // is readable rather than a line drawn over itself.
    for point in &points {
        gizmos.circle_2d(*point, grid.size * 0.12, PLAN_COLOR);
    }

    // Destination ring, larger, so the endpoint is unambiguous.
    if let Some(end) = points.last() {
        gizmos.circle_2d(*end, grid.size * 0.42, PLAN_COLOR);

        commands.spawn((
            Text2d::new(units.format(path.cost_in_cells())),
            TextFont {
                // Scaled to the grid so the label stays readable at any zoom
                // and any cell size.
                font_size: (grid.size * 0.28).clamp(12.0, 48.0),
                ..default()
            },
            TextColor(PLAN_COLOR),
            Transform::from_translation((*end + Vec2::new(0.0, grid.size * 0.62)).extend(90.0)),
            PlanLabel,
        ));
    }
}
