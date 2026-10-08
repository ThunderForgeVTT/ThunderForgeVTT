//! The seeded tumble (research R3).
//!
//! Everything about how a die moves comes from the roll's id and the die's
//! index, so every board plays the same throw (FR-008). Positions are in
//! screen pixels from the throw's anchor, the middle of the lower third of
//! the viewport, with y up.

use std::f32::consts::TAU;

use glam::{Quat, Vec2, Vec3};

/// A die's size on screen, in pixels.
pub const DIE_PX: f32 = 56.0;
/// The distance between neighbouring resting places.
pub const SLOT_PX: f32 = 96.0;
/// Resting places per row; a throw of more dice takes a second row.
pub const PER_ROW: usize = 10;
/// The most a resting place moves off its slot: a quarter of a die.
pub const JITTER_PX: f32 = DIE_PX / 4.0;

/// FNV-1a, 64-bit, over the roll id's UTF-8 bytes.
pub fn seed(roll_id: &str) -> u64 {
    roll_id.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A splitmix64 stream of numbers for one die.
#[derive(Debug, Clone)]
pub struct Stream(u64);

impl Stream {
    /// The stream for die `index` of a throw. A d100's units die is
    /// sub-stream 1 of its index.
    pub fn new(seed: u64, index: usize, sub: u32) -> Self {
        Self(seed ^ index as u64 ^ (u64::from(sub) << 48))
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// In `[0, 1)`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Where a die comes from, where it rests, and how it turns on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct DiePath {
    pub entry: Vec2,
    pub rest: Vec2,
    /// The axis it tumbles about.
    pub axis: Vec3,
    /// Whole turns, 2 to 4.
    pub turns: u32,
    /// The landed spin about `+z`, one per mesh (a d100 has two).
    pub spins: [f32; 2],
}

/// The anchor's place on screen, from the screen's centre: the middle of the
/// lower third of a viewport `size` pixels across.
pub fn anchor(size: Vec2) -> Vec2 {
    Vec2::new(0.0, -size.y / 3.0)
}

/// The path of drawn die `index` of `count`, in a viewport `height` pixels
/// tall. Only the entry depends on the height; the resting place is the
/// same on every board.
pub fn die_path(seed: u64, index: usize, count: usize, height: f32) -> DiePath {
    let mut s = Stream::new(seed, index, 0);
    let rows = count.div_ceil(PER_ROW).max(1);
    let row = index / PER_ROW;
    let in_row = if row + 1 < rows {
        PER_ROW
    } else {
        count - row * PER_ROW
    }
    .max(1);
    let col = index % PER_ROW;
    let slot = Vec2::new(
        (col as f32 - (in_row as f32 - 1.0) / 2.0) * SLOT_PX,
        ((rows as f32 - 1.0) / 2.0 - row as f32) * SLOT_PX,
    );
    let entry_x = slot.x + (s.next_f32() - 0.5) * 2.0 * SLOT_PX;
    let jitter = Vec2::new(s.next_f32() - 0.5, s.next_f32() - 0.5) * 2.0 * JITTER_PX;
    let theta = s.next_f32() * TAU;
    let z = s.next_f32() * 2.0 - 1.0;
    let r = (1.0 - z * z).sqrt();
    let axis = Vec3::new(r * theta.cos(), r * theta.sin(), z).normalize_or(Vec3::X);
    let turns = 2 + (s.next_f32() * 3.0) as u32;
    let spin = s.next_f32() * TAU;
    let spin_units = Stream::new(seed, index, 1).next_f32() * TAU;
    DiePath {
        entry: Vec2::new(entry_x, -(height / 6.0) - DIE_PX),
        rest: slot + jitter.clamp_length_max(JITTER_PX),
        axis,
        turns: turns.min(4),
        spins: [spin, spin_units],
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Out-back: a small overshoot before it settles.
fn ease_out_back(t: f32) -> f32 {
    const C1: f32 = 1.2;
    let u = t - 1.0;
    1.0 + (C1 + 1.0) * u * u * u + C1 * u * u
}

/// Where the die is and how it is turned at `t` in `[0, 1]` of its tumble.
/// At `t = 1` it is exactly at rest in its landing orientation.
pub fn pose(path: &DiePath, landing: Quat, t: f32) -> (Vec2, Quat) {
    let t = t.clamp(0.0, 1.0);
    if t >= 1.0 {
        return (path.rest, landing);
    }
    let position = path.entry + (path.rest - path.entry) * ease_out_back(t);
    let angle = path.turns as f32 * TAU * (1.0 - ease_out_cubic(t));
    (position, landing * Quat::from_axis_angle(path.axis, angle))
}

#[cfg(test)]
#[path = "tumble_tests.rs"]
mod tests;
