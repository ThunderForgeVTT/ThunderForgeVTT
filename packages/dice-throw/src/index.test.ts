import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { test } from "node:test";
import { initSync, roll } from "@thunderforge/dice";
import { diceCommand, type Resolution } from "./index.ts";

// The real crate, so these shapes are what it writes and not a copy of them.
const wasm = createRequire(import.meta.url).resolve("@thunderforge/dice/dice_bg.wasm");
initSync({ module: readFileSync(wasm) });

const SEED = new Uint32Array([7, 11, 13, 17]);
const resolve = (formula: string) => JSON.parse(roll(formula, "{}", SEED)) as Resolution;

interface Thrown {
  id: string;
  rollerName: string;
  formula: string;
  resultKind: string;
  resultValue: number;
  dice: Array<{
    sidesKind: string;
    numericSides: number | null;
    rolls: number[];
    steps: string[];
    kept: boolean;
    finalValue: number;
  }>;
}
const thrown = (formula: string) => {
  const detail = resolve(formula);
  const command = diceCommand(detail, { rollerName: "Front page", id: "r1" });
  assert.equal(command.type, "trigger_dice_roll");
  return { detail, roll: command.roll as Thrown };
};

test("a numeric roll carries every die's faces and the total, unchanged", () => {
  const { detail, roll } = thrown("2d6 + 3");
  assert.equal(roll.id, "r1");
  assert.equal(roll.rollerName, "Front page");
  assert.equal(roll.formula, detail.formula);
  assert.equal(roll.resultKind, "TOTAL");
  assert.equal(roll.resultValue, (detail.kind as { Total: number }).Total);
  assert.equal(roll.dice.length, 2);
  for (const [i, die] of roll.dice.entries()) {
    assert.equal(die.sidesKind, "NUMERIC");
    assert.equal(die.numericSides, 6);
    assert.deepEqual(die.rolls, detail.dice[i]!.rolls);
    assert.equal(die.finalValue, detail.dice[i]!.final_value);
    assert.ok(die.finalValue >= 1 && die.finalValue <= 6);
  }
  assert.equal(roll.resultValue, roll.dice[0]!.finalValue + roll.dice[1]!.finalValue + 3);
});

test("a d100 is one hundred-sided die", () => {
  const { roll } = thrown("1d100");
  assert.equal(roll.dice.length, 1);
  assert.equal(roll.dice[0]!.numericSides, 100);
  assert.equal(roll.resultValue, roll.dice[0]!.finalValue);
});

test("Fate dice have no sides number and faces of -1, 0 or 1", () => {
  const { roll } = thrown("4dF");
  assert.equal(roll.dice.length, 4);
  for (const die of roll.dice) {
    assert.equal(die.sidesKind, "FATE");
    assert.equal(die.numericSides, null);
    assert.ok([-1, 0, 1].includes(die.finalValue));
  }
});

test("kept and dropped dice say which they are, and only the kept count", () => {
  const { roll } = thrown("4d6dl1");
  assert.equal(roll.dice.filter((d) => d.kept).length, 3);
  const dropped = roll.dice.find((d) => !d.kept)!;
  assert.ok(roll.dice.every((d) => !d.kept || d.finalValue >= dropped.finalValue));
  assert.equal(
    roll.resultValue,
    roll.dice.filter((d) => d.kept).reduce((n, d) => n + d.finalValue, 0),
  );
});

test("a success-count roll is sent as one", () => {
  const { detail, roll } = thrown("6d10cs>=8");
  assert.equal(roll.resultKind, "SUCCESS_COUNT");
  assert.equal(roll.resultValue, (detail.kind as { SuccessCount: number }).SuccessCount);
  assert.equal(roll.resultValue, roll.dice.filter((d) => d.finalValue >= 8).length);
});

test("exploding dice carry their steps", () => {
  // A seed is found that explodes, so the step is really there. Seeds are
  // spread across all 32 bits: the generator's first draw barely moves for
  // small words, and every die would land on the same face.
  for (let s = 1; s < 500; s++) {
    const word = Math.imul(s, 0x9e3779b1) >>> 0;
    const detail = JSON.parse(
      roll("1d6x", "{}", new Uint32Array([word, word ^ 0xdeadbeef, ~word >>> 0, word >>> 3])),
    ) as Resolution;
    if ((detail.dice[0]!.steps ?? []).length === 0) continue;
    const die = (diceCommand(detail, { rollerName: "x" }).roll as Thrown).dice[0]!;
    assert.ok(die.steps.length > 0);
    assert.ok(die.steps.every((st) => st === "EXPLODE"));
    assert.equal(die.rolls.length, die.steps.length + 1);
    return;
  }
  assert.fail("no seed in 500 exploded a d6");
});

test("each throw gets its own id when none is given", () => {
  const detail = resolve("1d20");
  const a = diceCommand(detail, { rollerName: "x" }).roll as Thrown;
  const b = diceCommand(detail, { rollerName: "x" }).roll as Thrown;
  assert.notEqual(a.id, b.id);
});
