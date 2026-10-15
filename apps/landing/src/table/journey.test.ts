import assert from "node:assert/strict";
import { test } from "node:test";
import { dawning, duskAt, EVENING, NIGHT } from "./journey.ts";

// Run with `node --test src/table/journey.test.ts` (Node strips the types).

/** The dark at every 100 px from the top to `to`. */
const steps = (to: number, at: (y: number) => number) => {
  const out: number[] = [];
  for (let y = 0; y <= to; y += 100) out.push(at(y));
  return out;
};

for (const vh of [700, 900, 1080, 1440, 2160]) {
  test(`no 100 px step changes the dark by more than 0.08 at ${vh} px tall (SC-006)`, () => {
    const dark = steps(12000, (y) => duskAt(y, 600, 4000, 9000, vh));
    for (let i = 1; i < dark.length; i++) {
      const step = Math.abs(dark[i]! - dark[i - 1]!);
      assert.ok(step <= 0.08, `${(i - 1) * 100}->${i * 100}: ${step.toFixed(3)}`);
    }
  });
}

test("day holds to its end, and it is evening, then night, by the room", () => {
  assert.equal(duskAt(600, 600, 6000, 20000, 900), 0);
  assert.ok(Math.abs(duskAt(3000, 600, 6000, 20000, 900) - EVENING) < 0.01);
  assert.equal(duskAt(6000, 600, 6000, 20000, 900), NIGHT);
});

test("dawn breaks over at least as much page as the evening falls", () => {
  assert.equal(dawning(9000, 9000, 900), 1);
  assert.ok(dawning(9000 - 1000, 9000, 900) > 0.4);
  assert.equal(dawning(9000 - 2100, 9000, 900), 0);
});
