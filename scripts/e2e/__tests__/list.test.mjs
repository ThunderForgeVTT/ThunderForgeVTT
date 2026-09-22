/**
 * Unit tests for `scripts/e2e/list.mjs` (spec 060, T027 and T032).
 *
 * Against in-memory slices and times, never the recorded ones: what a slice
 * measured last week must not decide whether the state rules hold.
 */

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import {
  LIMIT_SECONDS,
  TARGET_SECONDS,
  buildRows,
  estimateSeconds,
  formatDetail,
  formatEstimate,
  formatMeasured,
  formatTable,
  lanesCell,
  sliceState,
  timeCell,
  totals,
} from "../list.mjs";

function slice(name, own, extra = {}) {
  return {
    name,
    summary: `${name} (specs/000-${name})`,
    own,
    neighbours: [],
    paths: [],
    ...extra,
  };
}

function record(wallSeconds, failed = 0) {
  return {
    wallSeconds,
    measuredAt: "2026-09-22",
    commit: "abc1234",
    passed: 3,
    failed,
    flaky: 0,
    skipped: 0,
    specs: 3,
  };
}

const SPECS = [
  "e2e/combat-attack.spec.ts",
  "e2e/combat-turn-order.spec.ts",
  "e2e/hero-builder-npc.spec.ts",
  "e2e/instance-setup.spec.ts",
  "e2e/status-display.spec.ts",
];

const SLICES = [
  slice("combat", ["combat-"]),
  slice("hero-builder", ["hero-builder-"], {
    standalone: "pnpm -F @thunderforge/hero-builder-app test:e2e",
  }),
  slice("instance", ["instance-"]),
  slice("status", ["status-"], {
    neighbours: [{ spec: "combat-attack.spec.ts", seam: "a hit changes HP" }],
  }),
];

describe("sliceState", () => {
  test("no record is not measured", () => {
    assert.equal(sliceState(undefined), "not measured");
  });

  test("the target and the limit are both inclusive", () => {
    assert.equal(sliceState(record(TARGET_SECONDS)), "measured");
    assert.equal(sliceState(record(TARGET_SECONDS + 1)), "over target");
    assert.equal(sliceState(record(LIMIT_SECONDS)), "over target");
    assert.equal(sliceState(record(LIMIT_SECONDS + 1)), "over limit");
  });

  test("red wins over any time", () => {
    assert.equal(sliceState(record(30, 1)), "red");
    assert.equal(sliceState(record(LIMIT_SECONDS + 100, 2)), "red");
  });
});

describe("estimates", () => {
  test("sums what is recorded and names what is not", () => {
    const estimate = estimateSeconds(SPECS.slice(0, 2), {
      "e2e/combat-attack.spec.ts": 90,
    });
    assert.deepEqual(estimate, {
      seconds: 90,
      missing: ["e2e/combat-turn-order.spec.ts"],
    });
  });

  test("an estimate always says est., and * when it has gaps", () => {
    assert.equal(formatEstimate({ seconds: 378, missing: [] }), "est. 6.3m");
    assert.equal(formatEstimate({ seconds: 60, missing: ["x"] }), "est. 1.0m*");
    assert.equal(formatMeasured(400), "6m 40s");
  });
});

describe("rows and the table", () => {
  const durations = {
    "e2e/combat-attack.spec.ts": 100,
    "e2e/combat-turn-order.spec.ts": 50,
    "e2e/status-display.spec.ts": 40,
  };
  const measurements = { combat: record(650), status: record(90, 1) };
  const rows = buildRows(SLICES, { specFiles: SPECS, measurements, durations });
  const byName = Object.fromEntries(rows.map((row) => [row.name, row]));

  test("a neighbour counts toward specs but not toward own", () => {
    assert.equal(byName.status.specs, 2);
    assert.equal(byName.status.own, 1);
    assert.equal(byName.status.neighbours, 1);
  });

  test("measured time replaces the estimate; unmeasured shows est.", () => {
    assert.equal(byName.combat.state, "over target");
    assert.equal(timeCell(byName.combat), "10m 50s");
    assert.equal(byName["hero-builder"].state, "not measured");
    assert.match(timeCell(byName["hero-builder"]), /^est\. /);
    assert.equal(byName.status.state, "red");
  });

  test("lanes follow the runner's partition", () => {
    assert.deepEqual(byName.instance.lanes, ["first-run"]);
    assert.deepEqual(byName.combat.lanes, ["default"]);
  });

  test("a slice with a standalone half is marked in the table", () => {
    assert.equal(lanesCell(byName["hero-builder"]), "default +standalone");
    assert.equal(lanesCell(byName.combat), "default");
    const table = formatTable(rows, totals(rows, SPECS.length));
    assert.match(table, /hero-builder.*default \+standalone/);
    assert.match(
      table,
      /4 slices, 5 specs \(6 runs counting neighbours\); 2 measured, 1 over target, 0 over limit, 1 red/,
    );
    assert.match(table, /^est\. = /m);
  });

  test("the detail shows the standalone command, and only when there is one", () => {
    const options = { slices: SLICES, specFiles: SPECS };
    const hero = formatDetail(SLICES[1], byName["hero-builder"], options);
    assert.match(
      hero,
      /^Standalone: pnpm -F @thunderforge\/hero-builder-app test:e2e$/m,
    );
    const combat = formatDetail(SLICES[0], byName.combat, options);
    assert.doesNotMatch(combat, /Standalone:/);
    assert.match(combat, /combat-attack\.spec\.ts\s+prefix "combat-"/);
    assert.match(combat, /Time: 10m 50s \(over target\)/);
  });

  test("the detail names the seam of each neighbour", () => {
    const status = formatDetail(SLICES[3], byName.status, {
      slices: SLICES,
      specFiles: SPECS,
    });
    assert.match(status, /combat-attack\.spec\.ts — a hit changes HP/);
    assert.match(status, /^Run: pnpm e2e:status$/m);
  });
});
