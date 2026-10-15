//! A token's glide from one square to the next (spec 099 US1).
//!
//! A snapped token used to teleport a square at a time: a WASD step, a
//! teammate's move from the server, the landing's walker crossing the page.
//! The token's position is still where the rules put it, in the frame they
//! put it there. Only the picture is drawn `offset` away from it, and the
//! offset shrinks to nothing at an even speed.
//!
//! The glide lasts as long as the time since the token's last hop (FR-002).
//! A token stepping steadily therefore glides for exactly the gap between
//! its steps, and its picture moves at an even pace rather than sliding and
//! stopping on every square.

use glam::Vec2;

/// The shortest glide, in seconds: a token stepping faster than this lags
/// its position by at most this long.
pub const MIN_GLIDE_SECS: f32 = 0.08;
/// The longest glide, in seconds: one step after a rest.
pub const MAX_GLIDE_SECS: f32 = 0.25;
/// A hop shorter than this, in squares, is followed at once: a free drag.
pub const MIN_HOP_SQUARES: f32 = 0.25;
/// A hop longer than this, in squares, is a jump: a scene load, a teleport.
pub const MAX_HOP_SQUARES: f32 = 4.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glide {
    /// Where the picture is drawn, from the token's position.
    pub offset: Vec2,
    /// How fast the offset shrinks, in world units per second.
    speed: f32,
    /// Seconds since the last hop that glided.
    since_hop: f32,
}

impl Default for Glide {
    fn default() -> Self {
        Self {
            offset: Vec2::ZERO,
            speed: 0.0,
            // A token at rest takes its first step at the longest glide.
            since_hop: MAX_GLIDE_SECS,
        }
    }
}

impl Glide {
    /// The token's position moved by `delta`. `square` is the grid's size
    /// in world units; `reduced` is reduced motion, where every move jumps.
    pub fn hop(&mut self, delta: Vec2, square: f32, reduced: bool) {
        let length = delta.length();
        if reduced || square <= 0.0 || length > MAX_HOP_SQUARES * square {
            *self = Self::default();
            return;
        }
        if length < MIN_HOP_SQUARES * square {
            // The picture moves with the token: a drag never lags.
            return;
        }
        // The picture stays where it was drawn, and sets off from there.
        self.offset -= delta;
        let secs = self.since_hop.clamp(MIN_GLIDE_SECS, MAX_GLIDE_SECS);
        self.speed = self.offset.length() / secs;
        self.since_hop = 0.0;
    }

    /// Moves the picture on by `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        self.since_hop += dt;
        let left = self.offset.length();
        let travel = self.speed * dt;
        if travel >= left {
            self.offset = Vec2::ZERO;
        } else {
            self.offset -= self.offset * (travel / left);
        }
    }

    pub fn is_gliding(&self) -> bool {
        self.offset != Vec2::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SQUARE: f32 = 100.0;
    const EAST: Vec2 = Vec2::new(SQUARE, 0.0);

    #[test]
    fn a_step_from_rest_is_halfway_at_half_its_glide() {
        let mut g = Glide::default();
        g.hop(EAST, SQUARE, false);
        assert_eq!(g.offset, -EAST);
        g.step(MAX_GLIDE_SECS / 2.0);
        assert!((g.offset.x + SQUARE / 2.0).abs() < 0.01, "{:?}", g.offset);
        g.step(MAX_GLIDE_SECS / 2.0 + 0.001);
        assert!(!g.is_gliding());
    }

    #[test]
    fn a_steady_walker_moves_at_an_even_pace() {
        // A square every 0.15 s, drawn at 60 fps: where the picture is
        // drawn is the token's position plus the offset.
        let mut g = Glide::default();
        let (dt, every) = (1.0 / 60.0, 9);
        let mut position = 0.0;
        let mut drawn = vec![0.0f32];
        for frame in 1..=120 {
            if frame % every == 0 {
                position += SQUARE;
                g.hop(EAST, SQUARE, false);
            }
            g.step(dt);
            drawn.push(position + g.offset.x);
        }
        // After the first step from rest, every frame travels the same.
        let travel: Vec<f32> = drawn.windows(2).map(|w| w[1] - w[0]).skip(30).collect();
        let mean = travel.iter().sum::<f32>() / travel.len() as f32;
        assert!(mean > 0.0);
        for t in travel {
            assert!(t <= 2.0 * mean && t >= 0.0, "{t} against {mean}");
        }
    }

    #[test]
    fn a_long_hop_jumps() {
        let mut g = Glide::default();
        g.hop(EAST, SQUARE, false);
        g.hop(EAST * 5.0, SQUARE, false);
        assert!(!g.is_gliding());
    }

    #[test]
    fn a_small_move_is_followed_at_once() {
        let mut g = Glide::default();
        g.hop(EAST * 0.1, SQUARE, false);
        assert!(!g.is_gliding());
    }

    #[test]
    fn reduced_motion_jumps() {
        let mut g = Glide::default();
        g.hop(EAST, SQUARE, true);
        assert!(!g.is_gliding());
    }

    #[test]
    fn a_step_mid_glide_catches_up_rather_than_falling_behind() {
        let mut g = Glide::default();
        g.hop(EAST, SQUARE, false);
        g.step(0.05);
        g.hop(EAST, SQUARE, false);
        // Behind by more than a square, closed in the shortest glide.
        assert!(g.offset.x < -SQUARE);
        g.step(MIN_GLIDE_SECS + 0.001);
        assert!(!g.is_gliding());
    }
}
