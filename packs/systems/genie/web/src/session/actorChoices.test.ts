/**
 * The session-resource actor picker's list, as a rule rather than a render.
 *
 * Owner, 2026-09-15: the Clocks & Timers surface offered a dropdown with
 * nothing in it, because the list was the *party* — player characters only
 * — and the world being played had NPCs. These tests pin what replaced it.
 *
 * node:test + node:assert, matching this package's other tests.
 */

import { test } from 'node:test';
import assert from 'node:assert/strict';

import {
  CHARACTERS_GROUP_LABEL,
  NPCS_GROUP_LABEL,
  groupActorChoices,
} from './actorChoices.ts';

const actor = (id: string, label: string, isNpc: boolean) => ({ id, label, isNpc });

test('an NPC is offered — the list is actors, not the party', () => {
  const groups = groupActorChoices([actor('a', 'Shopkeeper', true)]);
  assert.deepEqual(
    groups.map((group) => group.label),
    [NPCS_GROUP_LABEL],
  );
  assert.deepEqual(
    groups[0].actors.map((a) => a.id),
    ['a'],
  );
});

test('a world with only NPCs is not an empty dropdown', () => {
  // The defect itself: every actor in the world was an NPC, so the old
  // `!isNpc` filter left nothing to pick.
  const groups = groupActorChoices([
    actor('a', 'Guard', true),
    actor('b', 'Innkeeper', true),
  ]);
  assert.equal(groups.length, 1);
  assert.equal(groups[0].actors.length, 2);
});

test('characters come first and NPCs last, each group labelled', () => {
  const groups = groupActorChoices([
    actor('n1', 'Zahra the Broker', true),
    actor('p1', 'Aurelia', false),
    actor('n2', 'Ash', true),
    actor('p2', 'Brannic', false),
  ]);
  assert.deepEqual(
    groups.map((group) => group.label),
    [CHARACTERS_GROUP_LABEL, NPCS_GROUP_LABEL],
  );
  assert.deepEqual(
    groups[0].actors.map((a) => a.label),
    ['Aurelia', 'Brannic'],
  );
  assert.deepEqual(
    groups[1].actors.map((a) => a.label),
    ['Ash', 'Zahra the Broker'],
  );
});

test('a numbered roster sorts the way it was written, not the way ASCII would', () => {
  const groups = groupActorChoices([
    actor('c', 'Guard 10', true),
    actor('a', 'Guard 2', true),
    actor('b', 'Guard 9', true),
  ]);
  assert.deepEqual(
    groups[0].actors.map((a) => a.label),
    ['Guard 2', 'Guard 9', 'Guard 10'],
  );
});

test('two identically named NPCs keep a stable order', () => {
  const first = groupActorChoices([actor('b', 'Rat', true), actor('a', 'Rat', true)]);
  const second = groupActorChoices([actor('a', 'Rat', true), actor('b', 'Rat', true)]);
  assert.deepEqual(
    first[0].actors.map((a) => a.id),
    second[0].actors.map((a) => a.id),
  );
});

test('an empty world yields no groups at all, not two empty ones', () => {
  assert.deepEqual(groupActorChoices([]), []);
});

test('the caller does not have its array reordered underneath it', () => {
  const input = [actor('b', 'Brannic', false), actor('a', 'Aurelia', false)];
  groupActorChoices(input);
  assert.deepEqual(
    input.map((a) => a.id),
    ['b', 'a'],
  );
});
