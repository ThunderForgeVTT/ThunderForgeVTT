//! The words on a throw: each die's face number, the values a reroll or a
//! clamp struck through, the line of arithmetic above the dice, and the
//! `+N more` chip (data-model.md, "Derived: the readout").
//!
//! The text itself comes from `dice_throw::readout`; this module only places
//! it. The engine's font is ASCII only, which `readout()` already ensures.

use bevy::prelude::*;
use thunderforge_canvas_core::dice_throw::tumble::{DIE_PX, PER_ROW, SLOT_PX};

use super::DiceThrowEntity;

const READOUT_PX: f32 = 20.0;
const CHIP_PX: f32 = 16.0;
const FACE_PX: f32 = 20.0;
const STRUCK_PX: f32 = 15.0;
/// About how wide a glyph is, for the backing and the strike bar.
const GLYPH_W: f32 = 0.56;

pub fn ink() -> Color {
    Color::srgb(0.10, 0.09, 0.08)
}

pub fn paper() -> Color {
    Color::srgb(0.97, 0.96, 0.93)
}

fn backing() -> Color {
    Color::srgba(0.06, 0.06, 0.08, 0.78)
}

/// How far above the anchor the readout sits: over the top row of dice.
pub fn readout_y(drawn: usize) -> f32 {
    let rows = drawn.div_ceil(PER_ROW).max(1);
    (rows as f32 - 1.0) / 2.0 * SLOT_PX + DIE_PX * 0.5 + 34.0
}

/// The text entities of the readout line, hidden until the throw lands.
pub struct ReadoutView {
    pub entities: Vec<Entity>,
}

/// Spawns the readout and, when dice went undrawn, the chip, under `root`.
pub fn spawn_readout(
    commands: &mut Commands,
    root: Entity,
    text: &str,
    chip: Option<&str>,
    drawn: usize,
) -> ReadoutView {
    let y = readout_y(drawn);
    let width = text.len() as f32 * READOUT_PX * GLYPH_W + 28.0;
    let mut entities = vec![
        commands
            .spawn((
                DiceThrowEntity,
                Sprite::from_color(backing(), Vec2::new(width, READOUT_PX + 14.0)),
                Transform::from_xyz(0.0, y, 40.0),
                Visibility::Hidden,
                ChildOf(root),
            ))
            .id(),
        commands
            .spawn((
                DiceThrowEntity,
                Text2d::new(text),
                TextFont {
                    font_size: FontSize::Px(READOUT_PX),
                    ..default()
                },
                TextColor(paper()),
                Transform::from_xyz(0.0, y, 41.0),
                Visibility::Hidden,
                ChildOf(root),
            ))
            .id(),
    ];
    if let Some(chip) = chip {
        let chip_y = y + READOUT_PX + 14.0;
        let chip_w = chip.len() as f32 * CHIP_PX * GLYPH_W + 18.0;
        entities.push(
            commands
                .spawn((
                    DiceThrowEntity,
                    Sprite::from_color(backing(), Vec2::new(chip_w, CHIP_PX + 10.0)),
                    Transform::from_xyz(0.0, chip_y, 40.0),
                    Visibility::Hidden,
                    ChildOf(root),
                ))
                .id(),
        );
        entities.push(
            commands
                .spawn((
                    DiceThrowEntity,
                    Text2d::new(chip),
                    TextFont {
                        font_size: FontSize::Px(CHIP_PX),
                        ..default()
                    },
                    TextColor(paper()),
                    Transform::from_xyz(0.0, chip_y, 41.0),
                    Visibility::Hidden,
                    ChildOf(root),
                ))
                .id(),
        );
    }
    ReadoutView { entities }
}

/// A face number, placed on the landed face each frame.
pub fn spawn_face_number(commands: &mut Commands, parent: Entity) -> Entity {
    commands
        .spawn((
            DiceThrowEntity,
            Text2d::new(""),
            TextFont {
                font_size: FontSize::Px(FACE_PX),
                ..default()
            },
            TextColor(ink()),
            Transform::from_xyz(0.0, 0.0, 0.5),
            Visibility::Hidden,
            ChildOf(parent),
        ))
        .id()
}

/// The struck-through values over a die: a `Text2d` with a thin bar across
/// it.
pub struct StruckView {
    pub text: Entity,
    pub bar: Entity,
}

pub fn spawn_struck(commands: &mut Commands, parent: Entity) -> StruckView {
    let y = DIE_PX * 0.5 + 12.0;
    let text = commands
        .spawn((
            DiceThrowEntity,
            Text2d::new(""),
            TextFont {
                font_size: FontSize::Px(STRUCK_PX),
                ..default()
            },
            TextColor(paper()),
            Transform::from_xyz(0.0, y, 0.6),
            Visibility::Hidden,
            ChildOf(parent),
        ))
        .id();
    let bar = commands
        .spawn((
            DiceThrowEntity,
            Sprite::from_color(paper(), Vec2::new(1.0, 2.0)),
            Transform::from_xyz(0.0, y, 0.7),
            Visibility::Hidden,
            ChildOf(parent),
        ))
        .id();
    StruckView { text, bar }
}

/// The struck values as shown: `3 5`.
pub fn struck_text(values: &[i64]) -> String {
    values
        .iter()
        .map(i64::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The strike bar's width for `text`.
pub fn struck_width(text: &str) -> f32 {
    text.len() as f32 * STRUCK_PX * GLYPH_W + 4.0
}
