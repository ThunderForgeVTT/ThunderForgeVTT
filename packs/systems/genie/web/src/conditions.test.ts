/**
 * Tests for Genie's condition resolver.
 *
 * Mirrors derived-data.test.ts's pattern — a plain node:test + node:assert
 * unit test run directly against the TS source (no bundler/build step
 * required), per spec 018 User Story 4 (T051).
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import { GENIE_CONDITIONS, resolveCondition, resolveConditions } from './conditions.ts';

test('resolveCondition returns the manifest definition for a known key', () => {
  const bound = resolveCondition('bound');
  assert.ok(bound);
  assert.equal(bound.label, 'Bound');
  assert.ok(bound.description.length > 0);
});

test('resolveCondition returns undefined for an unknown key', () => {
  assert.equal(resolveCondition('not_a_real_condition'), undefined);
});

test('resolveConditions resolves a list in order and drops unknown keys', () => {
  const resolved = resolveConditions(['bound', 'not_a_real_condition', 'exposed']);
  assert.deepEqual(
    resolved.map((c) => c.key),
    ['bound', 'exposed'],
  );
});

test('resolveConditions returns an empty list for an empty input', () => {
  assert.deepEqual(resolveConditions([]), []);
});

test('GENIE_CONDITIONS has no duplicate keys', () => {
  const keys = GENIE_CONDITIONS.map((c) => c.key);
  assert.equal(new Set(keys).size, keys.length);
});
