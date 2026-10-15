//! Spec 096: a token that is down.
//!
//! The host says a token is down (`defeated: true` on `upsert_token`); the
//! engine greys its art and lays a skull and crossed bones over it. What
//! "down" means — dead, dying, out of the fight — is the game system's word,
//! and the engine knows no game system (ADR-062): it never decides this on
//! its own, it only draws it.
//!
//! Built the way condition badges are (`condition_markers.rs`): rectangles
//! as child sprites, sized from the token's footprint, rebuilt only when the
//! state or the size changes, and none for a token culled out of view.

use bevy::prelude::*;

use crate::payloads::TokenIdentity;
use crate::plugins::nameplate::token_side;
use crate::plugins::token_culling::{TokenCullSet, ViewportCull};
use crate::resources::{SceneGrid, TokenGridBehaviour};

/// Above the art and the condition badges (5.5), below the nameplate.
const MARK_Z: f32 = 5.6;

/// Whether the host has said this token is down.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TokenDefeated(pub bool);

/// What this plugin changed on a down token, so getting up puts it back.
#[derive(Component, Debug, PartialEq)]
struct DrawnDefeated {
    /// The art's colour before it was dimmed.
    art: Color,
    /// The side the mark was built for, or `None` while it has none.
    mark: Option<f32>,
}

/// A piece of the skull and bones, drawn for the token it is a child of.
#[derive(Component)]
pub(crate) struct DefeatMark;

pub struct TokenDefeatedPlugin;

impl Plugin for TokenDefeatedPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_defeated, mirror_defeated).chain().after(TokenCullSet),
        );
    }
}

/// The colour a down token's art is drawn in: greyed toward its own
/// lightness and darkened. On art, whose sprite colour is a white tint, that
/// darkens it; on a swatch it drains the colour as well.
pub(crate) fn dimmed(color: Color) -> Color {
    let c = color.to_srgba();
    let grey = 0.3 * c.red + 0.59 * c.green + 0.11 * c.blue;
    let mix = |v: f32| (v + (grey - v) * 0.7) * 0.5;
    Color::srgba(mix(c.red), mix(c.green), mix(c.blue) + 0.03, c.alpha)
}

/// The mark's side: most of the token, so it reads zoomed out.
fn mark_side(token_side: f32) -> f32 {
    token_side * 0.6
}

fn bone() -> Color {
    Color::srgb(0.93, 0.9, 0.82)
}

fn ink() -> Color {
    Color::srgba(0.06, 0.05, 0.07, 0.9)
}

/// One rectangle of the mark, in units of its side.
struct Piece {
    at: Vec2,
    size: Vec2,
    turn: f32,
    color: Color,
    /// Drawn on top of the bone: eyes, nose, teeth.
    hole: bool,
}

/// The skull and crossed bones as rectangles, the way a badge's glyph is.
fn pieces() -> Vec<Piece> {
    use std::f32::consts::FRAC_PI_4;
    let solid = |x: f32, y: f32, w: f32, h: f32, turn: f32| Piece {
        at: Vec2::new(x, y),
        size: Vec2::new(w, h),
        turn,
        color: bone(),
        hole: false,
    };
    let hole = |x: f32, y: f32, w: f32, h: f32| Piece {
        at: Vec2::new(x, y),
        size: Vec2::new(w, h),
        turn: 0.0,
        color: ink(),
        hole: true,
    };
    let mut list = Vec::new();
    // The bones behind, each with a knob at both ends.
    for turn in [FRAC_PI_4, -FRAC_PI_4] {
        list.push(solid(0.0, -0.05, 1.0, 0.13, turn));
        let along = Vec2::from_angle(turn) * 0.46;
        for end in [along, -along] {
            list.push(solid(end.x, end.y - 0.05, 0.2, 0.2, turn));
        }
    }
    // The skull: cranium, cheeks and jaw.
    list.push(solid(0.0, 0.12, 0.58, 0.44, 0.0));
    list.push(solid(0.0, -0.12, 0.42, 0.14, 0.0));
    list.push(solid(0.0, -0.2, 0.3, 0.12, 0.0));
    // Eyes, nose and the gaps between teeth.
    list.push(hole(-0.13, 0.08, 0.15, 0.15));
    list.push(hole(0.13, 0.08, 0.15, 0.15));
    list.push(hole(0.0, -0.06, 0.06, 0.08));
    list.push(hole(-0.06, -0.21, 0.03, 0.1));
    list.push(hole(0.06, -0.21, 0.03, 0.1));
    list
}

#[allow(clippy::type_complexity)]
fn draw_defeated(
    mut commands: Commands,
    grid: Option<Res<SceneGrid>>,
    mut tokens: Query<(
        Entity,
        &TokenDefeated,
        Option<&DrawnDefeated>,
        Option<&mut Sprite>,
        Option<&Children>,
        Option<&TokenGridBehaviour>,
        Option<&ViewportCull>,
    )>,
    marks: Query<(), With<DefeatMark>>,
) {
    for (token, defeated, drawn, sprite, children, behaviour, cull) in &mut tokens {
        let clear_marks = |commands: &mut Commands| {
            if let Some(children) = children {
                for child in children.iter().filter(|c| marks.contains(*c)) {
                    commands.entity(child).despawn();
                }
            }
        };

        if !defeated.0 {
            // Up, or never down — nearly every token, nearly every frame.
            let Some(drawn) = drawn else { continue };
            if let Some(mut sprite) = sprite
                && sprite.color == dimmed(drawn.art)
            {
                sprite.color = drawn.art;
            }
            clear_marks(&mut commands);
            commands.entity(token).remove::<DrawnDefeated>();
            continue;
        }

        // The art: dimmed, and dimmed again if new art replaced it.
        let mut art = drawn.as_ref().map(|d| d.art);
        if let Some(mut sprite) = sprite {
            let known = art.is_some_and(|a| sprite.color == dimmed(a));
            if !known {
                art = Some(sprite.color);
                sprite.color = dimmed(sprite.color);
            }
        }
        let art = art.unwrap_or(Color::WHITE);

        // The mark: none while culled, rebuilt when the token's size changes.
        let culled = cull.is_some_and(|c| c.culled);
        let side = token_side(grid.as_deref(), behaviour);
        let wanted = (!culled).then_some(side);
        let had = drawn.as_ref().and_then(|d| d.mark);
        let next = DrawnDefeated { art, mark: wanted };
        if drawn != Some(&next) {
            commands.entity(token).insert(next);
        }
        if drawn.is_some() && had == wanted {
            continue;
        }
        clear_marks(&mut commands);
        let Some(side) = wanted else { continue };
        let unit = mark_side(side);
        commands.entity(token).with_children(|parent| {
            for piece in pieces() {
                let at = piece.at * unit;
                let size = piece.size * unit;
                let rotation = Quat::from_rotation_z(piece.turn);
                if !piece.hole {
                    // A dark rim under every bone piece, so the mark reads
                    // on pale art as well as dark.
                    parent.spawn((
                        Sprite {
                            color: ink(),
                            custom_size: Some(size + Vec2::splat(unit * 0.07)),
                            ..default()
                        },
                        Transform::from_xyz(at.x, at.y, MARK_Z - 0.01).with_rotation(rotation),
                        DefeatMark,
                    ));
                }
                let z = if piece.hole { MARK_Z + 0.01 } else { MARK_Z };
                parent.spawn((
                    Sprite {
                        color: piece.color,
                        custom_size: Some(size),
                        ..default()
                    },
                    Transform::from_xyz(at.x, at.y, z).with_rotation(rotation),
                    DefeatMark,
                ));
            }
        });
    }
}

static DOWN: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();

fn mirror_defeated(tokens: Query<(&TokenIdentity, &TokenDefeated), Changed<TokenDefeated>>) {
    if tokens.is_empty() {
        return;
    }
    let slot = DOWN.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut down) = slot.lock() {
        for (id, defeated) in &tokens {
            down.retain(|d| d != &id.0);
            if defeated.0 {
                down.push(id.0.clone());
            }
        }
        down.sort();
    }
}

/// The tokens the engine draws as down, as a JSON array of ids. Read-only,
/// for tests (spec 096 FR-006).
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn token_defeated() -> String {
    let list = DOWN
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    report_of(list)
}

fn report_of(list: Vec<String>) -> String {
    serde_json::Value::from(list).to_string()
}

#[cfg(test)]
#[path = "token_defeated_tests.rs"]
mod tests;
