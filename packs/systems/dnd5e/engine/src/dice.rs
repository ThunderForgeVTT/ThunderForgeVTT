//! D&D 5e d20 Dice Roller
//!
//! Deterministic d20 rolling with advantage/disadvantage support.
//! Uses seed-based randomization for reproducibility (native only).
//! WASM: Phase 4.8.1 will add crypto-based randomization for browser.

use serde::{Deserialize, Serialize};

#[cfg(not(target_arch = "wasm32"))]
use rand::rngs::StdRng;
#[cfg(not(target_arch = "wasm32"))]
use rand::{RngExt, SeedableRng};

/// Advantage state for d20 roll
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum RollAdvantage {
    /// Normal roll (1d20)
    Normal,
    /// Advantage (roll 2d20, take higher)
    Advantage,
    /// Disadvantage (roll 2d20, take lower)
    Disadvantage,
}

/// D20 roll result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct D20Roll {
    /// Final result (1-20)
    pub result: i32,
    /// Individual rolls (for advantage/disadvantage, 1 or 2 elements)
    pub rolls: Vec<i32>,
    /// Advantage state used
    pub advantage: RollAdvantage,
    /// Total with modifier (result + modifier)
    pub total: i32,
}

#[cfg(not(target_arch = "wasm32"))]
/// Roll a d20 with optional advantage/disadvantage
pub fn roll_d20(modifier: i32, advantage: RollAdvantage) -> D20Roll {
    // Use system time as seed for reproducibility
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;

    roll_d20_seeded(modifier, advantage, seed)
}

#[cfg(target_arch = "wasm32")]
/// Roll a d20 with optional advantage/disadvantage (WASM stub)
pub fn roll_d20(_modifier: i32, _advantage: RollAdvantage) -> D20Roll {
    // Phase 4.8.1: Implement crypto randomization for WASM
    D20Roll {
        result: 10,
        rolls: vec![10],
        advantage: RollAdvantage::Normal,
        total: 10,
    }
}

#[cfg(not(target_arch = "wasm32"))]
/// Roll a d20 with a specific seed (for testing)
pub fn roll_d20_seeded(modifier: i32, advantage: RollAdvantage, seed: u64) -> D20Roll {
    let mut rng = StdRng::seed_from_u64(seed);

    let result = match advantage {
        RollAdvantage::Normal => {
            let roll = rng.random_range(1..=20);
            vec![roll]
        }
        RollAdvantage::Advantage => {
            let roll1 = rng.random_range(1..=20);
            let roll2 = rng.random_range(1..=20);
            vec![roll1, roll2]
        }
        RollAdvantage::Disadvantage => {
            let roll1 = rng.random_range(1..=20);
            let roll2 = rng.random_range(1..=20);
            vec![roll1, roll2]
        }
    };

    let final_result = match advantage {
        RollAdvantage::Normal => result[0],
        RollAdvantage::Advantage => *result.iter().max().unwrap(),
        RollAdvantage::Disadvantage => *result.iter().min().unwrap(),
    };

    D20Roll {
        result: final_result,
        rolls: result,
        advantage,
        total: final_result + modifier,
    }
}

#[cfg(target_arch = "wasm32")]
/// Roll a d20 with a specific seed (WASM stub)
pub fn roll_d20_seeded(_modifier: i32, _advantage: RollAdvantage, _seed: u64) -> D20Roll {
    // Phase 4.8.1: Implement crypto randomization for WASM
    D20Roll {
        result: 10,
        rolls: vec![10],
        advantage: RollAdvantage::Normal,
        total: 10,
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[test]
    fn test_d20_normal() {
        let roll = roll_d20_seeded(0, RollAdvantage::Normal, 12345);
        assert!(roll.result >= 1 && roll.result <= 20);
        assert_eq!(roll.rolls.len(), 1);
        assert_eq!(roll.total, roll.result);
    }

    #[test]
    fn test_d20_advantage() {
        let roll = roll_d20_seeded(2, RollAdvantage::Advantage, 12345);
        assert_eq!(roll.rolls.len(), 2);
        assert_eq!(roll.result, *roll.rolls.iter().max().unwrap());
        assert_eq!(roll.total, roll.result + 2);
    }

    #[test]
    fn test_d20_disadvantage() {
        let roll = roll_d20_seeded(-1, RollAdvantage::Disadvantage, 12345);
        assert_eq!(roll.rolls.len(), 2);
        assert_eq!(roll.result, *roll.rolls.iter().min().unwrap());
        assert_eq!(roll.total, roll.result - 1);
    }

    #[test]
    fn test_d20_modifier() {
        let roll = roll_d20_seeded(5, RollAdvantage::Normal, 67890);
        assert_eq!(roll.total, roll.result + 5);
    }
}
