import { test } from "node:test";
import assert from "node:assert/strict";

import {
  addStatus,
  isAdvancement,
  newStatusId,
  removeStatus,
  resolve,
  statusModifier,
  statusesOf,
  type Status,
} from "./game.ts";
import { everyPool } from "./gameTestFixtures.ts";

// ---------------------------------------------------------------------------
// Statuses (US4)
// ---------------------------------------------------------------------------

test("no statuses stored means none, not a default one", () => {
  // Deliberately unlike skillsOf, whose empty store reads as the starting
  // skill. Every character has a skill; almost no character has a status.
  assert.deepEqual(statusesOf(undefined), []);
  assert.deepEqual(statusesOf(null), []);
  assert.deepEqual(statusesOf({}), []);
  assert.deepEqual(statusesOf({ statuses: [] }), []);
  assert.deepEqual(statusesOf({ statuses: "Wounded" }), []);
});

test("statusesOf keeps the rows that are statuses and drops what is not", () => {
  const stored = {
    statuses: [
      { id: "a", name: "Wounded", modifier: -2 },
      { id: "b", name: "Missing a modifier" },
      "Soaked",
      null,
      { id: "c", name: "Fractional", modifier: 1.5 },
      { id: "d", name: "Blessed", modifier: 3 },
    ],
  };
  assert.deepEqual(statusesOf(stored), [
    { id: "a", name: "Wounded", modifier: -2 },
    { id: "d", name: "Blessed", modifier: 3 },
  ]);
});

test("statuses sum, including to zero and to a negative", () => {
  assert.equal(statusModifier([]), 0);
  assert.equal(
    statusModifier([
      { id: "a", name: "Wounded", modifier: -2 },
      { id: "b", name: "Blessed", modifier: 2 },
    ]),
    0,
  );
  assert.equal(
    statusModifier([
      { id: "a", name: "Wounded", modifier: -2 },
      { id: "b", name: "Soaked", modifier: -1 },
      { id: "c", name: "Cornered", modifier: -3 },
    ]),
    -6,
  );
});

test("a status needs a name, and the name is trimmed but never judged", () => {
  const refused = addStatus([], "   ", -2, "a");
  assert.equal(refused.added, false);
  assert.match(refused.added === false ? refused.reason : "", /name/i);

  const fractional = addStatus([], "Wounded", 1.5, "a");
  assert.equal(fractional.added, false);

  const made = addStatus([], "  On Fire  ", -3, "a");
  assert.equal(made.added, true);
  assert.deepEqual(made.added === true ? made.statuses : [], [
    { id: "a", name: "On Fire", modifier: -3 },
  ]);
});

test("two statuses may share a name, and both count", () => {
  const first = addStatus([], "Wounded", -2, "a");
  assert.equal(first.added, true);
  const second = addStatus(
    first.added === true ? first.statuses : [],
    "Wounded",
    -2,
    "b",
  );
  assert.equal(second.added, true);
  const both = second.added === true ? second.statuses : [];
  assert.equal(both.length, 2);
  assert.equal(statusModifier(both), -4);
});

test("removing a status that is not there is not an error", () => {
  const held: Status[] = [{ id: "a", name: "Wounded", modifier: -2 }];
  assert.deepEqual(removeStatus(held, "b"), held);
  assert.deepEqual(removeStatus(held, "a"), []);
  assert.deepEqual(removeStatus([], "a"), []);
});

test("status ids are generated and do not repeat", () => {
  const ids = new Set(Array.from({ length: 200 }, () => newStatusId()));
  assert.equal(ids.size, 200);
});

// ---------------------------------------------------------------------------
// The guarantee (FR-023): a status cannot reach the advancement
// ---------------------------------------------------------------------------

/**
 * This is structural, not remembered. `isAdvancement` takes the faces and the
 * dice bought with experience, and there is no third parameter a status could
 * arrive through — so the rule holds for reasons a future edit would have to
 * work at to break. The arity assertion is what would notice that edit.
 */
test("isAdvancement has no parameter a status could enter by", () => {
  assert.equal(
    isAdvancement.length,
    2,
    "isAdvancement grew a parameter — if a status can now be passed in, " +
      "a status can create or destroy an advancement",
  );
});

test("whatever the statuses, the advancement is the same answer", () => {
  const sets: Status[][] = [
    [],
    [{ id: "a", name: "Blessed", modifier: 100 }],
    [{ id: "a", name: "Doomed", modifier: -100 }],
    [
      { id: "a", name: "Wounded", modifier: -7 },
      { id: "b", name: "Lucky", modifier: 7 },
    ],
  ];

  for (const faces of everyPool()) {
    const truth = isAdvancement(faces, 0);
    for (const statuses of sets) {
      // The statuses are applied to the roll the only way they can be...
      const outcome = resolve({
        faces,
        statuses,
        opposition: 10,
        tieSucceeds: false,
      });
      assert.equal(outcome.modifier, statusModifier(statuses));
      // ...and the advancement still reads the raw faces.
      assert.equal(
        isAdvancement(faces, 0),
        truth,
        `statuses changed the advancement for ${JSON.stringify(faces)}`,
      );
    }
  }
});

test("a −100 status cannot make a positive total, and still cannot spoil all sixes", () => {
  const outcome = resolve({
    faces: [6, 6, 6],
    statuses: [{ id: "a", name: "Doomed", modifier: -100 }],
    opposition: 1,
    tieSucceeds: false,
  });
  assert.equal(outcome.sum, 18);
  assert.equal(outcome.total, -82);
  assert.equal(outcome.verdict, "failure");
  assert.equal(outcome.xpAwarded, 1);
  // The roll failed badly and the character still learned something new.
  assert.equal(isAdvancement([6, 6, 6], 0), true);
});
