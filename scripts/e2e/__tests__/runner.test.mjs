/**
 * Unit tests for how `e2e-parallel.mjs` picks its specs and records a
 * slice's time (spec 060, T008 and T025).
 *
 * The selection tests run against a fixture file list, never the real tree:
 * they pin what `--only` *means*, which must not change because a spec was
 * added. The argument tests do start the runner, but only on paths that exit
 * 2 while parsing arguments — before the dependency check, the run lock or
 * any stack — so they are as safe beside a live e2e run as `--help` would be.
 */

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";

import { ROOT_DIR } from "../../shared.mjs";
import { filterOnly, onlyPatterns, sliceConflict } from "../select.mjs";
import {
  RECORD_FIELDS,
  SLICE_DURATIONS_FILE,
  measuredDate,
  readSliceDurations,
  recordSliceDuration,
} from "../slice-durations.mjs";
import { resolveSlice } from "../slices.mjs";

// The runner's spelling: relative to `apps/web`.
const SPECS = [
  "e2e/combat-panel.spec.ts",
  "e2e/engine-lighting-limits.spec.ts",
  "e2e/interactive-lighting.spec.ts",
  "e2e/lighting-editor.spec.ts",
  "e2e/lighting-fog.spec.ts",
  "e2e/scene-lighting.spec.ts",
  "e2e/torture/lighting-storm.spec.ts",
];

function slice(name, own, neighbours = []) {
  return {
    name,
    summary: `${name} (specs/000-${name})`,
    own,
    neighbours: neighbours.map((spec) => ({ spec, seam: `borrows ${spec}` })),
    paths: [],
  };
}

const SLICES = [
  slice("combat", ["combat-"]),
  slice("engine-limits", ["engine-lighting-limits"]),
  slice(
    "lighting",
    ["lighting-", "scene-lighting", "interactive-lighting"],
    ["combat-panel.spec.ts"],
  ),
  slice("torture", ["torture/"]),
];

describe("--only, unchanged (FR-020)", () => {
  test("no --only selects every file", () => {
    assert.deepEqual(filterOnly(SPECS, null), SPECS);
    assert.deepEqual(filterOnly(SPECS, undefined), SPECS);
  });

  test("a substring anywhere in the path, including engine-lighting-limits", () => {
    // R3's example: this is the looseness `--slice` exists to avoid, and
    // `--only` keeps it on purpose.
    assert.deepEqual(filterOnly(SPECS, "lighting"), [
      "e2e/engine-lighting-limits.spec.ts",
      "e2e/interactive-lighting.spec.ts",
      "e2e/lighting-editor.spec.ts",
      "e2e/lighting-fog.spec.ts",
      "e2e/scene-lighting.spec.ts",
      "e2e/torture/lighting-storm.spec.ts",
    ]);
  });

  test("a comma list is a union, trimmed, empties dropped", () => {
    assert.deepEqual(onlyPatterns(" combat- , ,fog"), ["combat-", "fog"]);
    assert.deepEqual(filterOnly(SPECS, " combat- , ,fog"), [
      "e2e/combat-panel.spec.ts",
      "e2e/lighting-fog.spec.ts",
    ]);
  });

  test("a pattern matching nothing selects nothing", () => {
    assert.deepEqual(filterOnly(SPECS, "nothing-by-this-name"), []);
    assert.deepEqual(filterOnly(SPECS, ","), []);
  });
});

describe("--slice, exact", () => {
  test("lighting does not pull in engine-lighting-limits", () => {
    assert.deepEqual(resolveSlice("lighting", SLICES, SPECS), [
      "e2e/combat-panel.spec.ts",
      "e2e/interactive-lighting.spec.ts",
      "e2e/lighting-editor.spec.ts",
      "e2e/lighting-fog.spec.ts",
      "e2e/scene-lighting.spec.ts",
    ]);
  });

  test("the files --only=lighting adds are exactly the ones other slices own", () => {
    const loose = filterOnly(SPECS, "lighting");
    const exact = resolveSlice("lighting", SLICES, SPECS);
    assert.deepEqual(
      loose.filter((file) => !exact.includes(file)),
      [
        "e2e/engine-lighting-limits.spec.ts",
        "e2e/torture/lighting-storm.spec.ts",
      ],
    );
  });

  test("conflicting flags are named", () => {
    const base = { slice: "combat", only: null, all: false, suite: "e2e" };
    assert.equal(sliceConflict(base), null);
    assert.equal(sliceConflict({ ...base, slice: null, all: true }), null);
    assert.match(sliceConflict({ ...base, only: "x" }), /--only/);
    assert.match(sliceConflict({ ...base, all: true }), /--all/);
    assert.match(sliceConflict({ ...base, suite: "playtest" }), /playtest/);
  });
});

describe("the runner refuses a bad --slice with exit 2", () => {
  const runner = (...args) =>
    spawnSync(process.execPath, ["scripts/e2e-parallel.mjs", ...args], {
      cwd: ROOT_DIR,
      encoding: "utf-8",
      timeout: 30_000,
    });

  test("an unknown name lists the valid ones", () => {
    const result = runner("--slice=no-such-slice");
    assert.equal(result.status, 2);
    assert.match(result.stderr, /unknown slice "no-such-slice"/);
    assert.match(result.stderr, /Valid slices: .*combat/);
  });

  test("--slice with --only", () => {
    const result = runner("--slice=combat", "--only=combat-");
    assert.equal(result.status, 2);
    assert.match(result.stderr, /--slice cannot be combined with --only/);
  });

  test("--slice with --all", () => {
    const result = runner("--all", "--slice=combat");
    assert.equal(result.status, 2);
    assert.match(result.stderr, /--slice cannot be combined with --all/);
  });
});

describe("slice-durations.json", () => {
  const record = (overrides = {}) => ({
    commit: "7928092",
    measuredAt: "2026-09-22",
    skipped: 0,
    flaky: 0,
    failed: 0,
    passed: 16,
    specs: 4,
    wallSeconds: 214,
    ...overrides,
  });

  function withRoot(body) {
    const root = mkdtempSync(join(tmpdir(), "slice-durations-"));
    // The runner writes into an existing `scripts/e2e`; so does this.
    mkdirSync(join(root, "scripts/e2e"), { recursive: true });
    try {
      body(root, join(root, SLICE_DURATIONS_FILE));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  }

  test("a missing file reads as nothing measured", () => {
    withRoot((root) => assert.deepEqual(readSliceDurations(root), {}));
  });

  test("the first record creates the file in the contract's shape", () => {
    withRoot((root, file) => {
      recordSliceDuration(file, "hero-builder", record());
      const text = readFileSync(file, "utf-8");
      assert.equal(
        text,
        `${JSON.stringify(
          {
            "hero-builder": {
              wallSeconds: 214,
              specs: 4,
              passed: 16,
              failed: 0,
              flaky: 0,
              skipped: 0,
              measuredAt: "2026-09-22",
              commit: "7928092",
            },
          },
          null,
          2,
        )}\n`,
      );
      assert.deepEqual(
        Object.keys(readSliceDurations(root)["hero-builder"]),
        RECORD_FIELDS,
      );
    });
  });

  test("slices are sorted, others kept, the same slice overwritten", () => {
    withRoot((root, file) => {
      recordSliceDuration(file, "tokens", record({ wallSeconds: 300 }));
      recordSliceDuration(file, "combat", record({ wallSeconds: 400 }));
      // A red run is recorded like any other.
      recordSliceDuration(
        file,
        "tokens",
        record({ wallSeconds: 310, failed: 2, commit: "abc1234-dirty" }),
      );
      const durations = readSliceDurations(root);
      assert.deepEqual(Object.keys(durations), ["combat", "tokens"]);
      assert.equal(durations.combat.wallSeconds, 400);
      assert.equal(durations.tokens.wallSeconds, 310);
      assert.equal(durations.tokens.failed, 2);
      assert.equal(durations.tokens.commit, "abc1234-dirty");
      assert.ok(readFileSync(file, "utf-8").endsWith("}\n"));
    });
  });

  test("a corrupt file is an error, not 'not measured'", () => {
    withRoot((root, file) => {
      writeFileSync(file, "{ not json");
      assert.throws(() => readSliceDurations(root), SyntaxError);
    });
  });

  test("the date is the local calendar day", () => {
    assert.equal(measuredDate(new Date(2026, 0, 5, 23, 59)), "2026-01-05");
  });
});
