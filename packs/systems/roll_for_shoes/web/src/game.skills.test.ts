import { test } from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_SETTINGS,
  SLOT_CAPS,
  STARTING_SKILL,
  boughtSlotsAt,
  buySlot,
  capAtLevel,
  hasRoomAt,
  skillsOf,
  slotCost,
  startingSkills,
  slotsAvailable,
  slotsUsed,
  spendXp,
  type Skill,
  type WorldSettings,
} from "./game.ts";

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
