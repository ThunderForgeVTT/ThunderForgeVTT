//! Genie Derived Data Calculators
//!
//! Client-side calculation functions that derive stats from base data.
//! Mirrors packs/systems/dnd5e/web/src/derived-data.ts's calculateMaxSpellSlots
//! pattern (spec 018-genie-house-system, User Story 6 / research.md R4):
//! a pure function reading a fixed, level-keyed lookup table structurally
//! identical to the one on the manifest (packs/systems/genie/system.json's
//! `wishPoints` field), recomputed on read rather than cached/stored.

/**
 * Maximum Wish Points by character level
 *
 * Returns the maximum Wish Points total for a given character level, per
 * the `wishPoints` leveled table in packs/systems/genie/system.json.
 * Genie has a single resource tier (no multi-tier split the way 5e's spell
 * slots do), so each table entry is a single-element array.
 *
 * @param characterLevel - Character level (1+)
 * @returns Maximum Wish Points available at that level
 */
export function calculateMaxWishPoints(characterLevel: number): number {
  // Table of Wish Points by character level, mirroring the
  // `wishPoints` field in packs/systems/genie/system.json.
  const wishPointsByLevel: Record<number, number[]> = {
    1: [2],
    2: [3],
    3: [4],
    4: [5],
    5: [6],
    6: [7],
    7: [8],
    8: [9],
    9: [10],
    10: [12],
  };

  if (characterLevel < 1) {
    return 0;
  }

  const maxTableLevel = Math.max(...Object.keys(wishPointsByLevel).map(Number));
  const clampedLevel = Math.min(characterLevel, maxTableLevel);

  const slots = wishPointsByLevel[clampedLevel];
  return slots ? slots[0] : 0;
}
