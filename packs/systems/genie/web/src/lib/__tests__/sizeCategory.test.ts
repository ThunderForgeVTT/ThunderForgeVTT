/**
 * Tests for Genie's size category -> footprint resolution.
 *
 * node:test + node:assert, matching derived-data.test.ts / conditions.test.ts
 * in this package.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import { DEFAULT_FOOTPRINT, declaredSizesOf, resolveSizeFootprint } from '../sizeCategory.ts';

/** The real manifest, so a test cannot pass against a table nobody ships. */
const MANIFEST = JSON.parse(
  readFileSync(new URL('../../../../system.json', import.meta.url), 'utf8'),
);
const SIZES = declaredSizesOf(MANIFEST);

test('Genie declares its sizes under combat.sizes, not sizeCategories', () => {
  assert.equal(MANIFEST.sizeCategories, undefined);
  assert.deepEqual(SIZES?.source, { slot: 'traitData', field: 'size_category' });
});

test('resolveSizeFootprint resolves a known category to its manifest footprint', () => {
  assert.equal(resolveSizeFootprint(SIZES, 'colossal'), 4);
  assert.equal(resolveSizeFootprint(SIZES, 'diminutive'), 0.5);
});

test('resolveSizeFootprint falls back to one square for an unknown category', () => {
  assert.equal(resolveSizeFootprint(SIZES, 'gigantic'), DEFAULT_FOOTPRINT);
});

test('resolveSizeFootprint falls back to one square when the category is missing', () => {
  assert.equal(resolveSizeFootprint(SIZES, null), DEFAULT_FOOTPRINT);
  assert.equal(resolveSizeFootprint(SIZES, undefined), DEFAULT_FOOTPRINT);
});

test('resolveSizeFootprint falls back to one square when sizes are not declared', () => {
  assert.equal(resolveSizeFootprint(undefined, 'colossal'), DEFAULT_FOOTPRINT);
  assert.equal(resolveSizeFootprint(declaredSizesOf({}), 'colossal'), DEFAULT_FOOTPRINT);
});
