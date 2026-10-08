//! Each throw's life: queued, spawned under the stage, tumbled to the
//! server's faces, held with its readout, faded, despawned (research R3, R7,
//! R11). Only the moments come from here; every value and every path comes
//! from `dice_throw`.

use bevy::prelude::*;
use thunderforge_canvas_core::dice_throw::shapes::{self, Solid};
use thunderforge_canvas_core::dice_throw::tumble::{self, DIE_PX, DiePath};
use thunderforge_canvas_core::dice_throw::{
    self as model, ShapeKind, TIMINGS, ThrowDie, ThrowSpec, landing::landing,
};

use super::probe::{self, LandedDie, LandedThrow};
use super::readout::{self, ReadoutView, StruckView};
use super::{DiceInbox, DiceMotion, DiceQueue, DiceStage, DiceThrowEntity, DiceViewport, mesh};

/// A throw in the queue.
pub struct Throw {
    spec: ThrowSpec,
    dice: Vec<ThrowDie>,
    hidden: usize,
    reduced: bool,
    started_s: f64,
    view: Option<ThrowView>,
}

impl Throw {
    fn new(spec: ThrowSpec) -> Self {
        let (dice, hidden) = model::expand(&spec);
        Self {
            spec,
            dice,
            hidden,
            reduced: false,
            started_s: 0.0,
            view: None,
        }
    }

    /// When every drawn die shows its final value.
    fn lands_ms(&self) -> f64 {
        if self.reduced {
            f64::from(TIMINGS.reduced_ms)
        } else {
            f64::from(model::lands_ms(&self.dice))
        }
    }

    /// When it has faded out.
    fn gone_ms(&self) -> f64 {
        self.lands_ms() + f64::from(TIMINGS.hold_ms + TIMINGS.fade_ms)
    }

    fn at_ms(&self, now_s: f64) -> f64 {
        (now_s - self.started_s) * 1000.0
    }

    fn readout(&self) -> String {
        model::readout::readout(&self.spec)
    }

    fn chip(&self) -> Option<String> {
        model::readout::chip(self.dice.len(), self.dice.len() + self.hidden)
    }
}

/// What a throw spawned.
struct ThrowView {
    root: Entity,
    dice: Vec<DieView>,
    readout: ReadoutView,
}

struct DieView {
    entity: Entity,
    path: DiePath,
    subs: Vec<SubView>,
    struck: StruckView,
    /// What the die looks like during each of its segments.
    looks: Vec<Look>,
    dimmed: bool,
}

struct SubView {
    mesh: Handle<Mesh>,
    mesh_entity: Entity,
    number: Entity,
}

/// One segment's solids, each with its landed face and landing, and the
/// number each shows.
#[derive(Clone)]
struct Look {
    solids: Vec<(Solid, usize, Quat)>,
    labels: Vec<String>,
}

/// Drains what the web sent into the queue. A throw the queue pushes out is
/// written to the landed log as skipped: its roll is in the chat.
pub(super) fn take_inbox(
    mut inbox: ResMut<DiceInbox>,
    mut queue: ResMut<DiceQueue>,
    mut motion: ResMut<DiceMotion>,
) {
    if let Some(reduced) = inbox.reduced.take() {
        motion.reduced = reduced;
    }
    for spec in inbox.throws.drain(..) {
        if let Some(skipped) = queue.0.push(Throw::new(spec)) {
            info!("dice: skipped throw {}", skipped.spec.roll_id);
            probe::record(LandedThrow {
                roll_id: skipped.spec.roll_id,
                skipped: true,
                reduced_motion: skipped.reduced,
                readout: None,
                chip: None,
                dice: Vec::new(),
            });
        }
    }
}

/// Starts the playing throw, lands it, and despawns the faded ones.
#[allow(clippy::too_many_arguments)]
pub(super) fn play(
    mut commands: Commands,
    mut queue: ResMut<DiceQueue>,
    motion: Res<DiceMotion>,
    viewport: Res<DiceViewport>,
    time: Res<Time<Real>>,
    stage: Query<Entity, With<DiceStage>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut material: Local<Option<Handle<ColorMaterial>>>,
) {
    let Some(stage) = stage.iter().next() else {
        return;
    };
    let now = time.elapsed_secs_f64();
    let material = material
        .get_or_insert_with(|| materials.add(ColorMaterial::from_color(Color::WHITE)))
        .clone();
    while let Some(throw) = queue.0.playing.as_mut() {
        if throw.view.is_none() {
            throw.reduced = motion.reduced;
            throw.started_s = now;
            throw.view = Some(spawn(
                &mut commands,
                stage,
                throw,
                viewport.0,
                &mut meshes,
                &material,
            ));
        }
        if throw.at_ms(now) < throw.lands_ms() {
            break;
        }
        record_landed(throw);
        queue.0.landed();
    }
    for gone in queue.0.fade_done(|t| t.at_ms(now) >= t.gone_ms()) {
        if let Some(view) = gone.view {
            // Despawning the root takes its children and their mesh
            // handles with it, so the meshes are freed.
            commands.entity(view.root).despawn();
        }
    }
}

fn record_landed(throw: &Throw) {
    let Some(view) = throw.view.as_ref() else {
        return;
    };
    let dice = throw
        .dice
        .iter()
        .zip(&view.dice)
        .map(|(die, die_view)| {
            let sides = throw.spec.resolution.dice[die.outcome].sides;
            LandedDie::of(die, sides, die_view.path.rest)
        })
        .collect();
    probe::record(LandedThrow {
        roll_id: throw.spec.roll_id.clone(),
        skipped: false,
        reduced_motion: throw.reduced,
        readout: Some(throw.readout()),
        chip: throw.chip(),
        dice,
    });
}

fn is_flat(shape: ShapeKind) -> bool {
    matches!(shape, ShapeKind::Coin | ShapeKind::D2 | ShapeKind::Disc(_))
}

fn looks_of(die: &ThrowDie, path: &DiePath) -> Vec<Look> {
    let mut looks: Vec<Look> = Vec::new();
    for segment in &die.segments {
        let look = match (segment.tumble, looks.last()) {
            // A clamp swaps the number in place: the same solid, the same
            // pose, the clamped value.
            (false, Some(previous)) => {
                let mut look = previous.clone();
                for (k, label) in look.labels.iter_mut().enumerate() {
                    *label = if k == 0 {
                        segment.value.to_string()
                    } else {
                        String::new()
                    };
                }
                look
            }
            _ => {
                let solids: Vec<(Solid, usize, Quat)> = shapes::build(die.shape, segment.value)
                    .into_iter()
                    .enumerate()
                    .map(|(k, (solid, face))| {
                        let q = landing(&solid, face, path.spins[k.min(1)]);
                        (solid, face, q)
                    })
                    .collect();
                let labels = solids
                    .iter()
                    .map(|(solid, face, _)| solid.faces[*face].label.clone())
                    .collect();
                Look { solids, labels }
            }
        };
        looks.push(look);
    }
    looks
}

fn spawn(
    commands: &mut Commands,
    stage: Entity,
    throw: &Throw,
    viewport: Vec2,
    meshes: &mut Assets<Mesh>,
    material: &Handle<ColorMaterial>,
) -> ThrowView {
    let root = commands
        .spawn((
            DiceThrowEntity,
            Transform::from_translation(tumble::anchor(viewport).extend(0.0)),
            Visibility::Inherited,
            ChildOf(stage),
        ))
        .id();
    let seed = tumble::seed(&throw.spec.roll_id);
    let count = throw.dice.len();
    let dice = throw
        .dice
        .iter()
        .enumerate()
        .map(|(i, die)| {
            let mut path = tumble::die_path(seed, i, count, viewport.y);
            if is_flat(die.shape) {
                // A disc flips end over end rather than tumbling.
                path.axis = Vec3::X;
            }
            let looks = looks_of(die, &path);
            let subs_count = looks.iter().map(|l| l.solids.len()).max().unwrap_or(1);
            let entity = commands
                .spawn((
                    DiceThrowEntity,
                    Transform::from_xyz(path.entry.x, path.entry.y, i as f32),
                    Visibility::Hidden,
                    ChildOf(root),
                ))
                .id();
            let subs = (0..subs_count)
                .map(|k| {
                    let handle = meshes.add(mesh::empty());
                    let mesh_entity = commands
                        .spawn((
                            DiceThrowEntity,
                            Mesh2d(handle.clone()),
                            MeshMaterial2d(material.clone()),
                            Transform::from_xyz(sub_offset(k, subs_count), 0.0, 0.0),
                            ChildOf(entity),
                        ))
                        .id();
                    let number = readout::spawn_face_number(commands, mesh_entity);
                    SubView {
                        mesh: handle,
                        mesh_entity,
                        number,
                    }
                })
                .collect();
            let struck = readout::spawn_struck(commands, entity);
            DieView {
                entity,
                path,
                subs,
                struck,
                looks,
                dimmed: !die.kept || die.succeeded == Some(false),
            }
        })
        .collect();
    let chip = throw.chip();
    let readout = readout::spawn_readout(
        commands,
        root,
        &throw.readout(),
        chip.as_deref(),
        throw.dice.len(),
    );
    ThrowView {
        root,
        dice,
        readout,
    }
}

/// A d100's tens die sits left of its units die.
fn sub_offset(k: usize, count: usize) -> f32 {
    if count < 2 {
        0.0
    } else {
        (k as f32 - (count as f32 - 1.0) / 2.0) * DIE_PX * 1.05
    }
}

/// The throw's opacity at `t` ms: a fade in under reduced motion, and the
/// fade out after the hold.
fn alpha(throw: &Throw, t: f64) -> f32 {
    let fade_in = if throw.reduced {
        (t / f64::from(TIMINGS.reduced_ms)).clamp(0.0, 1.0)
    } else {
        1.0
    };
    let out_from = throw.lands_ms() + f64::from(TIMINGS.hold_ms);
    let fade_out = 1.0 - ((t - out_from) / f64::from(TIMINGS.fade_ms)).clamp(0.0, 1.0);
    (fade_in * fade_out) as f32
}

/// The entities a throw's frame touches.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ThrowParts<'w, 's> {
    transforms: Query<'w, 's, &'static mut Transform, (With<DiceThrowEntity>, Without<DiceStage>)>,
    visibility: Query<'w, 's, &'static mut Visibility, With<DiceThrowEntity>>,
    texts: Query<'w, 's, (&'static mut Text2d, &'static mut TextColor)>,
    sprites: Query<'w, 's, &'static mut Sprite, With<DiceThrowEntity>>,
}

/// Poses every drawn die of every live throw for this frame.
pub(super) fn draw(
    queue: Res<DiceQueue>,
    viewport: Res<DiceViewport>,
    time: Res<Time<Real>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: ThrowParts,
) {
    let now = time.elapsed_secs_f64();
    let live = queue.0.playing.iter().chain(queue.0.fading.iter());
    for throw in live {
        let Some(view) = throw.view.as_ref() else {
            continue;
        };
        let t = throw.at_ms(now);
        let a = alpha(throw, t);
        if let Ok(mut transform) = parts.transforms.get_mut(view.root) {
            transform.translation = tumble::anchor(viewport.0).extend(0.0);
        }
        for (die, die_view) in throw.dice.iter().zip(&view.dice) {
            draw_die(throw, die, die_view, t, a, &mut meshes, &mut parts);
        }
        let landed = t >= throw.lands_ms();
        for &entity in &view.readout.entities {
            show(&mut parts, entity, landed);
            set_alpha(&mut parts, entity, a);
        }
    }
}

fn draw_die(
    throw: &Throw,
    die: &ThrowDie,
    view: &DieView,
    t: f64,
    a: f32,
    meshes: &mut Assets<Mesh>,
    parts: &mut ThrowParts,
) {
    let t = t as f32;
    if !throw.reduced && t < die.enters_ms() as f32 {
        show(parts, view.entity, false);
        return;
    }
    show(parts, view.entity, true);
    let last = die.segments.len().saturating_sub(1);
    let index = if throw.reduced {
        last
    } else {
        die.segments
            .iter()
            .rposition(|s| s.starts_ms as f32 <= t)
            .unwrap_or(0)
    };
    let segment = &die.segments[index];
    let tumbling = !throw.reduced && segment.tumble && t < segment.lands_ms as f32;
    let progress = if tumbling {
        let span = (segment.lands_ms - segment.starts_ms).max(1) as f32;
        (t - segment.starts_ms as f32) / span
    } else {
        1.0
    };
    // After its first segment, a die tumbles again where it rests.
    let mut path = view.path.clone();
    if index > 0 {
        path.entry = path.rest;
    }
    let look = &view.looks[index];
    let colour = mesh::base_colour(view.dimmed);
    let mut position = path.rest;
    for (k, sub) in view.subs.iter().enumerate() {
        let Some((solid, face, landed)) = look.solids.get(k) else {
            show(parts, sub.mesh_entity, false);
            continue;
        };
        show(parts, sub.mesh_entity, true);
        let (at, rotation) = tumble::pose(&path, *landed, progress);
        if k == 0 {
            position = at;
        }
        if let Some(mut mesh) = meshes.get_mut(&sub.mesh) {
            mesh::write(&mut mesh, solid, rotation, colour, a);
        }
        let label = look.labels.get(k).cloned().unwrap_or_default();
        show(parts, sub.number, !tumbling && !label.is_empty());
        if !tumbling {
            let centre = mesh::face_centre(solid, *face, rotation);
            if let Ok(mut transform) = parts.transforms.get_mut(sub.number) {
                transform.translation = centre.extend(0.5);
            }
            set_text(parts, sub.number, &label);
            set_alpha(parts, sub.number, a * if view.dimmed { 0.6 } else { 1.0 });
        }
    }
    if let Ok(mut transform) = parts.transforms.get_mut(view.entity) {
        transform.translation.x = position.x;
        transform.translation.y = position.y;
    }
    // A value is struck through once the segment after it starts.
    let struck: Vec<i64> = die.segments[..index]
        .iter()
        .filter(|s| s.struck)
        .map(|s| s.value)
        .collect();
    let text = readout::struck_text(&struck);
    let shown = !struck.is_empty();
    show(parts, view.struck.text, shown);
    show(parts, view.struck.bar, shown);
    if shown {
        set_text(parts, view.struck.text, &text);
        set_alpha(parts, view.struck.text, a);
        if let Ok(mut sprite) = parts.sprites.get_mut(view.struck.bar) {
            sprite.custom_size = Some(Vec2::new(readout::struck_width(&text), 2.0));
            sprite.color = sprite.color.with_alpha(a);
        }
    }
}

fn show(parts: &mut ThrowParts, entity: Entity, visible: bool) {
    if let Ok(mut visibility) = parts.visibility.get_mut(entity) {
        let wanted = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
    }
}

fn set_text(parts: &mut ThrowParts, entity: Entity, text: &str) {
    if let Ok((mut current, _)) = parts.texts.get_mut(entity)
        && current.0 != text
    {
        current.0 = text.to_string();
    }
}

fn set_alpha(parts: &mut ThrowParts, entity: Entity, a: f32) {
    if let Ok((_, mut colour)) = parts.texts.get_mut(entity)
        && (colour.0.alpha() - a).abs() > 1e-3
    {
        colour.0.set_alpha(a);
    }
    if let Ok(mut sprite) = parts.sprites.get_mut(entity) {
        // Only the readout's backing is a sprite here; it is translucent.
        let base = a * 0.78;
        if (sprite.color.alpha() - base).abs() > 1e-3 {
            sprite.color.set_alpha(base);
        }
    }
}
