import { test } from "node:test";
import assert from "node:assert/strict";

import {
  grantSkill,
  isAdvancement,
  lineageOrder,
  remainingNonSixes,
  skillsOf,
  spendXp,
  verdict,
  xpAward,
  xpOf,
  type Skill,
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
