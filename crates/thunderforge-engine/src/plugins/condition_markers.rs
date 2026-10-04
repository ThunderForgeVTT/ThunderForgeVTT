//! Condition markers on tokens — spec 067 Story 4.
//!
//! A row of small badges along a token's bottom edge, one for each condition
//! its character is under. Children of the token, like its bars and its name,
//! so they travel with it.
//!
//! # What this plugin does not know
//!
//! What any condition is. It is handed an identifier, a glyph from the host's
//! short list and a colour token, and draws that (ADR-062): no ruleset's
//! vocabulary appears here, and a system that declares a condition gets its
//! marker without this file changing. What the marker *means* is the
//! system's to say, and chrome's to show.
//!
//! Nor who may see one. Conditions arrive on the token a viewer is sent
//! (`graphql::token_art`), so an engine that has the token has its markers
//! and one that does not has neither.
//!
//! # Only for tokens someone could see
//!
//! As for names (`nameplate`): a token outside the padded view has no badges
//! at all, and gets them back before it can be seen.

use bevy::prelude::*;

use crate::payloads::{ConditionPayload, TokenIdentity};
use crate::plugins::nameplate::token_side;
use crate::plugins::token_culling::{TokenCullSet, ViewportCull};
use crate::resources::{SceneGrid, TokenGridBehaviour};

/// Above the bars (`status_display` draws them at 5), below the name.
const MARKER_Z: f32 = 5.5;

/// The shape of a marker. The host's list; a system picks from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    Dot,
    Ring,
    Bar,
    Cross,
    Split,
    Corner,
}

impl Glyph {
    /// A glyph this build does not know — a newer server — draws as a dot
    /// rather than as nothing: the table still sees that something is there.
    fn parse(name: &str) -> Self {
        match name {
            "ring" => Self::Ring,
            "bar" => Self::Bar,
            "cross" => Self::Cross,
            "split" => Self::Split,
            "corner" => Self::Corner,
            _ => Self::Dot,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Dot => "dot",
            Self::Ring => "ring",
            Self::Bar => "bar",
            Self::Cross => "cross",
            Self::Split => "split",
            Self::Corner => "corner",
        }
    }
}

/// The colour of a marker, as a token. The host's list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Danger,
    Warning,
    Positive,
    Info,
    Arcane,
    Neutral,
}

impl Tone {
    fn parse(name: &str) -> Self {
        match name {
            "danger" => Self::Danger,
            "warning" => Self::Warning,
            "positive" => Self::Positive,
            "info" => Self::Info,
            "arcane" => Self::Arcane,
            _ => Self::Neutral,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Danger => "danger",
            Self::Warning => "warning",
            Self::Positive => "positive",
            Self::Info => "info",
            Self::Arcane => "arcane",
            Self::Neutral => "neutral",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Danger => Color::srgb(0.90, 0.26, 0.24),
            Self::Warning => Color::srgb(0.96, 0.70, 0.18),
            Self::Positive => Color::srgb(0.35, 0.78, 0.40),
            Self::Info => Color::srgb(0.30, 0.62, 0.95),
            Self::Arcane => Color::srgb(0.68, 0.45, 0.92),
            Self::Neutral => Color::srgb(0.80, 0.80, 0.84),
        }
    }
}

/// One condition, as much of it as the board draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub id: String,
    pub glyph: Glyph,
    pub tone: Tone,
}

/// The conditions a token's character is under, in the order to draw them.
#[derive(Component, Debug, Clone, PartialEq, Eq, Default)]
pub struct TokenConditions(pub Vec<Marker>);

impl TokenConditions {
    pub(crate) fn from_payload(conditions: &[ConditionPayload]) -> Self {
        Self(
            conditions
                .iter()
                .map(|condition| Marker {
                    id: condition.id.clone(),
                    glyph: Glyph::parse(&condition.glyph),
                    tone: Tone::parse(&condition.color),
                })
                .collect(),
        )
    }
}

/// What a token's badges were last drawn from and at what size, so an update
/// that repeats them — every token move sends one — redraws nothing.
#[derive(Component, PartialEq)]
struct DrawnConditions {
    markers: Vec<Marker>,
    side: f32,
}

/// A piece of a badge, drawn for the token it is a child of.
#[derive(Component)]
pub(crate) struct ConditionBadge;

pub struct ConditionMarkersPlugin;

impl Plugin for ConditionMarkersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_changed_conditions, mirror_conditions)
                .chain()
                // So a token that arrives off-screen is culled before its
                // badges are built, not built and torn down a frame later.
                .after(TokenCullSet),
        );
    }
}

/// A badge's side: a fifth of the token, within what stays legible.
fn badge_side(token_side: f32) -> f32 {
    (token_side * 0.2).clamp(6.0, 20.0)
}

/// Where badge `index` sits, from the token's centre: along the bottom edge
/// from the left, wrapping upward when the edge is full.
fn badge_offset(index: usize, token_side: f32) -> Vec2 {
    let badge = badge_side(token_side);
    let step = badge * 1.2;
    let per_row = (((token_side - badge) / step).floor() as usize + 1).max(1);
    let (column, row) = (index % per_row, index / per_row);
    let first = -token_side / 2.0 + badge / 2.0;
    Vec2::new(first + column as f32 * step, first + row as f32 * step)
}

/// One rectangle of a glyph, in badge units: centre, size, and a turn.
struct Part {
    offset: Vec2,
    size: Vec2,
    turn: f32,
}

const fn part(x: f32, y: f32, width: f32, height: f32) -> Part {
    Part {
        offset: Vec2::new(x, y),
        size: Vec2::new(width, height),
        turn: 0.0,
    }
}

/// A glyph as rectangles, in units of the badge's side. Rectangles because
/// that is what a sprite is, the way `interaction_marker` builds its own.
fn glyph_parts(glyph: Glyph) -> Vec<Part> {
    use std::f32::consts::FRAC_PI_4;
    match glyph {
        Glyph::Dot => vec![part(0.0, 0.0, 0.5, 0.5)],
        Glyph::Ring => vec![
            part(0.0, 0.31, 0.8, 0.18),
            part(0.0, -0.31, 0.8, 0.18),
            part(-0.31, 0.0, 0.18, 0.8),
            part(0.31, 0.0, 0.18, 0.8),
        ],
        Glyph::Bar => vec![part(0.0, 0.0, 0.8, 0.3)],
        Glyph::Cross => vec![
            Part {
                turn: FRAC_PI_4,
                ..part(0.0, 0.0, 0.9, 0.2)
            },
            Part {
                turn: -FRAC_PI_4,
                ..part(0.0, 0.0, 0.9, 0.2)
            },
        ],
        Glyph::Split => vec![part(-0.2, 0.0, 0.4, 0.8)],
        Glyph::Corner => vec![part(0.0, 0.275, 0.8, 0.25), part(-0.275, 0.0, 0.25, 0.8)],
    }
}

fn backing_color() -> Color {
    Color::srgba(0.05, 0.05, 0.08, 0.82)
}

#[allow(clippy::type_complexity)]
fn draw_changed_conditions(
    mut commands: Commands,
    grid: Option<Res<SceneGrid>>,
    tokens: Query<(
        Entity,
        &TokenConditions,
        Option<&DrawnConditions>,
        Option<&Children>,
        Option<&TokenGridBehaviour>,
        Option<&ViewportCull>,
    )>,
    badges: Query<(), With<ConditionBadge>>,
) {
    for (token, conditions, drawn, children, behaviour, cull) in &tokens {
        let culled = cull.is_some_and(|c| c.culled);
        let wanted: &[Marker] = if culled { &[] } else { &conditions.0 };
        // Nothing drawn and nothing to draw — nearly every token, nearly
        // every frame.
        if wanted.is_empty() && drawn.is_none() {
            continue;
        }
        let side = token_side(grid.as_deref(), behaviour);
        if drawn.is_some_and(|d| d.markers == wanted && d.side == side) {
            continue;
        }

        if let Some(children) = children {
            for child in children.iter().filter(|c| badges.contains(*c)) {
                commands.entity(child).despawn();
            }
        }
        if wanted.is_empty() {
            // Forgotten as well as removed, so coming back into view finds
            // "never drawn" and builds whatever the conditions are by then.
            commands.entity(token).remove::<DrawnConditions>();
            continue;
        }
        commands.entity(token).insert(DrawnConditions {
            markers: wanted.to_vec(),
            side,
        });

        let badge = badge_side(side);
        commands.entity(token).with_children(|parent| {
            for (index, marker) in wanted.iter().enumerate() {
                let at = badge_offset(index, side);
                parent.spawn((
                    Sprite {
                        color: backing_color(),
                        custom_size: Some(Vec2::splat(badge)),
                        ..default()
                    },
                    Transform::from_xyz(at.x, at.y, MARKER_Z),
                    ConditionBadge,
                ));
                for piece in glyph_parts(marker.glyph) {
                    let centre = at + piece.offset * badge;
                    parent.spawn((
                        Sprite {
                            color: marker.tone.color(),
                            custom_size: Some(piece.size * badge),
                            ..default()
                        },
                        Transform::from_xyz(centre.x, centre.y, MARKER_Z + 0.01)
                            .with_rotation(Quat::from_rotation_z(piece.turn)),
                        ConditionBadge,
                    ));
                }
            }
        });
    }
}

/// The markers this engine has drawn, for [`token_conditions`].
type DrawnList = Vec<(String, Vec<Marker>)>;

static DRAWN: std::sync::OnceLock<std::sync::Mutex<DrawnList>> = std::sync::OnceLock::new();

fn mirror_conditions(
    changed: Query<(), Changed<DrawnConditions>>,
    mut removed: RemovedComponents<DrawnConditions>,
    drawn: Query<(&TokenIdentity, &DrawnConditions)>,
) {
    let any_removed = removed.read().count() > 0;
    if changed.is_empty() && !any_removed {
        return;
    }
    let list: DrawnList = drawn
        .iter()
        .map(|(id, drawn)| (id.0.clone(), drawn.markers.clone()))
        .collect();
    let slot = DRAWN.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock() {
        *current = list;
    }
}

/// Every condition marker this engine has drawn, as
/// `[{"tokenId":…,"conditions":[{"id":…,"glyph":…,"color":…}]}]`.
///
/// Read-only, and here so a test can ask what a viewer's canvas actually
/// shows rather than infer it from pixels. A token with no badges built —
/// under no condition, or out of view — is absent.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn token_conditions() -> String {
    let list = DRAWN
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    report_of(list)
}

fn report_of(list: DrawnList) -> String {
    let entries: Vec<serde_json::Value> = list
        .into_iter()
        .map(|(token_id, markers)| {
            let conditions: Vec<serde_json::Value> = markers
                .into_iter()
                .map(|marker| {
                    serde_json::json!({
                        "id": marker.id,
                        "glyph": marker.glyph.as_str(),
                        "color": marker.tone.as_str(),
                    })
                })
                .collect();
            serde_json::json!({ "tokenId": token_id, "conditions": conditions })
        })
        .collect();
    serde_json::Value::Array(entries).to_string()
}

#[cfg(test)]
#[path = "condition_markers_tests.rs"]
mod tests;
