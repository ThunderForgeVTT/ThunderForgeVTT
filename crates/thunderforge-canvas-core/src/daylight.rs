//! The time of day and the light it brings (spec 099 US2).
//!
//! One clock, one curve. The clock is an hour that runs on past 24 into the
//! next day, so passing the night from 22:00 is a move to 30:00, never back
//! through the afternoon. The curve turns an hour into how dark the map is
//! and what colour its shadow takes. The engine eases what it draws toward
//! the curve at a rate per second, so a transition looks the same at any
//! frame rate.

use crate::vision::Rgb;

/// How dark the night is: the map shows, faintly, under the lights.
pub const NIGHT: f32 = 0.98;
/// How dark it is when dusk gives way to the night, at 20:00.
pub const NIGHTFALL: f32 = 0.6;
/// The fastest the drawn darkness may change, per second: 0.02 a frame at
/// 30 frames a second (FR-005).
pub const MAX_SHADE_PER_SEC: f32 = 0.6;

/// The darkness at each turn of the day, as `(hour, darkness)`.
const CURVE: [(f32, f32); 6] = [
    (0.0, NIGHT),
    (5.0, NIGHT),
    (7.0, 0.0),
    (18.0, 0.0),
    (20.0, NIGHTFALL),
    (24.0, NIGHT),
];

/// The shadow's colour as dusk falls, and once night has.
const DUSK: Rgb = Rgb {
    r: 0.30,
    g: 0.12,
    b: 0.04,
};
const NIGHT_BLUE: Rgb = Rgb {
    r: 0.02,
    g: 0.03,
    b: 0.08,
};

/// Eases between two turns of the day. Partly linear so its steepest point
/// is gentle: no tenth of an hour changes the darkness by more than 0.06.
fn ease(t: f32) -> f32 {
    let smooth = t * t * (3.0 - 2.0 * t);
    0.6 * t + 0.4 * smooth
}

/// How dark the map is at `hour`, 0 at noon to [`NIGHT`] at midnight.
pub fn darkness_at(hour: f32) -> f32 {
    let h = hour.rem_euclid(24.0);
    for pair in CURVE.windows(2) {
        let ((h0, d0), (h1, d1)) = (pair[0], pair[1]);
        if h <= h1 {
            let t = ((h - h0) / (h1 - h0)).clamp(0.0, 1.0);
            return d0 + (d1 - d0) * ease(t);
        }
    }
    NIGHT
}

/// The shadow's colour at a darkness: amber at dusk, through violet, to
/// night blue. Blended in OKLCh so it never passes through grey.
pub fn tint_at(darkness: f32) -> Rgb {
    let t = ((darkness - 0.2) / (NIGHT - 0.2)).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    let (a, b) = (oklch(DUSK), oklch(NIGHT_BLUE));
    let mut dh = b.2 - a.2;
    if dh > std::f32::consts::PI {
        dh -= std::f32::consts::TAU;
    } else if dh < -std::f32::consts::PI {
        dh += std::f32::consts::TAU;
    }
    from_oklch((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, a.2 + dh * t))
}

/// The time of day: an hour, and where it is going.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clock {
    pub hour: f32,
    target: f32,
    /// Hours per second, while the clock moves to `target`.
    speed: f32,
}

impl Clock {
    pub fn at(hour: f32) -> Self {
        Self {
            hour,
            target: hour,
            speed: 0.0,
        }
    }

    /// Sets the clock to `hour`: at once, or at an even pace over `over`
    /// seconds. Returns whether it jumped.
    pub fn set(&mut self, hour: f32, over: Option<f32>) -> bool {
        match over {
            Some(secs) if secs > 0.0 && secs.is_finite() => {
                self.target = hour;
                self.speed = (hour - self.hour).abs() / secs;
                false
            }
            _ => {
                *self = Self::at(hour);
                true
            }
        }
    }

    /// Moves the clock on by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let left = self.target - self.hour;
        let travel = self.speed * dt;
        if travel >= left.abs() {
            self.hour = self.target;
            self.speed = 0.0;
        } else {
            self.hour += travel * left.signum();
        }
    }

    pub fn is_moving(&self) -> bool {
        self.hour != self.target
    }
}

/// Moves the drawn darkness toward `target`, no faster than
/// [`MAX_SHADE_PER_SEC`].
pub fn follow(shown: f32, target: f32, dt: f32) -> f32 {
    let most = MAX_SHADE_PER_SEC * dt;
    shown + (target - shown).clamp(-most, most)
}

// OKLab, from Björn Ottosson's reference, over linear sRGB.

fn oklch(c: Rgb) -> (f32, f32, f32) {
    let l = 0.412_221_46 * c.r + 0.536_332_55 * c.g + 0.051_445_995 * c.b;
    let m = 0.211_903_5 * c.r + 0.680_699_5 * c.g + 0.107_396_96 * c.b;
    let s = 0.088_302_46 * c.r + 0.281_718_85 * c.g + 0.629_978_7 * c.b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    let lab_l = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let b = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    (lab_l, a.hypot(b), b.atan2(a))
}

fn from_oklch((lab_l, chroma, hue): (f32, f32, f32)) -> Rgb {
    let (a, b) = (chroma * hue.cos(), chroma * hue.sin());
    let l = lab_l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m = lab_l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s = lab_l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l, m, s) = (l * l * l, m * m * m, s * s * s);
    Rgb {
        r: (4.076_741_7 * l - 3.307_711_6 * m + 0.230_969_94 * s).max(0.0),
        g: (-1.268_438 * l + 2.609_757_4 * m - 0.341_319_38 * s).max(0.0),
        b: (-0.004_196_086 * l - 0.703_418_6 * m + 1.707_614_7 * s).max(0.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_bright_and_midnight_dark() {
        assert_eq!(darkness_at(12.0), 0.0);
        assert!(darkness_at(0.0) >= 0.98);
        assert!(darkness_at(24.0) >= 0.98);
        assert!(darkness_at(3.0) >= 0.98);
        assert_eq!(darkness_at(12.0 + 24.0), 0.0);
    }

    #[test]
    fn no_tenth_of_an_hour_changes_the_darkness_by_more_than_006() {
        let mut worst: f32 = 0.0;
        for i in 0..=2400 {
            let h = i as f32 / 100.0;
            worst = worst.max((darkness_at(h + 0.1) - darkness_at(h)).abs());
        }
        assert!(worst <= 0.06, "{worst}");
    }

    #[test]
    fn dusk_passes_through_colour_not_grey() {
        for i in 0..=20 {
            let (_, chroma, _) = oklch(tint_at(i as f32 / 20.0));
            assert!(chroma > 0.03, "chroma {chroma} at {}", i as f32 / 20.0);
        }
        let night = tint_at(NIGHT);
        assert!((night.b - NIGHT_BLUE.b).abs() < 0.005, "{night:?}");
    }

    #[test]
    fn the_clock_moves_at_an_even_pace_and_on_into_the_next_day() {
        let mut clock = Clock::at(22.0);
        assert!(!clock.set(30.0, Some(4.0)));
        clock.step(1.0);
        assert!((clock.hour - 24.0).abs() < 1e-4);
        clock.step(1.0);
        assert!((clock.hour - 26.0).abs() < 1e-4);
        assert!(darkness_at(clock.hour) >= 0.98);
        clock.step(5.0);
        assert_eq!(clock.hour, 30.0);
        assert!(!clock.is_moving());
        assert!(clock.set(12.0, None));
        assert_eq!(clock.hour, 12.0);
    }

    #[test]
    fn the_shade_follows_at_the_same_pace_at_any_frame_rate() {
        // SC-004 in arithmetic: 12 -> 24 over a second.
        let at = |fps: f32| {
            let mut clock = Clock::at(12.0);
            clock.set(24.0, Some(1.0));
            let (dt, mut shown, mut worst) = (1.0 / fps, 0.0f32, 0.0f32);
            for _ in 0..(fps / 2.0) as usize {
                clock.step(dt);
                let next = follow(shown, darkness_at(clock.hour), dt);
                worst = worst.max(next - shown);
                shown = next;
            }
            (shown, worst)
        };
        let (slow, worst) = at(30.0);
        let (fast, _) = at(144.0);
        assert!((slow - fast).abs() <= 0.01, "{slow} {fast}");
        assert!(worst <= 0.02 + 1e-6, "{worst}");
    }
}
