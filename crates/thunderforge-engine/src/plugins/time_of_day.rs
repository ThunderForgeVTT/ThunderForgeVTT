//! The table's clock (spec 099 US2): the hour, and the darkness it brings.
//!
//! `set_time_of_day` sets [`TimeOfDay`]; [`tick_time_of_day`] runs the clock
//! and eases the drawn darkness toward the daylight curve at a rate per
//! second, before `sync_darkness` draws it. The arithmetic is
//! `thunderforge_canvas_core::daylight`. A scene with no time of day has no
//! clock, and its darkness comes from `set_ambient_light` as before.

use std::collections::HashSet;

use bevy::prelude::*;
use serde_json::json;
use thunderforge_canvas_core::daylight::{Clock, NIGHT, darkness_at, follow};

use crate::emit_event;
use crate::resources::{AmbientShade, SceneAmbient};
use crate::systems::background::NightBackgroundSprite;

#[derive(Resource, Default)]
pub struct TimeOfDay {
    pub clock: Option<Clock>,
    /// The darkness as drawn, easing toward the clock's.
    pub shown: f32,
    /// The darkness last told to the page, in hundredths.
    told: Option<i32>,
    /// Lights sent with `byNight` (US3): lit by night, out by day. Kept
    /// here, not on the light, so the server's light type is untouched.
    pub night_lights: HashSet<String>,
}

impl TimeOfDay {
    /// Sets the hour, at once or over `over` seconds; `None` takes the
    /// clock away. A jump is a scene load: the darkness jumps with it.
    pub fn set(&mut self, hour: Option<f32>, over: Option<f32>) {
        let Some(hour) = hour.filter(|h| h.is_finite()) else {
            self.clock = None;
            self.shown = 0.0;
            self.told = None;
            return;
        };
        let jumped = match self.clock.as_mut() {
            Some(clock) => clock.set(hour, over),
            None => {
                self.clock = Some(Clock::at(hour));
                true
            }
        };
        if jumped {
            self.shown = darkness_at(hour);
        }
        self.told = None;
    }

    /// How much of a night light's intensity burns at `darkness`: none by
    /// day, all of it by night, rising and falling with the dusk.
    pub fn night_factor(darkness: f32) -> f32 {
        (darkness / NIGHT).clamp(0.0, 1.0)
    }

    /// The clock's darkness, while there is a clock.
    pub fn shade(&self) -> Option<f32> {
        self.clock.map(|_| self.shown)
    }
}

pub(crate) fn tick_time_of_day(time: Res<Time>, mut tod: ResMut<TimeOfDay>) {
    let dt = time.delta_secs();
    let tod = tod.as_mut();
    let Some(clock) = tod.clock.as_mut() else {
        return;
    };
    clock.step(dt);
    let hour = clock.hour;
    tod.shown = follow(tod.shown, darkness_at(hour), dt);
    let hundredths = (tod.shown * 100.0).round() as i32;
    if tod.told != Some(hundredths) {
        tod.told = Some(hundredths);
        emit_event(json!({
            "type": "time_of_day",
            "hour": hour,
            "darkness": tod.shown,
        }));
    }
}

/// The night map shows as much as the night lights burn (FR-007): no
/// swap, no flash.
pub(crate) fn fade_night_background(
    tod: Res<TimeOfDay>,
    shade: Option<Res<AmbientShade>>,
    ambient: Option<Res<SceneAmbient>>,
    mut night: Query<&mut Sprite, With<NightBackgroundSprite>>,
) {
    let level = ambient.map_or(
        thunderforge_canvas_core::vision::Illumination::Bright,
        |a| a.0.level,
    );
    let darkness = super::darkness::drawn_darkness(shade.as_deref(), tod.shade(), level);
    let alpha = TimeOfDay::night_factor(darkness);
    for mut sprite in night.iter_mut() {
        if (sprite.color.alpha() - alpha).abs() > 0.001 {
            sprite.color.set_alpha(alpha);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// SC-004: 12:00 to 24:00 over a second, at a frame rate.
    fn drive(fps: u32) -> (f32, f32) {
        let mut world = World::new();
        world.init_resource::<TimeOfDay>();
        let mut time = Time::<()>::default();
        world.resource_mut::<TimeOfDay>().set(Some(12.0), None);
        world.resource_mut::<TimeOfDay>().set(Some(24.0), Some(1.0));
        let mut schedule = Schedule::default();
        schedule.add_systems(tick_time_of_day);
        let (mut last, mut worst) = (0.0f32, 0.0f32);
        for _ in 0..fps / 2 {
            time.advance_by(Duration::from_secs_f64(1.0 / fps as f64));
            world.insert_resource(time);
            schedule.run(&mut world);
            let now = world.resource::<TimeOfDay>().shown;
            worst = worst.max((now - last).abs());
            last = now;
        }
        (last, worst)
    }

    #[test]
    fn the_dusk_falls_alike_at_30_and_144_fps() {
        let (slow, worst) = drive(30);
        let (fast, _) = drive(144);
        assert!((slow - fast).abs() <= 0.01, "{slow} against {fast}");
        assert!(worst <= 0.02 + 1e-5, "{worst}");
    }

    #[test]
    fn the_night_map_shows_by_night_and_not_by_day() {
        let mut world = World::new();
        world.init_resource::<TimeOfDay>();
        let map = world
            .spawn((
                Sprite {
                    color: Color::WHITE.with_alpha(0.0),
                    ..default()
                },
                NightBackgroundSprite,
            ))
            .id();
        let mut schedule = Schedule::default();
        schedule.add_systems(fade_night_background);
        let alpha = |world: &mut World, hour: f32, schedule: &mut Schedule| {
            world.resource_mut::<TimeOfDay>().set(Some(hour), None);
            schedule.run(world);
            world.get::<Sprite>(map).unwrap().color.alpha()
        };
        assert_eq!(alpha(&mut world, 0.0, &mut schedule), 1.0);
        assert_eq!(alpha(&mut world, 12.0, &mut schedule), 0.0);
        let dusk = alpha(&mut world, 19.0, &mut schedule);
        assert!(dusk > 0.0 && dusk < 1.0, "{dusk}");
    }

    #[test]
    fn a_scene_load_jumps_and_no_time_is_no_clock() {
        let mut tod = TimeOfDay::default();
        tod.set(Some(0.0), Some(5.0));
        assert!(tod.shade().unwrap() >= 0.98);
        tod.set(None, None);
        assert_eq!(tod.shade(), None);
        assert_eq!(TimeOfDay::night_factor(0.0), 0.0);
        assert_eq!(TimeOfDay::night_factor(0.98), 1.0);
    }
}
