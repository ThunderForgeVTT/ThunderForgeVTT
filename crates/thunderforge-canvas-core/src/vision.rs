//! Illumination and vision: what a token can see, and how well.
//!
//! `wall::is_visible` answers only "is the line of sight blocked". That is one
//! of three independent questions a VTT has to answer before it can decide
//! whether a token sees something:
//!
//! 1. **Occlusion** — is there a wall in the way? (`wall::is_visible`)
//! 2. **Facing** — is the target inside the observer's vision cone?
//! 3. **Illumination** — is there enough light there for this observer's kind
//!    of sight?
//!
//! They are genuinely independent: a torch-lit target behind a wall is
//! invisible, an unlit target in front of an unaided observer is invisible,
//! and the same unlit target is plainly visible to a creature with darkvision.
//! Collapsing them into one boolean is what makes lighting systems feel wrong.
//!
//! # The illumination model
//!
//! Three levels — `Bright`, `Dim`, `Dark` — matching how tabletop rules and
//! every major VTT describe light. A light has two radii: `bright_radius`
//! (full illumination) and `dim_radius` (partial). A torch is the canonical
//! example: bright to 20ft, dim to 40ft, dark beyond.
//!
//! Levels combine by taking the **best** light reaching a point, not by
//! summing. Two dim lights overlapping do not make bright light — a point lit
//! dimly from two directions is still dimly lit. Summing is the intuitive
//! implementation and it is wrong.
//!
//! # Vision
//!
//! `VisionProfile` describes an observer's eyes. Normal sight needs `Dim` or
//! better. Darkvision converts `Dark` to `Dim` within its range, which is
//! exactly how the rules phrase it — and why a creature with darkvision still
//! cannot make out colour or fine detail in the dark, reported here as `Dim`
//! rather than `Clear`.

use glam::Vec2;

use crate::wall::{WallSet, is_visible};

/// How well-lit a point is.
///
/// Ordered: `Dark < Dim < Bright`, so combining lights is a `max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Illumination {
    #[default]
    Dark,
    Dim,
    Bright,
}

/// How well an observer perceives a particular target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Visibility {
    /// Not perceived at all — occluded, out of the vision cone, or too dark.
    Hidden,
    /// Perceived, but indistinctly: dim light, or darkvision in the dark.
    Dim,
    /// Fully perceived.
    Clear,
}

/// A linear RGB colour in 0..=1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl Rgb {
    pub const WHITE: Rgb = Rgb {
        r: 1.0,
        g: 1.0,
        b: 1.0,
    };

    /// Parses `#rrggbb` / `rrggbb` (and the 3-digit short form).
    ///
    /// Returns `None` rather than a default on malformed input so callers can
    /// distinguish "no colour specified" from "colour specified as nonsense" —
    /// `LightSource.color` is an `Option<String>` of unvalidated server text.
    pub fn parse_hex(value: &str) -> Option<Rgb> {
        let hex = value.trim().trim_start_matches('#');
        let expand = |c: u8| -> f32 { c as f32 / 255.0 };

        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Rgb {
                    r: expand(r),
                    g: expand(g),
                    b: expand(b),
                })
            }
            3 => {
                let digit = |i: usize| -> Option<u8> {
                    let d = u8::from_str_radix(&hex[i..i + 1], 16).ok()?;
                    // "f" means "ff", not "0f".
                    Some(d * 17)
                };
                Some(Rgb {
                    r: expand(digit(0)?),
                    g: expand(digit(1)?),
                    b: expand(digit(2)?),
                })
            }
            _ => None,
        }
    }

    /// Weighted blend toward `other`.
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }
}

/// A light, resolved to a world position and ready for illumination queries.
///
/// Separate from `lighting::LightSource` because that type is the wire/storage
/// shape: it carries an `attached_token_id` whose live position only the ECS
/// knows, and a `color` as unvalidated text. Resolving those is the engine's
/// job; this is what the math consumes.
#[derive(Clone, Copy, Debug)]
pub struct ResolvedLight {
    pub position: Vec2,
    /// Full illumination out to here.
    pub bright_radius: f32,
    /// Partial illumination out to here. Values below `bright_radius` are
    /// treated as equal to it — a light cannot be dim closer than it is bright.
    pub dim_radius: f32,
    pub color: Rgb,
    /// 0 turns the light off entirely without deleting it.
    pub intensity: f32,
    /// Whether walls occlude this light. A magical ambient glow might not.
    pub casts_shadows: bool,
}

impl ResolvedLight {
    /// A torch: bright to `bright`, dim to twice that, warm orange.
    ///
    /// The 1:2 bright-to-dim ratio is the standard tabletop shape (a torch is
    /// 20ft bright / 40ft total), so this is the convenient constructor for
    /// the overwhelmingly common case.
    pub fn torch(position: Vec2, bright: f32) -> Self {
        Self {
            position,
            bright_radius: bright,
            dim_radius: bright * 2.0,
            color: Rgb {
                r: 1.0,
                g: 0.78,
                b: 0.5,
            },
            intensity: 1.0,
            casts_shadows: true,
        }
    }

    fn is_on(&self) -> bool {
        self.intensity > 0.0
    }

    /// The illumination this light alone provides at `point`, ignoring walls.
    fn illumination_ignoring_walls(&self, point: Vec2) -> Illumination {
        if !self.is_on() {
            return Illumination::Dark;
        }
        let distance = self.position.distance(point);
        // A dim radius smaller than the bright radius is meaningless; clamp
        // rather than letting it carve a dark ring out of the bright zone.
        let dim = self.dim_radius.max(self.bright_radius);

        if distance <= self.bright_radius {
            Illumination::Bright
        } else if distance <= dim {
            Illumination::Dim
        } else {
            Illumination::Dark
        }
    }
}

/// An observer's eyes.
#[derive(Clone, Copy, Debug)]
pub struct VisionProfile {
    /// Range within which darkness is perceived as dim light. 0 disables it.
    pub darkvision: f32,
    /// Facing, in radians, for the vision cone. `None` sees in all directions.
    pub facing: Option<f32>,
    /// Total cone width in radians. Ignored when `facing` is `None`.
    pub fov: f32,
    /// Hard sight limit regardless of light. `None` is unlimited.
    pub max_range: Option<f32>,
}

impl Default for VisionProfile {
    /// Unaided, omnidirectional sight: the sensible default for a token
    /// nobody has configured.
    fn default() -> Self {
        Self {
            darkvision: 0.0,
            facing: None,
            fov: std::f32::consts::TAU,
            max_range: None,
        }
    }
}

impl VisionProfile {
    /// Darkvision out to `range`, otherwise unaided.
    pub fn with_darkvision(range: f32) -> Self {
        Self {
            darkvision: range,
            ..Self::default()
        }
    }

    /// A directional cone: `fov` radians wide, centred on `facing`.
    pub fn cone(facing: f32, fov: f32) -> Self {
        Self {
            facing: Some(facing),
            fov,
            ..Self::default()
        }
    }

    /// Whether `target` falls inside the cone from `observer`.
    ///
    /// A target at the observer's own position is always inside — the angle is
    /// undefined there, and "you cannot see your own square" is not a rule
    /// anyone wants.
    pub fn in_cone(&self, observer: Vec2, target: Vec2) -> bool {
        let Some(facing) = self.facing else {
            return true;
        };
        if self.fov >= std::f32::consts::TAU {
            return true;
        }

        let delta = target - observer;
        if delta.length_squared() <= f32::EPSILON {
            return true;
        }

        let angle = delta.y.atan2(delta.x);
        // Shortest signed angular difference, wrapped to (-PI, PI]. Comparing
        // raw angles fails across the ±PI seam, which is exactly where a token
        // facing left would develop a blind spot straight ahead.
        let mut diff = angle - facing;
        while diff > std::f32::consts::PI {
            diff -= std::f32::consts::TAU;
        }
        while diff < -std::f32::consts::PI {
            diff += std::f32::consts::TAU;
        }

        diff.abs() <= self.fov / 2.0
    }
}

/// Scene-wide lighting conditions.
#[derive(Clone, Copy, Debug, Default)]
pub struct AmbientLight {
    /// The illumination of a point no light source reaches. `Bright` is
    /// daylight outdoors; `Dark` is an unlit dungeon.
    pub level: Illumination,
    pub color: Option<Rgb>,
}

impl AmbientLight {
    pub fn daylight() -> Self {
        Self {
            level: Illumination::Bright,
            color: None,
        }
    }

    pub fn unlit() -> Self {
        Self {
            level: Illumination::Dark,
            color: None,
        }
    }
}

/// The illumination at `point`, and the colour of the light reaching it.
///
/// Occlusion is evaluated per light: a shadow-casting light behind a wall
/// contributes nothing, while one in the open contributes normally. Lights
/// with `casts_shadows == false` ignore walls entirely.
///
/// The returned colour is that of the **brightest contributing light**, not an
/// average. Averaging makes a bright torch beside a weak blue glow read as
/// washed-out lavender; the dominant source is what an eye actually reports.
pub fn illumination_at(
    point: Vec2,
    lights: &[ResolvedLight],
    walls: &WallSet,
    ambient: AmbientLight,
) -> (Illumination, Rgb) {
    let mut best = ambient.level;
    let mut color = ambient.color.unwrap_or(Rgb::WHITE);
    // Tracks how close the winning light is, to break ties between two lights
    // of the same level in favour of the nearer one.
    let mut best_distance = f32::INFINITY;

    for light in lights {
        let level = light.illumination_ignoring_walls(point);
        if level == Illumination::Dark {
            continue;
        }
        if light.casts_shadows && !is_visible(light.position, point, walls) {
            continue;
        }

        let distance = light.position.distance(point);
        if level > best || (level == best && distance < best_distance) {
            best = level;
            color = light.color;
            best_distance = distance;
        }
    }

    (best, color)
}

/// How well an observer at `observer` perceives something at `target`.
///
/// Combines all three questions in the order that lets each one short-circuit:
/// range, then cone, then occlusion, then illumination.
pub fn visibility_of(
    observer: Vec2,
    vision: &VisionProfile,
    target: Vec2,
    lights: &[ResolvedLight],
    walls: &WallSet,
    ambient: AmbientLight,
) -> Visibility {
    let distance = observer.distance(target);

    if let Some(max_range) = vision.max_range
        && distance > max_range
    {
        return Visibility::Hidden;
    }

    if !vision.in_cone(observer, target) {
        return Visibility::Hidden;
    }

    if !is_visible(observer, target, walls) {
        return Visibility::Hidden;
    }

    let (level, _color) = illumination_at(target, lights, walls, ambient);

    match level {
        Illumination::Bright => Visibility::Clear,
        Illumination::Dim => Visibility::Dim,
        // Darkvision turns darkness into dim sight, within its range — never
        // into clear sight, which is what the rules say and why a
        // darkvision-equipped token still cannot read a scroll in the dark.
        Illumination::Dark if distance <= vision.darkvision => Visibility::Dim,
        Illumination::Dark => Visibility::Hidden,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wall::{DoorState, Wall};

    fn wall_between(x1: f32, y1: f32, x2: f32, y2: f32) -> WallSet {
        let mut walls = WallSet::default();
        walls.upsert(Wall {
            id: "w".into(),
            x1,
            y1,
            x2,
            y2,
            blocks_vision: true,
            blocks_movement: false,
            door_state: DoorState::None,
            locked: false,
            secret: false,
        });
        walls
    }

    fn no_walls() -> WallSet {
        WallSet::default()
    }

    // --- colour ----------------------------------------------------------

    #[test]
    fn hex_colours_parse_in_both_lengths() {
        assert_eq!(Rgb::parse_hex("#ffffff"), Some(Rgb::WHITE));
        assert_eq!(
            Rgb::parse_hex("000000"),
            Some(Rgb {
                r: 0.0,
                g: 0.0,
                b: 0.0
            })
        );
        // "f" expands to "ff", not "0f" — otherwise #fff would come out grey.
        assert_eq!(Rgb::parse_hex("#fff"), Some(Rgb::WHITE));
        let red = Rgb::parse_hex("#f00").unwrap();
        assert!((red.r - 1.0).abs() < 1e-6 && red.g == 0.0 && red.b == 0.0);
    }

    #[test]
    fn malformed_colours_are_rejected_rather_than_defaulted() {
        // `LightSource.color` is unvalidated server text, so callers need to
        // tell "unset" apart from "garbage".
        assert_eq!(Rgb::parse_hex("nonsense"), None);
        assert_eq!(Rgb::parse_hex("#12345"), None);
        assert_eq!(Rgb::parse_hex(""), None);
        assert_eq!(Rgb::parse_hex("#gggggg"), None);
    }

    // --- illumination ----------------------------------------------------

    #[test]
    fn a_torch_is_bright_then_dim_then_dark() {
        let torch = ResolvedLight::torch(Vec2::ZERO, 20.0);
        let lights = [torch];
        let walls = no_walls();
        let ambient = AmbientLight::unlit();

        let at = |d: f32| illumination_at(Vec2::new(d, 0.0), &lights, &walls, ambient).0;

        assert_eq!(at(10.0), Illumination::Bright);
        assert_eq!(at(20.0), Illumination::Bright);
        assert_eq!(at(30.0), Illumination::Dim);
        assert_eq!(at(40.0), Illumination::Dim);
        assert_eq!(at(41.0), Illumination::Dark);
    }

    #[test]
    fn two_dim_lights_do_not_add_up_to_bright() {
        // The tempting implementation sums intensities. It is wrong: a point
        // lit dimly from two sides is still dimly lit.
        // Each torch is bright to 10 and dim to 20, and the origin sits 15
        // from both — inside each dim ring, inside neither bright one.
        let a = ResolvedLight::torch(Vec2::new(-15.0, 0.0), 10.0);
        let b = ResolvedLight::torch(Vec2::new(15.0, 0.0), 10.0);
        let (level, _) = illumination_at(Vec2::ZERO, &[a, b], &no_walls(), AmbientLight::unlit());
        assert_eq!(level, Illumination::Dim);
    }

    #[test]
    fn a_wall_casts_a_shadow() {
        let torch = ResolvedLight::torch(Vec2::new(-50.0, 0.0), 100.0);
        // A wall standing between the torch and the far point.
        let walls = wall_between(0.0, -50.0, 0.0, 50.0);
        let ambient = AmbientLight::unlit();

        let shadowed = illumination_at(Vec2::new(50.0, 0.0), &[torch], &walls, ambient).0;
        assert_eq!(shadowed, Illumination::Dark, "wall should block the light");

        // Past the end of the wall. The sight line from (-50,0) to (50,120)
        // crosses x=0 at y=60, clear of the wall's -50..50 span, and the point
        // is 156 away — inside the torch's 200-unit dim radius.
        let lit = illumination_at(Vec2::new(50.0, 120.0), &[torch], &walls, ambient).0;
        assert_ne!(
            lit,
            Illumination::Dark,
            "light should reach past the wall's end"
        );
    }

    #[test]
    fn an_open_door_stops_casting_a_shadow() {
        let torch = ResolvedLight::torch(Vec2::new(-50.0, 0.0), 100.0);
        let mut walls = wall_between(0.0, -50.0, 0.0, 50.0);
        let ambient = AmbientLight::unlit();
        let probe = Vec2::new(50.0, 0.0);

        assert_eq!(
            illumination_at(probe, &[torch], &walls, ambient).0,
            Illumination::Dark,
        );

        let mut door = walls.get("w").unwrap().clone();
        door.door_state = DoorState::Open;
        walls.upsert(door);

        assert_ne!(
            illumination_at(probe, &[torch], &walls, ambient).0,
            Illumination::Dark,
            "an open door must not cast a shadow",
        );
    }

    #[test]
    fn a_light_that_ignores_walls_shines_through_them() {
        let mut glow = ResolvedLight::torch(Vec2::new(-50.0, 0.0), 100.0);
        glow.casts_shadows = false;
        let walls = wall_between(0.0, -50.0, 0.0, 50.0);

        let level = illumination_at(Vec2::new(50.0, 0.0), &[glow], &walls, AmbientLight::unlit()).0;
        assert_eq!(level, Illumination::Bright);
    }

    #[test]
    fn ambient_light_is_the_floor_not_a_light_source() {
        let walls = wall_between(0.0, -50.0, 0.0, 50.0);
        // Ambient reaches everywhere, including behind walls — it is the
        // scene's baseline, not something emitted from a point.
        let level = illumination_at(Vec2::new(50.0, 0.0), &[], &walls, AmbientLight::daylight()).0;
        assert_eq!(level, Illumination::Bright);
    }

    #[test]
    fn a_light_off_contributes_nothing() {
        let mut torch = ResolvedLight::torch(Vec2::ZERO, 50.0);
        torch.intensity = 0.0;
        let level = illumination_at(
            Vec2::new(5.0, 0.0),
            &[torch],
            &no_walls(),
            AmbientLight::unlit(),
        )
        .0;
        assert_eq!(level, Illumination::Dark);
    }

    #[test]
    fn the_nearest_of_two_equal_lights_sets_the_colour() {
        let mut near = ResolvedLight::torch(Vec2::new(5.0, 0.0), 100.0);
        near.color = Rgb {
            r: 1.0,
            g: 0.0,
            b: 0.0,
        };
        let mut far = ResolvedLight::torch(Vec2::new(90.0, 0.0), 100.0);
        far.color = Rgb {
            r: 0.0,
            g: 0.0,
            b: 1.0,
        };

        let (_, color) =
            illumination_at(Vec2::ZERO, &[far, near], &no_walls(), AmbientLight::unlit());
        assert_eq!(
            color,
            Rgb {
                r: 1.0,
                g: 0.0,
                b: 0.0
            }
        );
    }

    #[test]
    fn a_dim_radius_below_the_bright_radius_does_not_carve_a_dark_ring() {
        let mut light = ResolvedLight::torch(Vec2::ZERO, 50.0);
        light.dim_radius = 10.0; // nonsense: smaller than bright
        let level = illumination_at(
            Vec2::new(40.0, 0.0),
            &[light],
            &no_walls(),
            AmbientLight::unlit(),
        )
        .0;
        assert_eq!(level, Illumination::Bright);
    }

    // --- vision cones ----------------------------------------------------

    #[test]
    fn omnidirectional_vision_sees_every_direction() {
        let vision = VisionProfile::default();
        for angle in [0.0_f32, 1.0, 3.0, -2.5] {
            let target = Vec2::new(angle.cos(), angle.sin()) * 10.0;
            assert!(vision.in_cone(Vec2::ZERO, target));
        }
    }

    #[test]
    fn a_cone_excludes_what_is_behind_it() {
        // Facing +X, 90 degrees wide.
        let vision = VisionProfile::cone(0.0, std::f32::consts::FRAC_PI_2);
        assert!(vision.in_cone(Vec2::ZERO, Vec2::new(10.0, 0.0)));
        assert!(vision.in_cone(Vec2::ZERO, Vec2::new(10.0, 9.0)));
        assert!(!vision.in_cone(Vec2::ZERO, Vec2::new(-10.0, 0.0)));
        assert!(!vision.in_cone(Vec2::ZERO, Vec2::new(0.0, 10.0)));
    }

    #[test]
    fn a_cone_facing_across_the_angle_seam_has_no_blind_spot() {
        // Facing exactly -X (±PI) is where naive angle comparison breaks: the
        // straight-ahead direction sits on the wrap boundary and reads as
        // maximally far away.
        let vision = VisionProfile::cone(std::f32::consts::PI, std::f32::consts::FRAC_PI_2);
        assert!(
            vision.in_cone(Vec2::ZERO, Vec2::new(-10.0, 0.0)),
            "straight ahead must be inside the cone",
        );
        assert!(vision.in_cone(Vec2::ZERO, Vec2::new(-10.0, 3.0)));
        assert!(vision.in_cone(Vec2::ZERO, Vec2::new(-10.0, -3.0)));
        assert!(!vision.in_cone(Vec2::ZERO, Vec2::new(10.0, 0.0)));
    }

    #[test]
    fn an_observer_always_sees_its_own_position() {
        // The angle to yourself is undefined; it must not read as out-of-cone.
        let vision = VisionProfile::cone(0.0, std::f32::consts::FRAC_PI_4);
        assert!(vision.in_cone(Vec2::ZERO, Vec2::ZERO));
    }

    // --- combined visibility ---------------------------------------------

    #[test]
    fn an_unaided_observer_cannot_see_in_the_dark() {
        let seen = visibility_of(
            Vec2::ZERO,
            &VisionProfile::default(),
            Vec2::new(30.0, 0.0),
            &[],
            &no_walls(),
            AmbientLight::unlit(),
        );
        assert_eq!(seen, Visibility::Hidden);
    }

    #[test]
    fn darkvision_reveals_the_dark_only_dimly_and_only_in_range() {
        let vision = VisionProfile::with_darkvision(60.0);
        let dark = AmbientLight::unlit();

        // Inside range: perceived, but never clearly.
        assert_eq!(
            visibility_of(
                Vec2::ZERO,
                &vision,
                Vec2::new(30.0, 0.0),
                &[],
                &no_walls(),
                dark
            ),
            Visibility::Dim,
        );
        // Beyond range: nothing.
        assert_eq!(
            visibility_of(
                Vec2::ZERO,
                &vision,
                Vec2::new(90.0, 0.0),
                &[],
                &no_walls(),
                dark
            ),
            Visibility::Hidden,
        );
    }

    #[test]
    fn bright_light_is_seen_clearly_and_dim_light_dimly() {
        let torch = ResolvedLight::torch(Vec2::new(30.0, 0.0), 10.0);
        let vision = VisionProfile::default();
        let dark = AmbientLight::unlit();

        // Right at the torch: bright.
        assert_eq!(
            visibility_of(
                Vec2::ZERO,
                &vision,
                Vec2::new(35.0, 0.0),
                &[torch],
                &no_walls(),
                dark
            ),
            Visibility::Clear,
        );
        // In its dim ring.
        assert_eq!(
            visibility_of(
                Vec2::ZERO,
                &vision,
                Vec2::new(45.0, 0.0),
                &[torch],
                &no_walls(),
                dark
            ),
            Visibility::Dim,
        );
    }

    #[test]
    fn a_wall_hides_a_target_however_well_lit_it_is() {
        // The failure this guards: checking illumination but not occlusion, so
        // a brightly-lit enemy in the next room is visible through the wall.
        let walls = wall_between(10.0, -50.0, 10.0, 50.0);
        let torch = ResolvedLight::torch(Vec2::new(30.0, 0.0), 100.0);
        let seen = visibility_of(
            Vec2::ZERO,
            &VisionProfile::default(),
            Vec2::new(30.0, 0.0),
            &[torch],
            &walls,
            AmbientLight::daylight(),
        );
        assert_eq!(seen, Visibility::Hidden);
    }

    #[test]
    fn facing_hides_a_lit_unoccluded_target_behind_the_observer() {
        let vision = VisionProfile {
            facing: Some(0.0),
            fov: std::f32::consts::FRAC_PI_2,
            ..VisionProfile::default()
        };
        let seen = visibility_of(
            Vec2::ZERO,
            &vision,
            Vec2::new(-30.0, 0.0),
            &[],
            &no_walls(),
            AmbientLight::daylight(),
        );
        assert_eq!(seen, Visibility::Hidden);
    }

    #[test]
    fn max_range_caps_sight_even_in_daylight() {
        let vision = VisionProfile {
            max_range: Some(50.0),
            ..VisionProfile::default()
        };
        let day = AmbientLight::daylight();
        assert_eq!(
            visibility_of(
                Vec2::ZERO,
                &vision,
                Vec2::new(40.0, 0.0),
                &[],
                &no_walls(),
                day
            ),
            Visibility::Clear,
        );
        assert_eq!(
            visibility_of(
                Vec2::ZERO,
                &vision,
                Vec2::new(60.0, 0.0),
                &[],
                &no_walls(),
                day
            ),
            Visibility::Hidden,
        );
    }

    #[test]
    fn darkvision_does_not_see_through_walls() {
        // Darkvision defeats darkness, not geometry.
        let walls = wall_between(10.0, -50.0, 10.0, 50.0);
        let seen = visibility_of(
            Vec2::ZERO,
            &VisionProfile::with_darkvision(120.0),
            Vec2::new(30.0, 0.0),
            &[],
            &walls,
            AmbientLight::unlit(),
        );
        assert_eq!(seen, Visibility::Hidden);
    }
}

/// The quadrilateral a wall casts away from a light: its shadow.
///
/// Returned as four points in order, forming a closed quad: the wall's two
/// endpoints, then those endpoints pushed directly away from the light. The
/// caller triangulates and renders it as solid darkness on top of the light
/// pool, which is how a light gets a shadow without any per-fragment occlusion
/// work in a shader.
///
/// `reach` is how far to project. It only has to exceed the light's own dim
/// radius — beyond that there is no light for a shadow to remove — so callers
/// pass the light's outer radius rather than some arbitrary huge number, which
/// keeps the geometry small and avoids precision loss at extreme coordinates.
///
/// Returns `None` when the wall cannot cast a shadow: a degenerate
/// (zero-length) wall, or an endpoint sitting exactly on the light, where the
/// direction to project is undefined.
pub fn shadow_quad(light: Vec2, wall_start: Vec2, wall_end: Vec2, reach: f32) -> Option<[Vec2; 4]> {
    if wall_start.distance_squared(wall_end) <= f32::EPSILON {
        return None;
    }

    let project = |point: Vec2| -> Option<Vec2> {
        let away = point - light;
        if away.length_squared() <= f32::EPSILON {
            return None;
        }
        Some(point + away.normalize() * reach)
    };

    let far_start = project(wall_start)?;
    let far_end = project(wall_end)?;

    // Ordered so the four points trace the perimeter rather than crossing over
    // themselves — a bow-tie here would triangulate into two wrong triangles
    // and leave the shadow's middle lit.
    Some([wall_start, wall_end, far_end, far_start])
}

#[cfg(test)]
mod shadow_tests {
    use super::*;

    #[test]
    fn a_shadow_extends_directly_away_from_the_light() {
        // Light left of a vertical wall: the shadow must fall to the right.
        let quad = shadow_quad(
            Vec2::new(-100.0, 0.0),
            Vec2::new(0.0, -50.0),
            Vec2::new(0.0, 50.0),
            1000.0,
        )
        .expect("a real wall casts a shadow");

        // Near edge is the wall itself.
        assert_eq!(quad[0], Vec2::new(0.0, -50.0));
        assert_eq!(quad[1], Vec2::new(0.0, 50.0));
        // Far edge is beyond the wall, away from the light.
        assert!(quad[2].x > 0.0, "far edge should be right of the wall");
        assert!(quad[3].x > 0.0, "far edge should be right of the wall");
    }

    #[test]
    fn the_quad_traces_its_perimeter_without_crossing() {
        // The failure this guards: ordering the points [start, end, far_start,
        // far_end] makes a bow-tie, whose triangles miss the shadow's middle
        // and leave a lit wedge inside it.
        let quad = shadow_quad(
            Vec2::new(0.0, -100.0),
            Vec2::new(-50.0, 0.0),
            Vec2::new(50.0, 0.0),
            500.0,
        )
        .unwrap();

        // Walking the perimeter of a simple quad turns consistently in one
        // direction; a bow-tie reverses.
        let cross = |a: Vec2, b: Vec2, c: Vec2| (b - a).perp_dot(c - b);
        let signs: Vec<bool> = (0..4)
            .map(|i| cross(quad[i], quad[(i + 1) % 4], quad[(i + 2) % 4]) > 0.0)
            .collect();
        assert!(
            signs.iter().all(|&s| s == signs[0]),
            "shadow quad is self-intersecting: {quad:?}",
        );
    }

    #[test]
    fn reach_controls_how_far_the_shadow_runs() {
        let short = shadow_quad(
            Vec2::ZERO,
            Vec2::new(10.0, -5.0),
            Vec2::new(10.0, 5.0),
            50.0,
        )
        .unwrap();
        let long = shadow_quad(
            Vec2::ZERO,
            Vec2::new(10.0, -5.0),
            Vec2::new(10.0, 5.0),
            500.0,
        )
        .unwrap();
        assert!(long[2].length() > short[2].length());
    }

    #[test]
    fn degenerate_input_casts_no_shadow() {
        // Zero-length wall.
        assert!(shadow_quad(Vec2::ZERO, Vec2::new(5.0, 5.0), Vec2::new(5.0, 5.0), 100.0).is_none());
        // An endpoint exactly on the light — no direction to project.
        assert!(shadow_quad(Vec2::ZERO, Vec2::ZERO, Vec2::new(10.0, 0.0), 100.0).is_none());
    }
}
