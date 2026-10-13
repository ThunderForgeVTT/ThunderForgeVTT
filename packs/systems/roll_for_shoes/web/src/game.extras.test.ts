import { test } from "node:test";
import assert from "node:assert/strict";

import {
  BAND_DICE,
  BAND_TARGET,
  BANDS,
  DEFAULT_SETTINGS,
  isAdvancement,
  resolve,
  statusModifier,
  verdict,
  xpAward,
  type Status,
} from "./game.ts";
import { everyPool } from "./gameTestFixtures.ts";

// ---------------------------------------------------------------------------
// Extras (spec 062)
//
// Every test below belongs to a setting that is off by default. The first one
// is the exception and the point: it is the claim that with all of them off,
// the game is still exactly the game spec 061 shipped.
// ---------------------------------------------------------------------------

/**
 * **The single most important test in this feature.**
 *
 * A world that enables none of the Extras must play the core game, and this is
 * the only place that claim is checked exhaustively rather than by example. If
 * `resolve` ever disagrees with spec 061's `verdict`/`xpAward` on any pool and
 * any opposition, every existing world's rules have silently changed.
 */
test("with the defaults, resolve is the core game and nothing else", () => {
  const oppositions: (number | null)[] = [null];
  for (let opposition = 0; opposition <= 20; opposition += 1) {
    oppositions.push(opposition);
  }

  for (const faces of everyPool()) {
    const sum = faces.reduce((running, face) => running + face, 0);
    for (const opposition of oppositions) {
      const outcome = resolve({
        faces,
        statuses: [],
        opposition,
        tieSucceeds: DEFAULT_SETTINGS.tieSucceeds,
      });
      const expected = verdict(sum, opposition);

      assert.equal(outcome.sum, sum);
      assert.equal(outcome.modifier, 0, "no statuses means no modifier");
      assert.equal(outcome.total, sum, "with no statuses the total is the sum");
      assert.equal(
        outcome.verdict,
        expected,
        `resolve disagreed with spec 061 on ${JSON.stringify(faces)} vs ${opposition}`,
      );
      assert.equal(outcome.xpAwarded, xpAward(expected));
    }
  }
});

test("statuses sum, and an empty set is worth nothing", () => {
  assert.equal(statusModifier([]), 0);
  const carried: Status[] = [
    { id: "a", name: "Winded", modifier: -2 },
    { id: "b", name: "Inspired", modifier: 3 },
    { id: "c", name: "Bleeding", modifier: -1 },
  ];
  assert.equal(statusModifier(carried), 0);
  assert.equal(statusModifier(carried.slice(0, 2)), 1);
});

/**
 * The order is dice → sum → statuses → opposition → comparison. Two things
 * follow from it, and both are load-bearing: a status moves the total rather
 * than the dice, and so it can neither create nor destroy an advancement.
 */
test("a status moves the total, never the dice", () => {
  const faces = [6, 6, 6];
  const blessed = resolve({
    faces,
    statuses: [{ id: "a", name: "Blessed", modifier: 4 }],
    opposition: 20,
    tieSucceeds: false,
  });

  assert.equal(blessed.sum, 18, "the dice are the dice");
  assert.equal(blessed.modifier, 4);
  assert.equal(blessed.total, 22);
  assert.equal(blessed.verdict, "success");
  // The advancement reads the same faces and cannot see the status at all.
  assert.equal(isAdvancement(faces, 0), true);
});

test("a status cannot destroy an advancement, however bad it is", () => {
  const faces = [6, 6];
  const cursed = resolve({
    faces,
    statuses: [{ id: "a", name: "Cursed", modifier: -12 }],
    opposition: 5,
    tieSucceeds: false,
  });

  assert.equal(cursed.total, 0, "the modifier is flat and may go below zero");
  assert.equal(cursed.verdict, "failure");
  assert.equal(cursed.xpAwarded, 1);
  assert.equal(isAdvancement(faces, 0), true, "every die still showed a six");
});

test("a status cannot create an advancement either", () => {
  const faces = [5, 5];
  assert.equal(
    resolve({
      faces,
      statuses: [{ id: "a", name: "Lucky", modifier: 2 }],
      opposition: null,
      tieSucceeds: false,
    }).total,
    12,
    "the total reaches what three sixes would, and means nothing by it",
  );
  assert.equal(isAdvancement(faces, 0), false);
});

test("an unjudged roll stays unjudged and pays nothing, statuses or not", () => {
  const outcome = resolve({
    faces: [4, 4],
    statuses: [{ id: "a", name: "Winded", modifier: -3 }],
    opposition: null,
    tieSucceeds: true,
  });
  assert.equal(outcome.verdict, "unjudged");
  assert.equal(outcome.xpAwarded, 0);
  assert.equal(outcome.total, 5, "the total is still computed");
});

test("with the tie rule on, matching the opposition succeeds and pays nothing", () => {
  const tied = resolve({
    faces: [3, 3],
    statuses: [],
    opposition: 6,
    tieSucceeds: true,
  });
  assert.equal(tied.verdict, "success");
  assert.equal(
    tied.xpAwarded,
    0,
    "a success has never paid, and a tie is now one",
  );

  // The same roll in a world that has not turned it on.
  const core = resolve({
    faces: [3, 3],
    statuses: [],
    opposition: 6,
    tieSucceeds: false,
  });
  assert.equal(core.verdict, "failure");
  assert.equal(core.xpAwarded, 1);
});

test("the tie rule changes only the tie", () => {
  for (let total = 0; total <= 12; total += 1) {
    for (let opposition = 0; opposition <= 12; opposition += 1) {
      if (total === opposition) {
        continue;
      }
      const faces = [total];
      const lenient = resolve({
        faces,
        statuses: [],
        opposition,
        tieSucceeds: true,
      });
      const strict = resolve({
        faces,
        statuses: [],
        opposition,
        tieSucceeds: false,
      });
      assert.equal(lenient.verdict, strict.verdict);
      assert.equal(lenient.xpAwarded, strict.xpAwarded);
    }
  }
});

test("the difficulty bands are the four the game names, in order", () => {
  assert.deepEqual([...BANDS], ["easy", "moderate", "hard", "veryHard"]);
  assert.deepEqual(
    BANDS.map((band) => BAND_DICE[band]),
    [1, 2, 3, 4],
  );
  assert.deepEqual(
    BANDS.map((band) => BAND_TARGET[band]),
    [3, 6, 9, 12],
  );
});

test("a rolled band is judged by the same comparison a named number is", () => {
  // The Game Master rolled two dice for Moderate and got 7.
  const gmTotal = 7;
  assert.equal(
    resolve({
      faces: [4, 3],
      statuses: [],
      opposition: gmTotal,
      tieSucceeds: false,
    }).verdict,
    "failure",
    "matching the Game Master's dice is not beating them",
  );
  assert.equal(
    resolve({
      faces: [4, 4],
      statuses: [],
      opposition: gmTotal,
      tieSucceeds: false,
    }).verdict,
    "success",
  );
});

/**
 * FR-014, as far as a pure module can state it: the opposition — however the
 * Game Master arrived at it, typed or rolled — is a single number, and the
 * advancement never sees it. `isAdvancement` takes the character's faces and
 * their bought dice, and there is no parameter a Game Master die could enter
 * by. The sheet's half of the claim is that `rollPool("BAND", …)` writes to
 * `gmDice` and `rollPool("LEVEL", …)` to `attempt.faces`, and nothing merges
 * them; the e2e counts the two sets of dice separately to prove it on screen.
 */
test("the opposition cannot reach the advancement, whatever it is", () => {
  const faces = [6, 6, 6];
  for (let opposition = 0; opposition <= 30; opposition += 1) {
    const outcome = resolve({
      faces,
      statuses: [],
      opposition,
      tieSucceeds: opposition % 2 === 0,
    });
    assert.deepEqual(outcome.sum, 18);
    assert.equal(
      isAdvancement(faces, 0),
      true,
      "three sixes is three sixes against any number",
    );
    assert.equal(
      outcome.verdict,
      opposition < 18 ? "success" : outcome.verdict,
    );
  }
});

test("the tie rule does not touch the advancement either", () => {
  // The only pool that can tie a 6 and show all sixes at once.
  const faces = [6];
  for (const tieSucceeds of [true, false]) {
    resolve({ faces, statuses: [], opposition: 6, tieSucceeds });
    assert.equal(
      isAdvancement(faces, 0),
      true,
      "advancement is read from the raw dice, which the tie rule never edits",
    );
  }
  assert.equal(
    resolve({ faces, statuses: [], opposition: 6, tieSucceeds: true }).verdict,
    "success",
  );
  assert.equal(
    resolve({ faces, statuses: [], opposition: 6, tieSucceeds: false }).verdict,
    "failure",
  );
});
