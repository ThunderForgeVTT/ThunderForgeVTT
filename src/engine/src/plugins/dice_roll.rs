use bevy::prelude::*;

/// Spec 014 (US4, research.md §6): renders/animates dice already
/// resolved by the server's `rollDice` mutation. This plugin never calls
/// `thunderforge_dice::resolve()` itself and never guesses an
/// outcome — it only ever settles visuals onto values it was handed
/// (Constitution Principle I: the outcome is decided by the
/// simulation/server boundary, not drawn speculatively; FR-015).
///
/// Independently addable/removable per Constitution Principle II:
/// `apply_external_commands` in lib.rs degrades gracefully (a
/// `trigger_dice_roll` command is simply dropped) if this plugin isn't
/// registered, same pattern as `BackgroundPlugin`/`WallPlugin`.
pub struct DiceRollPlugin;

impl Plugin for DiceRollPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PendingDiceRoll>()
            .add_systems(Update, (spawn_pending_roll, settle_rolling_dice));
    }
}

/// One die's already-resolved outcome, handed in from the `rollDice`
/// GraphQL response via `apply_world_command`'s `trigger_dice_roll`
/// command (lib.rs).
#[derive(Debug, Clone)]
pub struct DiceRollDie {
    pub final_value: i64,
}

/// Set by `lib.rs`'s `apply_external_commands` whenever a
/// `trigger_dice_roll` command arrives; drained (and cleared) by
/// `spawn_pending_roll` on the very next frame.
#[derive(Resource, Default)]
pub struct PendingDiceRoll(pub Option<Vec<DiceRollDie>>);

/// Marks an entity as one animated die from the most recent roll. Its
/// real, already-known final value is rendered as a `Text2d` child the
/// moment it spawns (`spawn_pending_roll`) — this component only tracks
/// the settle-animation's own progress, it never computes a value.
#[derive(Component)]
struct RollingDie {
    elapsed: f32,
}

/// How long the "dice bouncing" reveal takes before the die's face is
/// considered settled (SC-004: a few seconds total including
/// animation). A fixed, generous default — the exact tuning is a visual
/// design decision, not product behavior (spec.md's Assumptions).
const SETTLE_DURATION_SECS: f32 = 1.2;

const DIE_SIZE: Vec2 = Vec2::new(48.0, 48.0);
const DIE_SPACING: f32 = 64.0;

fn spawn_pending_roll(
    mut commands: Commands,
    mut pending: ResMut<PendingDiceRoll>,
    existing: Query<Entity, With<RollingDie>>,
) {
    let Some(dice) = pending.0.take() else {
        return;
    };

    // A new roll replaces whatever was still animating — the server's
    // response is always the newest authoritative state.
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    let count = dice.len() as f32;
    let start_x = -(count - 1.0) * DIE_SPACING / 2.0;

    for (i, die) in dice.into_iter().enumerate() {
        let x = start_x + (i as f32) * DIE_SPACING;
        commands
            .spawn((
                Sprite {
                    color: Color::srgb(0.9, 0.85, 0.2),
                    custom_size: Some(DIE_SIZE),
                    ..default()
                },
                Transform::from_xyz(x, 0.0, 900.0),
                RollingDie { elapsed: 0.0 },
            ))
            .with_children(|parent| {
                // The die's real, already-resolved face value — shown
                // from the moment it spawns, exactly like every other
                // `DieOutcome.final_value` from the `rollDice` response
                // (FR-015: this label is never a guess, only a rendering
                // of an already-known value).
                parent.spawn((
                    Text2d::new(die.final_value.to_string()),
                    TextFont { font_size: 20.0, ..default() },
                    TextColor(Color::BLACK),
                    Transform::from_xyz(0.0, 0.0, 1.0),
                ));
            });
    }
}

/// Settles each die's wobble/spin down to rest — a simple visual reveal
/// rather than full rigid-body physics, the face value itself already
/// (tasks.md's Implementation Strategy: acceptable so long as every
/// die visibly lands on its exact resolved value, which this does).
fn settle_rolling_dice(mut query: Query<(&mut Transform, &mut RollingDie)>, time: Res<Time>) {
    for (mut transform, mut die) in query.iter_mut() {
        die.elapsed += time.delta_secs();
        let progress = (die.elapsed / SETTLE_DURATION_SECS).min(1.0);

        // Spins down to rest — a die still "rolling" until progress
        // reaches 1.0, at which point it's visually settled on its
        // final value (represented by scale returning to 1.0; the
        // actual numeric face is exposed to the DOM/React layer via the
        // `rollDice` response itself, not read back out of the canvas).
        let wobble = (1.0 - progress) * (die.elapsed * 10.0).sin() * 0.15;
        transform.scale = Vec3::splat(1.0 + wobble);
        transform.rotation = Quat::from_rotation_z((1.0 - progress) * die.elapsed * 6.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_roll_defaults_to_none() {
        let pending = PendingDiceRoll::default();
        assert!(pending.0.is_none());
    }
}
