//! The dice, for the browser.
//!
//! The demo (spec 074) has no server, so its in-page backend is the party
//! that rolls. It rolls through this: the same parser and evaluator
//! `rollDice` runs (`crates/thunderforge-server/src/graphql/mutations_roll.rs`),
//! compiled for a different target, so a formula the server refuses is
//! refused here in the same words and a formula it accepts resolves by the
//! same rules. Not a second dice language.
//!
//! The crate still owns no entropy (research.md §3). The caller hands in a
//! seed — fresh from `crypto.getRandomValues` in play, fixed in a test — and
//! the generator below turns it into as many draws as the formula needs.

use std::collections::HashMap;

use wasm_bindgen::prelude::*;

use crate::{DiceFormula, resolve};

/// xoshiro128** (Blackman and Vigna): small, fast, and good enough for dice
/// whose seed is the browser's own cryptographic randomness.
struct Xoshiro128 {
    s: [u32; 4],
}

impl Xoshiro128 {
    fn new(seed: &[u32]) -> Self {
        let mut s = [0x9e37_79b9, 0x243f_6a88, 0xb7e1_5162, 0x1234_5678];
        for (slot, word) in s.iter_mut().zip(seed) {
            *slot ^= *word;
        }
        // All-zero is the one state the generator never leaves.
        if s == [0; 4] {
            s[0] = 1;
        }
        Xoshiro128 { s }
    }

    fn next(&mut self) -> u32 {
        let result = self.s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = self.s[1] << 9;
        self.s[2] ^= self.s[0];
        self.s[3] ^= self.s[1];
        self.s[1] ^= self.s[2];
        self.s[0] ^= self.s[3];
        self.s[2] ^= t;
        self.s[3] = self.s[3].rotate_left(11);
        result
    }
}

impl rand_core::TryRng for Xoshiro128 {
    type Error = std::convert::Infallible;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(self.next())
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok((u64::from(self.next()) << 32) | u64::from(self.next()))
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        for chunk in dest.chunks_mut(4) {
            let bytes = self.next().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

/// `validateDiceFormula`: does the formula parse. No dice are rolled.
#[wasm_bindgen(js_name = validateFormula)]
pub fn validate_formula(formula: &str) -> bool {
    DiceFormula::parse(formula).is_ok()
}

/// Resolve `formula`, substituting `bindings` (a JSON object of placeholder
/// name to number, or empty), with draws from `seed`.
///
/// Answers the `RollResolution` as JSON — the same value the server stores in
/// a roll record's `detail` — or throws the `FormulaError`'s own message.
#[wasm_bindgen]
pub fn roll(formula: &str, bindings: &str, seed: &[u32]) -> Result<String, JsError> {
    let parsed = DiceFormula::parse(formula).map_err(|e| JsError::new(&e.to_string()))?;
    let bindings: HashMap<String, f64> = if bindings.trim().is_empty() {
        HashMap::new()
    } else {
        serde_json::from_str(bindings).map_err(|e| JsError::new(&e.to_string()))?
    };
    let mut rng = Xoshiro128::new(seed);
    let resolution =
        resolve(&parsed, &bindings, &mut rng).map_err(|e| JsError::new(&e.to_string()))?;
    serde_json::to_string(&resolution).map_err(|e| JsError::new(&e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_seed_rolls_the_same_dice_twice() {
        let a = {
            let f = DiceFormula::parse("4d6kh3").unwrap();
            resolve(&f, &HashMap::new(), &mut Xoshiro128::new(&[7, 8, 9, 10])).unwrap()
        };
        let b = {
            let f = DiceFormula::parse("4d6kh3").unwrap();
            resolve(&f, &HashMap::new(), &mut Xoshiro128::new(&[7, 8, 9, 10])).unwrap()
        };
        assert_eq!(a, b);
        assert_eq!(a.dice.len(), 4);
    }

    #[test]
    fn an_all_zero_seed_still_rolls() {
        let mut rng = Xoshiro128::new(&[0x9e37_79b9, 0x243f_6a88, 0xb7e1_5162, 0x1234_5678]);
        // A stuck generator answers 0 for ever; this one moves off it.
        assert!((0..8).map(|_| rng.next()).any(|word| word != 0));
    }

    #[test]
    fn validate_matches_the_server_check() {
        assert!(validate_formula("1d20 + STAT + MODIFIERS"));
        assert!(!validate_formula("1d20 +"));
    }
}
