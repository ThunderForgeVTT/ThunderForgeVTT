//! Tokens talk — spec 103.
//!
//! When someone speaks or rolls as a token, the line floats above that token
//! for a few seconds: a bubble with the speaker's name and what they said, or
//! the roll's type and result. Children of the token, like the nameplate
//! (`plugins::nameplate`), so a token that moves carries its bubble, and kept
//! upright and one size the same way.
//!
//! # Where a line goes
//!
//! Judged once, when the line arrives (research R2): over the token when the
//! viewer can see it — its `Visibility` is not `Hidden` — and its top edge is
//! inside the camera's view less whatever page chrome covers
//! (`set_view_insets`). Anything else is the page's to show, in its stack at
//! the right edge. Either way the engine says which with `speech_placed`, so
//! the page never guesses where a token is.
//!
//! # What this plugin does not decide
//!
//! Who may hear a line. The server withholds a Game-Master-only message and a
//! hidden token's name from players, and the page sends only what its viewer
//! may read. The one thing decided here is the one only the engine knows: a
//! token hidden from this viewer by fog or vision never wears a bubble, so a
//! line never gives away where it stands (FR-015).

use std::time::Duration;

use bevy::prelude::*;

use crate::TokenIdentity;
use crate::plugins::nameplate::token_side;
use crate::resources::{SceneGrid, TokenGridBehaviour};
use crate::systems::token_glide::ReducedMotion;

/// Above the nameplate (6), below selection furniture.
const SPEECH_Z: f32 = 7.0;

/// Space between the token's top edge and the bubble's tail, in screen px.
/// Clears the nameplate and a couple of bars without measuring them.
const SPEECH_LIFT: f32 = 34.0;

/// The bubble's text size and padding, in screen px.
const SPEECH_FONT: f32 = 15.0;
const SPEECH_PAD: Vec2 = Vec2::new(10.0, 7.0);

/// A line longer than this ends in "…": about three lines of a bubble a third
/// of a phone's width.
pub(crate) const SPEECH_MAX_CHARS: usize = 120;

/// How far a bubble rises over its life, and how long it takes to fade, in
/// screen px and seconds. Neither, under reduced motion.
const RISE_PX: f32 = 14.0;
const FADE_SECS: f32 = 0.6;

/// A spoken line or a roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SpeechKind {
    #[default]
    Line,
    Roll,
}

/// One `say` command.
#[derive(Debug, Clone, PartialEq)]
pub struct Say {
    pub id: String,
    pub token_id: String,
    pub name: String,
    pub text: String,
    pub kind: SpeechKind,
    pub ms: u32,
}

/// The CSS px of the canvas that page chrome covers, edge by edge.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViewInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Where a line went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    Over,
    Edge,
}

impl Placement {
    fn as_str(self) -> &'static str {
        match self {
            Placement::Over => "over",
            Placement::Edge => "edge",
        }
    }
}

/// What the command loop hands this plugin, and what it last placed.
#[derive(Resource, Default)]
pub struct Speech {
    pub says: Vec<Say>,
    pub clear: bool,
    pub insets: ViewInsets,
    /// The lines placed on the last frame that placed any, for tests; the
    /// page hears the same through `speech_placed`.
    pub placed: Vec<(String, Placement)>,
}

/// A bubble, drawn for the token it is a child of.
#[derive(Component, Debug, Clone)]
pub struct SpeechBubble {
    pub id: String,
    pub token_id: String,
    pub name: String,
    pub text: String,
    pub kind: SpeechKind,
    life: Timer,
}

/// The bubble's backing, its child.
#[derive(Component)]
struct SpeechBacking;

/// The bubble's words, its child.
#[derive(Component)]
struct SpeechText;

pub struct SpeechPlugin;

impl Plugin for SpeechPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Speech>().add_systems(
            Update,
            (
                take_says,
                hush_hidden_tokens,
                age_bubbles,
                fit_backings,
                mirror_bubbles,
            )
                .chain(),
        );
    }
}

/// `text` cut to [`SPEECH_MAX_CHARS`], ending in "…" when it was longer.
pub(crate) fn clip(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= SPEECH_MAX_CHARS {
        return text.to_string();
    }
    let cut: String = text.chars().take(SPEECH_MAX_CHARS - 1).collect();
    format!("{}…", cut.trim_end())
}

/// The world units one CSS px covers, and the view less the insets.
fn usable_view(
    cameras: &Query<(&Transform, &Projection), With<Camera2d>>,
    insets: ViewInsets,
) -> Option<(f32, Rect)> {
    let (transform, projection) = cameras.iter().next()?;
    let Projection::Orthographic(ortho) = projection else {
        return None;
    };
    let size = ortho.area.size();
    if !(size.x > 0.0 && size.y > 0.0 && size.is_finite()) {
        return None;
    }
    let per_px = ortho.scale.max(f32::EPSILON);
    let centre = transform.translation.truncate();
    // Screen y runs down and world y up: the top inset trims `max.y`.
    let min = centre + ortho.area.min + Vec2::new(insets.left, insets.bottom) * per_px;
    let max = centre + ortho.area.max - Vec2::new(insets.right, insets.top) * per_px;
    Some((per_px, Rect::from_corners(min, max)))
}

#[allow(clippy::too_many_arguments)]
fn take_says(
    mut commands: Commands,
    mut speech: ResMut<Speech>,
    grid: Option<Res<SceneGrid>>,
    cameras: Query<(&Transform, &Projection), With<Camera2d>>,
    tokens: Query<(
        Entity,
        &TokenIdentity,
        &GlobalTransform,
        &Transform,
        Option<&Visibility>,
        Option<&TokenGridBehaviour>,
        Option<&Children>,
    )>,
    bubbles: Query<Entity, With<SpeechBubble>>,
) {
    if speech.clear {
        speech.clear = false;
        for bubble in &bubbles {
            commands.entity(bubble).despawn();
        }
    }
    if speech.says.is_empty() {
        return;
    }
    let says = std::mem::take(&mut speech.says);
    let view = usable_view(&cameras, speech.insets);
    let per_px = view.map_or(1.0, |(p, _)| p);
    let mut placed = Vec::with_capacity(says.len());

    for say in says {
        let token = tokens.iter().find(|(_, id, ..)| id.0 == say.token_id);
        let over = token.and_then(
            |(entity, _, global, transform, visibility, behaviour, children)| {
                if visibility == Some(&Visibility::Hidden) {
                    return None;
                }
                let side = token_side(grid.as_deref(), behaviour) * transform.scale.y;
                let anchor = global.translation().truncate() + Vec2::new(0.0, side / 2.0);
                // No camera to ask is "everything is in view", as culling has it:
                // a missing camera must not send every line to the edge.
                if view.is_some_and(|(_, rect)| !rect.contains(anchor)) {
                    return None;
                }
                Some((entity, transform, side, children))
            },
        );
        let placement = if over.is_some() {
            Placement::Over
        } else {
            Placement::Edge
        };
        crate::emit_event(serde_json::json!({
            "type": "speech_placed",
            "id": say.id,
            "tokenId": say.token_id,
            "placement": placement.as_str(),
        }));
        placed.push((say.id.clone(), placement));

        let Some((entity, transform, side, children)) = over else {
            continue;
        };
        // One bubble a token: the new line replaces the old.
        if let Some(children) = children {
            for child in children.iter().filter(|c| bubbles.contains(*c)) {
                commands.entity(child).despawn();
            }
        }
        spawn_bubble(&mut commands, entity, transform, side, per_px, say);
    }
    speech.placed = placed;
}

fn spawn_bubble(
    commands: &mut Commands,
    token: Entity,
    token_transform: &Transform,
    side: f32,
    per_px: f32,
    say: Say,
) {
    let scale = token_transform.scale.x.max(f32::EPSILON);
    let label = match say.kind {
        SpeechKind::Line => format!("{}: {}", say.name, clip(&say.text)),
        SpeechKind::Roll => format!("{} · {}", say.name, clip(&say.text)),
    };
    let height = side / 2.0 + SPEECH_LIFT * per_px;
    let bubble = SpeechBubble {
        id: say.id,
        token_id: say.token_id,
        name: say.name,
        text: say.text,
        kind: say.kind,
        life: Timer::new(Duration::from_millis(u64::from(say.ms)), TimerMode::Once),
    };
    let text_color = match bubble.kind {
        SpeechKind::Line => Color::srgb(0.97, 0.95, 0.90),
        SpeechKind::Roll => Color::srgb(1.0, 0.84, 0.45),
    };
    commands.entity(token).with_children(|parent| {
        parent
            .spawn((
                bubble,
                // In the token's own space, counter-rotated and counter-scaled
                // so it reads the same on any token; then sized in screen px.
                Transform {
                    translation: Vec3::new(0.0, height / scale, SPEECH_Z),
                    rotation: token_transform.rotation.inverse(),
                    scale: Vec3::splat(per_px / scale),
                },
                Visibility::Inherited,
            ))
            .with_children(|bubble| {
                bubble.spawn((
                    Sprite::from_color(Color::srgba(0.07, 0.07, 0.09, 0.92), Vec2::ONE),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    SpeechBacking,
                ));
                bubble.spawn((
                    Text2d::new(label),
                    TextFont {
                        font_size: FontSize::Px(SPEECH_FONT),
                        ..default()
                    },
                    TextColor(text_color),
                    TextLayout::justify(Justify::Center),
                    // A third of a phone's width; the caller keeps lines short.
                    bevy::text::TextBounds::new_horizontal(260.0),
                    Transform::from_xyz(0.0, 0.0, 0.01),
                    SpeechText,
                ));
            });
    });
}

/// A token that turns hidden — a viewer change, fog closing — loses its
/// bubble at once, so the bubble never marks where it stands.
fn hush_hidden_tokens(
    mut commands: Commands,
    bubbles: Query<(Entity, &ChildOf), With<SpeechBubble>>,
    visibility: Query<&Visibility>,
) {
    for (bubble, parent) in &bubbles {
        if visibility.get(parent.parent()) == Ok(&Visibility::Hidden) {
            commands.entity(bubble).despawn();
        }
    }
}

/// Rises, fades and goes after its time; under reduced motion it only goes.
fn age_bubbles(
    mut commands: Commands,
    time: Res<Time>,
    motion: Option<Res<ReducedMotion>>,
    mut bubbles: Query<(Entity, &mut SpeechBubble, &mut Transform, &Children)>,
    mut backings: Query<&mut Sprite, With<SpeechBacking>>,
    mut words: Query<&mut TextColor, With<SpeechText>>,
) {
    let reduced = motion.is_some_and(|m| m.0);
    for (entity, mut bubble, mut transform, children) in &mut bubbles {
        let before = bubble.life.elapsed_secs();
        bubble.life.tick(time.delta());
        if bubble.life.is_finished() {
            commands.entity(entity).despawn();
            continue;
        }
        if reduced {
            continue;
        }
        let total = bubble.life.duration().as_secs_f32().max(f32::EPSILON);
        let rise = RISE_PX * (bubble.life.elapsed_secs() - before) / total;
        // `scale` is world units per px over the token's scale, so a px of
        // rise is a px on screen however the token is sized.
        transform.translation.y += rise * transform.scale.y;
        let left = bubble.life.remaining_secs();
        if left < FADE_SECS {
            let alpha = (left / FADE_SECS).clamp(0.0, 1.0);
            for child in children.iter() {
                if let Ok(mut sprite) = backings.get_mut(child) {
                    sprite.color.set_alpha(0.92 * alpha);
                }
                if let Ok(mut color) = words.get_mut(child) {
                    color.0.set_alpha(alpha);
                }
            }
        }
    }
}

/// Sizes each backing to the words it holds, once they are laid out.
fn fit_backings(
    words: Query<
        (&ChildOf, &bevy::text::TextLayoutInfo),
        (With<SpeechText>, Changed<bevy::text::TextLayoutInfo>),
    >,
    bubbles: Query<&Children, With<SpeechBubble>>,
    mut backings: Query<&mut Sprite, With<SpeechBacking>>,
) {
    for (parent, layout) in &words {
        let Ok(children) = bubbles.get(parent.parent()) else {
            continue;
        };
        let size = layout.size + SPEECH_PAD * 2.0;
        for child in children.iter() {
            if let Ok(mut sprite) = backings.get_mut(child) {
                sprite.custom_size = Some(size);
            }
        }
    }
}

/// The bubbles alive now, for [`speech_bubbles`].
type BubbleList = Vec<SpeechBubble>;

static BUBBLES: std::sync::OnceLock<std::sync::Mutex<BubbleList>> = std::sync::OnceLock::new();

fn mirror_bubbles(
    added: Query<(), Added<SpeechBubble>>,
    mut removed: RemovedComponents<SpeechBubble>,
    bubbles: Query<&SpeechBubble>,
) {
    let any_removed = removed.read().count() > 0;
    if added.is_empty() && !any_removed {
        return;
    }
    let list: BubbleList = bubbles.iter().cloned().collect();
    let slot = BUBBLES.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock() {
        *current = list;
    }
}

/// Every bubble alive now, as
/// `[{"id":…,"tokenId":…,"name":…,"text":…,"kind":"line"|"roll"}]`.
///
/// Read-only, and here so a test can ask what a canvas shows over a token
/// rather than infer it from pixels.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn speech_bubbles() -> String {
    let list = BUBBLES
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    let entries: Vec<serde_json::Value> = list
        .into_iter()
        .map(|b| {
            serde_json::json!({
                "id": b.id,
                "tokenId": b.token_id,
                "name": b.name,
                "text": b.text,
                "kind": match b.kind { SpeechKind::Line => "line", SpeechKind::Roll => "roll" },
            })
        })
        .collect();
    serde_json::Value::Array(entries).to_string()
}

#[cfg(test)]
#[path = "speech_tests.rs"]
mod tests;
