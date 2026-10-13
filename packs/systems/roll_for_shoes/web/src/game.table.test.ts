import { test } from "node:test";
import assert from "node:assert/strict";

import {
  NO_TABLE_DIFFICULTY,
  addSkillByHand,
  advancementOwed,
  findSkills,
  grantSkillAmong,
  lineageOrder,
  oppositionFor,
  removeSkill,
  renameSkill,
  setSkillLevel,
  type Skill,
} from "./game.ts";
import { root } from "./gameTestFixtures.ts";

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
