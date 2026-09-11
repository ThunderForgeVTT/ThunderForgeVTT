//! Names above tokens — playtest 2026-09-10 P7.
//!
//! A token's name, drawn above it and above its bars, following it as it
//! moves. Children of the token, like the bars (`status_display`), so moving
//! the token carries its name for free — but kept upright and one size, so a
//! rotated or enlarged token does not turn its name on its side or blow it up.
//!
//! # What this plugin does not decide
//!
//! Whether a viewer may read a name. The server withholds a name hidden from
//! players from every client but a Game Master's (`graphql::token_art`), so a
//! player's engine never has the text to draw. A Game Master's does, and draws
//! it dimmed: they see every name, and can tell which ones the table cannot.

use bevy::prelude::*;

use crate::TOKEN_SIZE;
use crate::TokenIdentity;
use crate::plugins::status_display::{Appearance, TokenStatus};
use crate::resources::{SceneGrid, TokenGridBehaviour};
use thunderforge_canvas_core::grid::Footprint;

/// Above the bars (`status_display` draws them at 5), below selection
/// furniture.
const NAME_Z: f32 = 6.0;

/// Space between the top bar — or the token, with no bars — and the name.
const NAME_GAP: f32 = 4.0;

/// How far the shadow copy sits from the name, down and to the right, so the
/// name reads over light and dark map art alike.
const SHADOW_OFFSET: Vec2 = Vec2::new(1.5, -1.5);

/// A token's name, as this viewer is entitled to see it.
#[derive(Component, Debug, Clone, PartialEq, Default)]
pub struct TokenName {
    /// `None` or blank draws nothing.
    pub text: Option<String>,
    /// Hidden from players. Only a Game Master's client is ever sent such a
    /// name, and it draws it dimmed.
    pub hidden_from_players: bool,
}

impl TokenName {
    fn shown(&self) -> Option<&str> {
        self.text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
    }
}

/// What a token's nameplate was last drawn from, so an update that repeats
/// the same name — every token move sends one — redraws nothing.
#[derive(Component, PartialEq)]
struct DrawnName(TokenName);

/// A name, or its shadow, drawn for the token it is a child of.
#[derive(Component)]
struct Nameplate {
    shadow: bool,
    font_px: f32,
}

pub struct NameplatePlugin;

impl Plugin for NameplatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (draw_changed_names, place_names, mirror_nameplates).chain(),
        );
    }
}

/// A token's side in world units — the calculation `status_display` makes, so
/// the name and the bars agree about where the token ends.
fn token_side(grid: Option<&SceneGrid>, behaviour: Option<&TokenGridBehaviour>) -> f32 {
    let footprint = behaviour.map_or_else(Footprint::default, |b| b.footprint);
    grid.map_or(TOKEN_SIZE.x, |grid| footprint.world_size(grid.size))
}

/// Readable at any zoom and any cell size, the way the movement label is.
fn font_px(side: f32) -> f32 {
    (side * 0.22).clamp(11.0, 32.0)
}

/// Where the middle of the name sits above the token's centre, in the token's
/// own units: clear of the bars when it has any.
fn name_height(side: f32, bar_rows: usize, appearance: &Appearance, font: f32) -> f32 {
    let top = if bar_rows == 0 {
        side / 2.0
    } else {
        let last = (bar_rows - 1) as f32;
        side / 2.0
            + appearance.first_bar_offset
            + last * (appearance.bar_height + appearance.bar_gap)
            + appearance.bar_height / 2.0
    };
    top + NAME_GAP + font / 2.0
}

fn name_color(hidden_from_players: bool, shadow: bool) -> Color {
    match (shadow, hidden_from_players) {
        (false, false) => Color::srgb(0.97, 0.95, 0.90),
        (false, true) => Color::srgba(0.80, 0.80, 0.86, 0.55),
        (true, false) => Color::srgba(0.0, 0.0, 0.0, 0.8),
        (true, true) => Color::srgba(0.0, 0.0, 0.0, 0.4),
    }
}

fn draw_changed_names(
    mut commands: Commands,
    grid: Option<Res<SceneGrid>>,
    changed: Query<
        (
            Entity,
            &TokenName,
            Option<&DrawnName>,
            Option<&Children>,
            Option<&TokenGridBehaviour>,
        ),
        Changed<TokenName>,
    >,
    plates: Query<(), With<Nameplate>>,
) {
    for (token, name, drawn, children, behaviour) in &changed {
        if drawn.is_some_and(|d| d.0 == *name) {
            continue;
        }
        if let Some(children) = children {
            for child in children.iter().filter(|c| plates.contains(*c)) {
                commands.entity(child).despawn();
            }
        }
        commands.entity(token).insert(DrawnName(name.clone()));

        let Some(text) = name.shown() else {
            continue;
        };
        let font = font_px(token_side(grid.as_deref(), behaviour));
        let text = text.to_string();
        let hidden = name.hidden_from_players;
        commands.entity(token).with_children(|parent| {
            for shadow in [true, false] {
                parent.spawn((
                    Text2d::new(text.clone()),
                    TextFont {
                        font_size: FontSize::Px(font),
                        ..default()
                    },
                    TextColor(name_color(hidden, shadow)),
                    // Placed by `place_names`, which runs straight after.
                    Transform::from_xyz(0.0, 0.0, NAME_Z),
                    Nameplate {
                        shadow,
                        font_px: font,
                    },
                ));
            }
        });
    }
}

/// Keeps every name above its token's bars, upright, and one size.
///
/// Each frame, but writing only what moved: the bars above a token change as
/// its status does, and its rotation and scale as someone turns or resizes
/// it, and none of those reach this plugin as an event.
fn place_names(
    grid: Option<Res<SceneGrid>>,
    appearance: Option<Res<Appearance>>,
    tokens: Query<
        (
            &Transform,
            &Children,
            Option<&TokenStatus>,
            Option<&TokenGridBehaviour>,
        ),
        (With<TokenName>, Without<Nameplate>),
    >,
    mut plates: Query<(&mut Transform, &mut Nameplate, &mut TextFont), Without<TokenName>>,
) {
    let fallback = Appearance::default();
    let appearance = appearance.as_deref().unwrap_or(&fallback);
    for (token, children, status, behaviour) in &tokens {
        let side = token_side(grid.as_deref(), behaviour);
        let font = font_px(side);
        let rows = status.map_or(0, |s| s.resources.len());
        let height = name_height(side, rows, appearance, font);
        let scale = token.scale.x.max(f32::EPSILON);

        for child in children.iter() {
            let Ok((mut transform, mut plate, mut text_font)) = plates.get_mut(child) else {
                continue;
            };
            let offset = if plate.shadow {
                SHADOW_OFFSET
            } else {
                Vec2::ZERO
            };
            let z = if plate.shadow { NAME_Z - 0.01 } else { NAME_Z };
            // In the token's own space, so it sits above the bars wherever
            // they are; counter-rotated and counter-scaled, so it reads the
            // same on any token.
            let wanted = Transform {
                translation: Vec3::new(offset.x / scale, height + offset.y / scale, z),
                rotation: token.rotation.inverse(),
                scale: Vec3::splat(1.0 / scale),
            };
            transform.set_if_neq(wanted);
            if (plate.font_px - font).abs() > f32::EPSILON {
                plate.font_px = font;
                text_font.font_size = FontSize::Px(font);
            }
        }
    }
}

/// The names this engine draws, for [`token_nameplates`].
type NameplateList = Vec<(String, String, bool)>;

static NAMEPLATES: std::sync::OnceLock<std::sync::Mutex<NameplateList>> =
    std::sync::OnceLock::new();

fn mirror_nameplates(
    changed: Query<(), Changed<TokenName>>,
    mut removed: RemovedComponents<TokenName>,
    names: Query<(&TokenIdentity, &TokenName)>,
) {
    let any_removed = removed.read().count() > 0;
    if changed.is_empty() && !any_removed {
        return;
    }
    let list: NameplateList = names
        .iter()
        .filter_map(|(id, name)| {
            name.shown()
                .map(|text| (id.0.clone(), text.to_string(), name.hidden_from_players))
        })
        .collect();
    let slot = NAMEPLATES.get_or_init(|| std::sync::Mutex::new(Vec::new()));
    if let Ok(mut current) = slot.lock() {
        *current = list;
    }
}

/// Every name this engine draws, as
/// `[{"tokenId":…,"text":…,"dimmed":…}]`.
///
/// Read-only, and here so a test can ask what a player's canvas actually shows
/// — the claim that matters for a hidden name — rather than infer it from
/// pixels.
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn token_nameplates() -> String {
    let list = NAMEPLATES
        .get()
        .and_then(|slot| slot.lock().ok().map(|l| l.clone()))
        .unwrap_or_default();
    let entries: Vec<serde_json::Value> = list
        .into_iter()
        .map(|(token_id, text, dimmed)| {
            serde_json::json!({ "tokenId": token_id, "text": text, "dimmed": dimmed })
        })
        .collect();
    serde_json::Value::Array(entries).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(NameplatePlugin)
            .init_resource::<Appearance>();
        app
    }

    fn named(text: Option<&str>, hidden: bool) -> TokenName {
        TokenName {
            text: text.map(str::to_string),
            hidden_from_players: hidden,
        }
    }

    /// The names drawn (not their shadows), with their colours.
    fn drawn(app: &mut App) -> Vec<(String, Color)> {
        let mut query = app.world_mut().query::<(&Text2d, &TextColor, &Nameplate)>();
        query
            .iter(app.world())
            .filter(|(_, _, plate)| !plate.shadow)
            .map(|(text, color, _)| (text.0.clone(), color.0))
            .collect()
    }

    fn spawn_token(app: &mut App, name: TokenName, transform: Transform) -> Entity {
        app.world_mut()
            .spawn((transform, TokenIdentity("t1".to_string()), name))
            .id()
    }

    #[test]
    fn a_token_shows_its_name_and_follows_every_change_to_it() {
        let mut app = app();
        let token = spawn_token(&mut app, named(Some("Grom"), false), Transform::default());
        app.update();
        let names: Vec<String> = drawn(&mut app).into_iter().map(|(t, _)| t).collect();
        assert_eq!(names, ["Grom"]);

        // Renamed: the old name goes, rather than the new one drawing over it.
        app.world_mut()
            .entity_mut(token)
            .insert(named(Some("Grom the Bold"), false));
        app.update();
        let names: Vec<String> = drawn(&mut app).into_iter().map(|(t, _)| t).collect();
        assert_eq!(names, ["Grom the Bold"]);

        // No name, or a blank one, draws nothing — not an empty plate.
        app.world_mut()
            .entity_mut(token)
            .insert(named(Some("   "), false));
        app.update();
        assert!(drawn(&mut app).is_empty());
    }

    #[test]
    fn a_name_hidden_from_players_is_drawn_dimmed_for_the_game_master() {
        let mut app = app();
        spawn_token(
            &mut app,
            named(Some("The Lich"), true),
            Transform::default(),
        );
        app.update();
        let [(_, color)] = drawn(&mut app)[..] else {
            panic!("one name drawn");
        };
        assert!(color.alpha() < 1.0, "dimmed, got {color:?}");
    }

    #[test]
    fn the_name_stays_upright_and_one_size_on_a_turned_enlarged_token() {
        let mut app = app();
        let turned =
            Transform::from_rotation(Quat::from_rotation_z(1.2)).with_scale(Vec3::splat(2.0));
        spawn_token(&mut app, named(Some("Kai"), false), turned);
        app.update();
        app.update();
        let mut query = app.world_mut().query::<(&Transform, &Nameplate)>();
        for (transform, _) in query.iter(app.world()) {
            let world_rotation = turned.rotation * transform.rotation;
            assert!(
                world_rotation.angle_between(Quat::IDENTITY) < 1e-4,
                "upright"
            );
            assert!((transform.scale.x * 2.0 - 1.0).abs() < 1e-5, "one size");
        }
    }

    #[test]
    fn the_name_clears_the_bars_above_the_token() {
        let appearance = Appearance::default();
        let bare = name_height(96.0, 0, &appearance, 20.0);
        let with_bars = name_height(96.0, 2, &appearance, 20.0);
        assert!(bare > 48.0, "above the token's top edge");
        assert!(with_bars > bare, "and higher still over a stack of bars");
    }
}
