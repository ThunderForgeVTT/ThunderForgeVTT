//! Dice a fight can be given (spec 079 FR-007).
//!
//! The rules never own entropy: every roll takes an injected [`rand_core::Rng`]
//! (as `thunderforge_dice` does). The server passes its own; these are the two
//! the demo and the tests need, with no OS entropy behind either.
//!
//! - [`SeededDice`] — a SplitMix64 stream. Real rolls in play when seeded from
//!   the browser's randomness; one fixed fight when seeded with a constant.
//! - [`ScriptedDice`] — named faces, for a test that has to say "the goblin
//!   rolls a 14".

use std::convert::Infallible;

/// SplitMix64: small, fast, and every seed gives a full-period stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeededDice {
    state: u64,
}

impl SeededDice {
    pub fn new(seed: u64) -> Self {
        SeededDice { state: seed }
    }

    /// Where the stream has reached, so a caller can keep it between calls.
    pub fn state(&self) -> u64 {
        self.state
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

impl rand_core::TryRng for SeededDice {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok((self.next() >> 32) as u32)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.next())
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

/// Dice that come up as named, in order, and then round again.
///
/// The dice crate rolls a face as `1 + next_u32 % sides`, so a face is given
/// as itself: `ScriptedDice::faces([14, 6])` rolls a 14 on the d20 and a 6 on
/// the d8 that follows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptedDice {
    draws: Vec<u32>,
    at: usize,
}

impl ScriptedDice {
    pub fn faces(faces: impl IntoIterator<Item = u32>) -> Self {
        let draws: Vec<u32> = faces.into_iter().map(|f| f.max(1) - 1).collect();
        ScriptedDice {
            draws: if draws.is_empty() { vec![0] } else { draws },
            at: 0,
        }
    }
}

impl rand_core::TryRng for ScriptedDice {
    type Error = Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let draw = self.draws[self.at % self.draws.len()];
        self.at += 1;
        Ok(draw)
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(self.try_next_u32()? as u64)
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for b in dest.iter_mut() {
            *b = self.try_next_u32()? as u8;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attack::roll;
    use thunderforge_dice::PlaceholderBindings;

    #[test]
    fn scripted_faces_come_up_as_named() {
        let mut dice = ScriptedDice::faces([14, 6]);
        let (_, total) = roll("1d20+4", &PlaceholderBindings::new(), &mut dice).unwrap();
        assert_eq!(total, 18.0);
        let (_, total) = roll("1d8+2", &PlaceholderBindings::new(), &mut dice).unwrap();
        assert_eq!(total, 8.0);
    }

    #[test]
    fn one_seed_is_one_fight() {
        let draw = |seed| {
            let mut dice = SeededDice::new(seed);
            (0..20)
                .map(|_| {
                    roll("1d20", &PlaceholderBindings::new(), &mut dice)
                        .unwrap()
                        .1
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(draw(7), draw(7));
        assert_ne!(draw(7), draw(8));
        assert!(draw(7).iter().all(|v| (1.0..=20.0).contains(v)));
    }
}
