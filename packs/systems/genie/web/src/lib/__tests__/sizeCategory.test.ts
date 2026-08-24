/**
 * Tests for Genie's size-category -> token-scale resolution.
 *
 * Converted to node:test + node:assert to match the convention already
 * established by derived-data.test.ts / conditions.test.ts in this package
 * (no vitest dependency exists anywhere in this repo yet — see spec 018
 * tasks.md T046/T050 notes).
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { DEFAULT_SIZE_SCALE, resolveSizeScale, type SizeCategoriesLookup } from '../sizeCategory.ts';

const SIZE_CATEGORIES: SizeCategoriesLookup = {
  diminutive: { scale: 0.5, label: 'Diminutive' },
  small: { scale: 0.75, label: 'Small' },
  medium: { scale: 1.0, label: 'Medium' },
  large: { scale: 2.0, label: 'Large' },
  huge: { scale: 3.0, label: 'Huge' },
  colossal: { scale: 4.0, label: 'Colossal' },
};

test('resolveSizeScale resolves a known category to its manifest scale', () => {
  assert.equal(resolveSizeScale(SIZE_CATEGORIES, 'colossal'), 4.0);
  assert.equal(resolveSizeScale(SIZE_CATEGORIES, 'diminutive'), 0.5);
});

test('resolveSizeScale falls back to the default scale for an unknown category', () => {
  assert.equal(resolveSizeScale(SIZE_CATEGORIES, 'gigantic'), DEFAULT_SIZE_SCALE);
});

test('resolveSizeScale falls back to the default scale when the category is missing', () => {
  assert.equal(resolveSizeScale(SIZE_CATEGORIES, null), DEFAULT_SIZE_SCALE);
  assert.equal(resolveSizeScale(SIZE_CATEGORIES, undefined), DEFAULT_SIZE_SCALE);
});

test('resolveSizeScale falls back to the default scale when the lookup table itself is missing', () => {
  assert.equal(resolveSizeScale(undefined, 'colossal'), DEFAULT_SIZE_SCALE);
  assert.equal(resolveSizeScale(null, 'colossal'), DEFAULT_SIZE_SCALE);
});
