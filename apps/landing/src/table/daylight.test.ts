import assert from "node:assert/strict";
import { test } from "node:test";
import { darknessAt, eveningFor, NIGHT_DARK } from "./daylight.ts";

// Run with `node --test src/table/daylight.test.ts` (Node strips the types).

test("the curve turns where the engine's does", () => {
  assert.equal(darknessAt(12), 0);
  assert.equal(darknessAt(18), 0);
  assert.ok(Math.abs(darknessAt(20) - 0.6) < 1e-9);
  assert.equal(darknessAt(0), NIGHT_DARK);
  assert.equal(darknessAt(24 + 12), 0);
});

test("the evening hour brings the dusk asked for", () => {
  for (const d of [0.05, 0.3, 0.6, 0.9, 0.95]) {
    assert.ok(Math.abs(darknessAt(eveningFor(d)) - d) < 1e-4, `${d}`);
  }
  assert.equal(eveningFor(0), 18);
  assert.equal(eveningFor(1), 24);
});
