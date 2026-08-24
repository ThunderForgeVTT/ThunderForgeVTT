/**
 * Tests for Genie's derived-data calculators.
 *
 * Mirrors the pattern used to validate dnd5e's calculateMaxSpellSlots — a
 * plain node:test + node:assert unit test run directly against the TS
 * source (no bundler/build step required), per T057.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { calculateMaxWishPoints } from './derived-data.ts';

test('calculateMaxWishPoints follows the wishPoints leveled table in system.json', () => {
  assert.equal(calculateMaxWishPoints(1), 2);
  assert.equal(calculateMaxWishPoints(2), 3);
  assert.equal(calculateMaxWishPoints(3), 4);
  assert.equal(calculateMaxWishPoints(4), 5);
  assert.equal(calculateMaxWishPoints(5), 6);
  assert.equal(calculateMaxWishPoints(6), 7);
  assert.equal(calculateMaxWishPoints(7), 8);
  assert.equal(calculateMaxWishPoints(8), 9);
  assert.equal(calculateMaxWishPoints(9), 10);
  assert.equal(calculateMaxWishPoints(10), 12);
});

test('calculateMaxWishPoints returns 0 for a level below the table range', () => {
  assert.equal(calculateMaxWishPoints(0), 0);
});

test('calculateMaxWishPoints clamps to the highest defined level above the table range', () => {
  // No entry for level 11+ yet; falls back to the highest known level's value
  // rather than silently returning 0, matching the "still a valid character" case.
  assert.equal(calculateMaxWishPoints(11), 12);
});
