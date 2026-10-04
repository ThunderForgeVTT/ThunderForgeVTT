import { test } from "node:test";
import assert from "node:assert/strict";

import {
  BAND_DICE,
  BAND_TARGET,
  BANDS,
  DEFAULT_SETTINGS,
  NO_TABLE_DIFFICULTY,
  SLOT_CAPS,
  STARTING_SKILL,
  addSkillByHand,
  addStatus,
  advancementOwed,
  boughtSlotsAt,
  buySlot,
  capAtLevel,
  findSkills,
  hasRoomAt,
  grantSkill,
  grantSkillAmong,
  isAdvancement,
  lineageOrder,
  newStatusId,
  oppositionFor,
  remainingNonSixes,
  removeSkill,
  removeStatus,
  renameSkill,
  resolve,
  setSkillLevel,
  skillsOf,
  slotCost,
  startingSkills,
  slotsAvailable,
  slotsUsed,
  spendXp,
  statusModifier,
  statusesOf,
  verdict,
  xpAward,
  xpOf,
  type Skill,
  type Status,
  type WorldSettings,
} from "./game.ts";

const root: Skill = { id: "s1", name: "Do Anything", level: 1, parentId: null };

test("beating the opposition means beating it, and a tie does not", () => {
  assert.equal(verdict(7, 6), "success");
  assert.equal(verdict(6, 6), "failure");
  assert.equal(verdict(5, 6), "failure");
});

test("with no opposition named there is nothing to judge against", () => {
  assert.equal(verdict(4, null), "unjudged");
});

test("failure is the only thing that pays", () => {
  assert.equal(xpAward("failure"), 1);
  assert.equal(xpAward("success"), 0);
  assert.equal(xpAward("unjudged"), 0);
});

test("a tie pays, because a tie is a failure", () => {
  assert.equal(xpAward(verdict(6, 6)), 1);
});

test("advancement is read from the dice, not from their sum", () => {
  // Three dice totalling 18 without a single six.
  assert.equal(isAdvancement([5, 6, 6, 1], 0), false);
  assert.equal(isAdvancement([6, 6], 0), true);
  assert.equal(isAdvancement([], 0), false);
});

test("a bought six counts towards the advancement", () => {
  assert.equal(isAdvancement([6, 3], 0), false);
  assert.equal(isAdvancement([6, 3], 1), true);
});

test("experience buys a die and nothing else", () => {
  const spend = spendXp(1, [4], 0);
  assert.equal(spend.allowed, true);
  assert.equal(spend.allowed && spend.balance, 0);
  assert.equal(spend.allowed && spend.bought, 1);

  // The roll it was spent on is untouched: same faces, same total, same
  // verdict, same experience already awarded.
  const faces = [4];
  const total = faces.reduce((sum, face) => sum + face, 0);
  assert.equal(total, 4);
  assert.equal(verdict(total, 6), "failure");
  assert.equal(xpAward(verdict(total, 6)), 1);
  // ...and yet it now earns the new skill.
  assert.equal(isAdvancement(faces, 1), true);
});

test("a spend beyond the balance is refused with the reason", () => {
  const refused = spendXp(0, [4], 0);
  assert.equal(refused.allowed, false);
  assert.match(refused.allowed === false ? refused.reason : "", /balance is 0/);
});

test("there is nothing left to buy once every die shows a six", () => {
  assert.equal(remainingNonSixes([6, 6], 0), 0);
  const refused = spendXp(5, [6, 6], 0);
  assert.equal(refused.allowed, false);
});

test("a granted skill sits one level above the one it grew out of", () => {
  const granted = grantSkill(root, "Kick A Door Down", "s2");
  assert.equal(granted.granted, true);
  assert.deepEqual(granted.granted && granted.skill, {
    id: "s2",
    name: "Kick A Door Down",
    level: 2,
    parentId: "s1",
  });
});

test("an empty name creates nothing", () => {
  const refused = grantSkill(root, "   ", "s2");
  assert.equal(refused.granted, false);
});

test("any non-empty name is accepted — specificity is the table’s call", () => {
  const absurd = grantSkill(root, "Do Literally Anything At All", "s2");
  assert.equal(absurd.granted, true);
});

test("a character who has never been saved already holds the starting skill", () => {
  const skills = skillsOf(null);
  assert.equal(skills.length, 1);
  assert.equal(skills[0].name, "Do Anything");
  assert.equal(skills[0].level, 1);
  assert.equal(skills[0].parentId, null);
  assert.equal(xpOf(null), 0);
});

test("a stored lineage is read back as it was written", () => {
  const stored = {
    skills: [root, { id: "s2", name: "Run", level: 2, parentId: "s1" }],
  };
  assert.equal(skillsOf(stored).length, 2);
});

test("the lineage reads as the history it is", () => {
  const skills: Skill[] = [
    { id: "s3", name: "Kick A Cellar Door Down", level: 3, parentId: "s2" },
    root,
    { id: "s2", name: "Kick A Door Down", level: 2, parentId: "s1" },
  ];

  assert.deepEqual(
    lineageOrder(skills).map((row) => [row.skill.id, row.depth]),
    [
      ["s1", 0],
      ["s2", 1],
      ["s3", 2],
    ],
  );
});

test("a skill whose parent is missing is still shown", () => {
  const orphaned: Skill[] = [
    root,
    { id: "s9", name: "Lost", level: 4, parentId: "gone" },
  ];
  assert.equal(lineageOrder(orphaned).length, 2);
});

// ---------------------------------------------------------------------------
// Extras (spec 062)
//
// Every test below belongs to a setting that is off by default. The first one
// is the exception and the point: it is the claim that with all of them off,
// the game is still exactly the game spec 061 shipped.
// ---------------------------------------------------------------------------

/** Every dice pool of one to three d6. A pool is what a skill level buys. */
function everyPool(): number[][] {
  const pools: number[][] = [];
  for (const a of [1, 2, 3, 4, 5, 6]) {
    pools.push([a]);
    for (const b of [1, 2, 3, 4, 5, 6]) {
      pools.push([a, b]);
      for (const c of [1, 2, 3, 4, 5, 6]) {
        pools.push([a, b, c]);
      }
    }
  }
  return pools;
}

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

// ---------------------------------------------------------------------------
// Skill slots (US5)
// ---------------------------------------------------------------------------

/** A character holding `counts[level]` skills at each level. */
function holding(counts: Record<number, number>): Skill[] {
  const skills: Skill[] = [];
  for (const [level, count] of Object.entries(counts)) {
    for (let i = 0; i < count; i += 1) {
      skills.push({
        id: `s${level}-${i}`,
        name: `Skill ${level}.${i}`,
        level: Number(level),
        parentId: null,
      });
    }
  }
  return skills;
}

test("the caps are the three the rule names, and nothing else is capped", () => {
  assert.deepEqual({ ...SLOT_CAPS }, { 2: 4, 3: 3, 4: 2 });

  // Uncapped is null, never a large number — a caller that compares against a
  // number would eventually be wrong, and this is what stops it compiling that
  // way in the first place.
  assert.equal(capAtLevel(1), null);
  assert.equal(capAtLevel(5), null);
  assert.equal(capAtLevel(99), null);
  assert.equal(capAtLevel(2), 4);
  assert.equal(capAtLevel(3), 3);
  assert.equal(capAtLevel(4), 2);
});

test("a slot costs twice its level", () => {
  assert.equal(slotCost(2), 4);
  assert.equal(slotCost(3), 6);
  assert.equal(slotCost(4), 8);
});

test("room is counted per level, and an uncapped level always has some", () => {
  const held = holding({ 1: 1, 2: 4, 3: 1 });
  assert.equal(slotsUsed(held, 2), 4);
  assert.equal(slotsUsed(held, 3), 1);
  assert.equal(slotsUsed(held, 5), 0);

  assert.equal(slotsAvailable(held, 2, 0), 0);
  assert.equal(slotsAvailable(held, 3, 0), 2);
  assert.equal(slotsAvailable(held, 1, 0), null);
  assert.equal(slotsAvailable(held, 7, 0), null);

  assert.equal(hasRoomAt(held, 2, 0), false);
  assert.equal(hasRoomAt(held, 2, 1), true, "a bought slot is room");
  assert.equal(hasRoomAt(held, 1, 0), true);
  assert.equal(hasRoomAt(held, 9, 0), true);
});

/**
 * FR-036. A character made before this setting existed may already be over the
 * cap, and turning the setting on must not break them — so "available" goes
 * negative rather than throwing or clamping, and the character still reads.
 */
test("a character already over the cap has negative room, not an error", () => {
  const crowded = holding({ 1: 1, 2: 6 });
  assert.equal(slotsAvailable(crowded, 2, 0), -2);
  assert.equal(hasRoomAt(crowded, 2, 0), false);
  // Two bought slots still are not enough, and three are.
  assert.equal(hasRoomAt(crowded, 2, 2), false);
  assert.equal(hasRoomAt(crowded, 2, 3), true);
});

test("bought slots read as none when nothing is stored", () => {
  assert.equal(boughtSlotsAt(undefined, 2), 0);
  assert.equal(boughtSlotsAt({}, 2), 0);
  assert.equal(boughtSlotsAt({ boughtSlots: {} }, 2), 0);
  assert.equal(boughtSlotsAt({ boughtSlots: { "2": 3 } }, 2), 3);
  assert.equal(boughtSlotsAt({ boughtSlots: { "2": 3 } }, 3), 0);
  assert.equal(boughtSlotsAt({ boughtSlots: { "2": -1 } }, 2), 0);
  assert.equal(boughtSlotsAt({ boughtSlots: "three" }, 2), 0);
});

test("buying a slot at an uncapped level is refused as pointless", () => {
  for (const level of [1, 5, 12]) {
    const refused = buySlot(100, level, 0);
    assert.equal(refused.bought, false);
    assert.match(refused.bought === false ? refused.reason : "", /no limit/i);
  }
});

/**
 * FR-035. Experience buys dice-into-sixes and it buys slots, and there is only
 * one balance — which is how the ledger stays honest across both. This walks a
 * sequence through both kinds of spend and checks the arithmetic at every step,
 * including that neither ever takes the balance below zero.
 */
test("the XP ledger is honest across both kinds of spend", () => {
  let balance = 5;

  // A level-2 slot: 4 XP.
  const slot = buySlot(balance, 2, 0);
  assert.equal(slot.bought, true);
  balance = slot.bought === true ? slot.balance : balance;
  assert.equal(balance, 1);
  assert.equal(slot.bought === true ? slot.slots : 0, 1);

  // The 1 XP left buys exactly one die into a six.
  const six = spendXp(balance, [3, 4], 0);
  assert.equal(six.allowed, true);
  balance = six.allowed === true ? six.balance : balance;
  assert.equal(balance, 0);

  // And now neither will go through, rather than going negative.
  const noMoreDice = spendXp(balance, [3, 4], 1);
  assert.equal(noMoreDice.allowed, false);

  const noMoreSlots = buySlot(balance, 2, 1);
  assert.equal(noMoreSlots.bought, false);
  assert.match(noMoreSlots.bought === false ? noMoreSlots.reason : "", /XP/);

  assert.equal(balance, 0);
});

test("a slot is refused when the balance is short of its price, not rounded down", () => {
  // Three XP does not buy a four-XP slot, however close it is.
  const refused = buySlot(3, 2, 0);
  assert.equal(refused.bought, false);

  const exact = buySlot(4, 2, 0);
  assert.equal(exact.bought, true);
  assert.equal(exact.bought === true ? exact.balance : -1, 0);
});

// ---------------------------------------------------------------------------
// Starting skills (US6)
// ---------------------------------------------------------------------------

test("no declared starting skills means the core rule, not a character with none", () => {
  for (const settings of [undefined, DEFAULT_SETTINGS]) {
    const given = startingSkills(settings);
    assert.equal(given.length, 1);
    assert.equal(given[0]!.name, STARTING_SKILL.name);
    assert.equal(given[0]!.level, STARTING_SKILL.level);
    assert.equal(given[0]!.parentId, null);
    assert.equal(given[0]!.id, "starting-skill");
  }
});

test("a world may declare several starting skills, each a root", () => {
  const given = startingSkills({
    ...DEFAULT_SETTINGS,
    startingSkills: [
      { name: "Scavenge", level: 3 },
      { name: "Run Away", level: 1 },
    ],
  });
  assert.equal(given.length, 2);
  assert.deepEqual(
    given.map((skill) => [skill.name, skill.level, skill.parentId]),
    [
      ["Scavenge", 3, null],
      ["Run Away", 1, null],
    ],
  );
  // Ids are distinct, and the first keeps the one the core skill has always
  // had — one declared skill is the core case renamed, not a new shape.
  assert.equal(given[0]!.id, "starting-skill");
  assert.equal(new Set(given.map((skill) => skill.id)).size, 2);
});

test("a declared level below 1 is pulled up to 1, which is the lowest pool there is", () => {
  const given = startingSkills({
    ...DEFAULT_SETTINGS,
    startingSkills: [{ name: "Barely", level: 0 }],
  });
  assert.equal(given[0]!.level, 1);
});

/**
 * **FR-039, reduced to the mechanical rule it actually is.**
 *
 * Changing a world's starting skills must not alter a character who already
 * exists — and that holds not because anything compares dates, but because a
 * character with stored skills never reaches the branch that reads the
 * settings. This asserts exactly that, which is why it is cheap and why it
 * cannot drift.
 */
test("a character with stored skills never reads the world's starting skills", () => {
  const stored = {
    skills: [
      { id: "starting-skill", name: "Do Anything", level: 1, parentId: null },
      {
        id: "s2",
        name: "Kick A Door Down",
        level: 2,
        parentId: "starting-skill",
      },
    ],
  };
  const beforeTheChange = skillsOf(stored);

  const worldsToTry: WorldSettings[] = [
    { ...DEFAULT_SETTINGS, startingSkills: [{ name: "Scavenge", level: 3 }] },
    {
      ...DEFAULT_SETTINGS,
      startingSkills: [
        { name: "Sail", level: 2 },
        { name: "Swim", level: 4 },
      ],
    },
    DEFAULT_SETTINGS,
  ];
  for (const settings of worldsToTry) {
    assert.deepEqual(
      skillsOf(stored, settings),
      beforeTheChange,
      "a stored character moved when the world's starting skills changed",
    );
  }
});

test("skillsOf keeps its one-argument behaviour exactly", () => {
  // Spec 061 called this with one argument everywhere, and still does.
  assert.deepEqual(skillsOf(undefined), startingSkills());
  assert.deepEqual(skillsOf({}), startingSkills());
  assert.deepEqual(skillsOf({ skills: [] }), startingSkills());

  // An empty stored list is a character nobody has opened yet, so it takes the
  // world's starting skills rather than staying empty.
  assert.deepEqual(
    skillsOf(
      { skills: [] },
      {
        ...DEFAULT_SETTINGS,
        startingSkills: [{ name: "Scavenge", level: 3 }],
      },
    ).map((skill) => skill.name),
    ["Scavenge"],
  );
});

/* ------------------------------------------------------------------ *
 * The table: owed advancements, skills by hand, finding, difficulty
 * ------------------------------------------------------------------ */

/**
 * What the server's validator demands of a lineage, restated: every parent
 * named is on the sheet, every child is exactly one level above its parent,
 * levels are whole and 1 or more, names are not blank, and something stands
 * on its own. Every hand edit below is checked against it, because an edit
 * the server would refuse is an edit the Game Master cannot make.
 */
function assertTheServerWouldTakeIt(skills: Skill[]): void {
  assert.ok(skills.length > 0, "an empty lineage");
  assert.equal(new Set(skills.map((s) => s.id)).size, skills.length);
  assert.ok(
    skills.some((s) => s.parentId === null),
    "no root",
  );
  for (const skill of skills) {
    assert.ok(skill.name.trim().length > 0, "a blank name");
    assert.ok(Number.isInteger(skill.level) && skill.level >= 1, "a bad level");
    if (skill.parentId !== null) {
      const parent = skills.find((s) => s.id === skill.parentId);
      assert.ok(parent, `${skill.name} names a parent that is gone`);
      assert.equal(
        skill.level,
        parent.level + 1,
        `${skill.name} skips a level`,
      );
    }
  }
}

const climb: Skill = { id: "s2", name: "Climb", level: 2, parentId: "s1" };
const tower: Skill = {
  id: "s3",
  name: "Clock Tower",
  level: 3,
  parentId: "s2",
};
const spire: Skill = { id: "s4", name: "The Spire", level: 4, parentId: "s3" };
const family: Skill[] = [root, climb, tower, spire];

function changed(edit: ReturnType<typeof renameSkill>): Skill[] {
  assert.equal(edit.changed, true, edit.changed ? "" : edit.reason);
  if (!edit.changed) {
    throw new Error("unreachable");
  }
  assertTheServerWouldTakeIt(edit.skills);
  return edit.skills;
}

test("an advancement is owed until it is named or declined", () => {
  const sixes = { faces: [6, 6], bought: 0, advancementAnswered: false };
  assert.equal(advancementOwed(sixes), true);
  assert.equal(advancementOwed({ ...sixes, advancementAnswered: true }), false);
  // Bought into: owed from the moment the last die is bought.
  assert.equal(
    advancementOwed({ faces: [6, 2], bought: 1, advancementAnswered: false }),
    true,
  );
});

test("a roll that earned nothing owes nothing, so it never blocks the next", () => {
  assert.equal(advancementOwed(null), false);
  assert.equal(advancementOwed(undefined), false);
  assert.equal(
    advancementOwed({ faces: [6, 5], bought: 0, advancementAnswered: false }),
    false,
  );
  assert.equal(
    advancementOwed({ faces: [], bought: 0, advancementAnswered: false }),
    false,
  );
});

test("an advancement hangs beneath the skill rolled while that skill stands", () => {
  const granted = grantSkillAmong(family, climb, "Scale Ice", "n1");
  assert.deepEqual(granted, {
    granted: true,
    skill: { id: "n1", name: "Scale Ice", level: 3, parentId: "s2" },
  });
  assert.deepEqual(grantSkillAmong(family, climb, "  ", "n1"), {
    granted: false,
    reason: "Give the new skill a name.",
  });
});

test("an advancement survives the rolled skill being removed or re-levelled", () => {
  // Removed while the prompt was open: one above what was rolled, on its own.
  const without = changed(removeSkill(family, "s2"));
  const afterRemoval = grantSkillAmong(without, climb, "Scale Ice", "n1");
  assert.equal(afterRemoval.granted, true);
  if (afterRemoval.granted) {
    assert.deepEqual(afterRemoval.skill, {
      id: "n1",
      name: "Scale Ice",
      level: 3,
      parentId: null,
    });
    assertTheServerWouldTakeIt([...without, afterRemoval.skill]);
  }

  // Re-levelled while the prompt was open: still one above what was *rolled*.
  const moved = changed(setSkillLevel(family, "s2", 5));
  const afterMove = grantSkillAmong(moved, climb, "Scale Ice", "n1");
  assert.equal(afterMove.granted, true);
  if (afterMove.granted) {
    assert.equal(afterMove.skill.level, 3);
    assert.equal(afterMove.skill.parentId, null);
    assertTheServerWouldTakeIt([...moved, afterMove.skill]);
  }
});

test("renaming moves the name and nothing else", () => {
  const renamed = changed(renameSkill(family, "s2", "  Clamber "));
  assert.deepEqual(
    renamed.find((s) => s.id === "s2"),
    { ...climb, name: "Clamber" },
  );
  assert.deepEqual(
    renamed.filter((s) => s.id !== "s2"),
    family.filter((s) => s.id !== "s2"),
  );
  assert.equal(renameSkill(family, "s2", "   ").changed, false);
  assert.equal(renameSkill(family, "gone", "Anything").changed, false);
});

test("a skill added beneath a parent sits one level above it, whatever was asked", () => {
  const added = changed(addSkillByHand(family, "Rope Work", "s2", 9, "n1"));
  assert.deepEqual(added.at(-1), {
    id: "n1",
    name: "Rope Work",
    level: 3,
    parentId: "s2",
  });
  assert.equal(
    addSkillByHand(family, "Rope Work", "gone", 1, "n1").changed,
    false,
  );
});

test("a skill added on its own takes the level given", () => {
  const added = changed(addSkillByHand(family, "Haggle", null, 3, "n1"));
  assert.deepEqual(added.at(-1), {
    id: "n1",
    name: "Haggle",
    level: 3,
    parentId: null,
  });
  for (const bad of [0, -1, 1.5, Number.NaN]) {
    assert.equal(
      addSkillByHand(family, "Haggle", null, bad, "n1").changed,
      false,
    );
  }
  assert.equal(addSkillByHand(family, " ", null, 1, "n1").changed, false);
});

test("re-levelling carries the whole branch and cuts it loose from a parent it no longer fits", () => {
  const moved = changed(setSkillLevel(family, "s2", 4));
  const byId = new Map(moved.map((s) => [s.id, s]));
  assert.deepEqual(byId.get("s1"), root, "the parent is not touched");
  assert.deepEqual(byId.get("s2"), { ...climb, level: 4, parentId: null });
  assert.deepEqual(byId.get("s3"), { ...tower, level: 5 });
  assert.deepEqual(byId.get("s4"), { ...spire, level: 6 });
});

test("re-levelling a root moves its branch and leaves other roots alone", () => {
  const other: Skill = { id: "o1", name: "Haggle", level: 2, parentId: null };
  const moved = changed(setSkillLevel([...family, other], "s1", 2));
  assert.deepEqual(
    moved.map((s) => s.level),
    [2, 3, 4, 5, 2],
  );
  assert.equal(moved.find((s) => s.id === "s2")?.parentId, "s1");
});

test("re-levelling to the level a skill already has changes nothing", () => {
  assert.deepEqual(changed(setSkillLevel(family, "s3", 3)), family);
});

test("re-levelling downward is refused only below one, and only for the skill named", () => {
  assert.equal(setSkillLevel(family, "s2", 0).changed, false);
  assert.equal(setSkillLevel(family, "s2", 2.5).changed, false);
  assert.equal(setSkillLevel(family, "gone", 2).changed, false);
  // Down to 1: the branch follows and stays one apart.
  const lowered = changed(setSkillLevel(family, "s3", 1));
  assert.deepEqual(
    lowered.map((s) => [s.id, s.level, s.parentId]),
    [
      ["s1", 1, null],
      ["s2", 2, "s1"],
      ["s3", 1, null],
      ["s4", 2, "s3"],
    ],
  );
});

test("removing a skill keeps what grew out of it, standing on its own", () => {
  const removed = changed(removeSkill(family, "s2"));
  assert.deepEqual(
    removed.map((s) => [s.id, s.level, s.parentId]),
    [
      ["s1", 1, null],
      ["s3", 3, null],
      ["s4", 4, "s3"],
    ],
  );
});

test("removing the only root leaves its children as the roots", () => {
  const removed = changed(removeSkill(family, "s1"));
  assert.equal(removed.find((s) => s.id === "s2")?.parentId, null);
  assert.equal(removed.length, 3);
});

test("the last skill cannot be removed", () => {
  const refused = removeSkill([root], "s1");
  assert.equal(refused.changed, false);
  assert.equal(removeSkill(family, "gone").changed, false);
});

test("finding a skill narrows the lineage without reordering it", () => {
  const rows = lineageOrder([
    ...family,
    { id: "o1", name: "Climbing Gear", level: 2, parentId: "s1" },
  ]);
  assert.deepEqual(findSkills(rows, ""), rows);
  assert.deepEqual(findSkills(rows, "   "), rows);
  assert.deepEqual(
    findSkills(rows, "CLIMB").map((row) => [row.skill.name, row.depth]),
    [
      ["Climb", 1],
      ["Climbing Gear", 1],
    ],
  );
  assert.deepEqual(findSkills(rows, "no such thing"), []);
});

test("the Game Master's number is used, and what the player typed is not", () => {
  const set = { ...NO_TABLE_DIFFICULTY, target: 7 };
  assert.equal(oppositionFor(set, ""), 7);
  assert.equal(oppositionFor(set, "1"), 7);
  assert.equal(oppositionFor(set, "nonsense"), 7);
});

test("with no difficulty set the sheet's own entry is read as it always was", () => {
  assert.equal(oppositionFor(NO_TABLE_DIFFICULTY, ""), null);
  assert.equal(oppositionFor(NO_TABLE_DIFFICULTY, "  "), null);
  assert.equal(oppositionFor(NO_TABLE_DIFFICULTY, "5"), 5);
  assert.equal(oppositionFor(NO_TABLE_DIFFICULTY, "abc"), null);
});
