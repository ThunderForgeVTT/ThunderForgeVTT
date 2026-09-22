/**
 * Unit tests for `scripts/check-e2e-slices.mjs` (spec 060, T021).
 *
 * One fixture per rule of research R7, each asserting the line the check
 * prints, because the line is the interface: it has to name the file and the
 * fix, and a rule that fires with the wrong words sends someone to the wrong
 * file. Everything is in memory, so these stay true however the real tree
 * and its slice list change.
 */

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import {
  canonicalScripts,
  checkSlices,
  fixPackageJson,
  parsePnpmCommand,
} from "../../check-e2e-slices.mjs";

/** A slice with every required key, so each test states only what it tests. */
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

const SPECS = [
  "e2e/combat-panel.spec.ts",
  "e2e/combat-turns.spec.ts",
  "e2e/status-display.spec.ts",
  "e2e/torture/lots-of-tokens.spec.ts",
];

const TRACKED = [
  "apps/web/src/features/combat/CombatPanel.tsx",
  "src/app/schema.graphql",
  "package.json",
];

const SLICES = [
  slice("combat", ["combat-"], {
    neighbours: [{ spec: "status-display.spec.ts", seam: "shows conditions" }],
    paths: ["apps/web/src/features/combat/**"],
  }),
  slice("status", ["status-display.spec.ts"]),
  slice("torture", ["torture/"]),
];

function scriptsFor(slices) {
  return Object.assign(
    { build: "./scripts/build.mjs" },
    ...slices.map(canonicalScripts),
  );
}

/**
 * A clean input, with `change` applied. Each test breaks exactly one thing,
 * so a failure names the rule rather than a pile-up of consequences.
 */
function run(change = {}) {
  const slices = change.slices ?? SLICES;
  return checkSlices({
    document: {
      crossCutting: [{ glob: "src/app/schema.graphql", why: "every client" }],
      slices,
      ...change.document,
    },
    specFiles: change.specFiles ?? SPECS,
    trackedFiles: change.trackedFiles ?? TRACKED,
    packageJson: change.packageJson ?? { scripts: scriptsFor(slices) },
    sliceDurations: change.sliceDurations ?? {},
    workspacePackages: change.workspacePackages ?? {},
  });
}

describe("checkSlices", () => {
  test("a true list has no problems", () => {
    assert.deepEqual(run(), []);
  });

  test("rule 1: a spec no slice owns is named, with the file to edit", () => {
    assert.deepEqual(run({ specFiles: [...SPECS, "e2e/new-thing.spec.ts"] }), [
      'apps/web/e2e/new-thing.spec.ts belongs to no slice — add it to "own" of a slice in scripts/e2e/slices.json',
    ]);
  });

  test("rule 2: an exact own name that no longer exists", () => {
    const slices = [
      slice("combat", ["combat-", "combat-old.spec.ts"]),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), [
      'slice "combat": "combat-old.spec.ts" in "own" does not exist — fix or remove it in scripts/e2e/slices.json',
    ]);
  });

  test("rule 2: a neighbour that no longer exists", () => {
    const slices = [
      slice("combat", ["combat-"], {
        neighbours: [{ spec: "combat-old.spec.ts", seam: "was renamed" }],
      }),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), [
      'slice "combat": neighbour "combat-old.spec.ts" does not exist',
    ]);
  });

  test("rule 2: a neighbour the slice owns itself is not a seam", () => {
    const slices = [
      slice("combat", ["combat-"], {
        neighbours: [{ spec: "combat-turns.spec.ts", seam: "its own spec" }],
      }),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), [
      'slice "combat": neighbour "combat-turns.spec.ts" is its own spec — remove it from "neighbours" in scripts/e2e/slices.json',
    ]);
  });

  test("rule 3: a prefix that matches no spec", () => {
    const slices = [
      slice("combat", ["combat-", "initiative-"]),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), [
      'slice "combat": prefix "initiative-" in "own" matches no spec — fix or remove it in scripts/e2e/slices.json',
    ]);
  });

  test("rule 4: a paths glob that matches no tracked file", () => {
    const slices = [
      slice("combat", ["combat-"], {
        paths: ["apps/web/src/features/fight/**"],
      }),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), [
      'slice "combat": path "apps/web/src/features/fight/**" matches no tracked file — fix or remove it in scripts/e2e/slices.json',
    ]);
  });

  test("rule 4: a cross-cutting glob that matches no tracked file", () => {
    assert.deepEqual(
      run({
        document: {
          crossCutting: [
            { glob: "src/server/migrations/**", why: "schema history" },
          ],
        },
      }),
      [
        'cross-cutting "src/server/migrations/**" matches no tracked file — fix or remove it in scripts/e2e/slices.json',
      ],
    );
  });

  test("rule 4: a glob matching a file deep under its directory is alive", () => {
    const slices = [
      slice("combat", ["combat-"], { paths: ["apps/web/src/**/*.tsx"] }),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), []);
  });

  test("rule 5: two slices owning a spec equally is a tie, naming both", () => {
    const slices = [
      slice("combat", ["combat-"]),
      slice("initiative", ["combat-t"]),
      slice("rounds", ["combat-t"]),
      ...SLICES.slice(1),
    ];
    assert.deepEqual(run({ slices }), [
      'scripts/e2e/slices.json: combat-turns.spec.ts is owned by both "initiative" ("combat-t") and "rounds" ("combat-t") — make one entry more specific',
    ]);
  });

  test("rule 6: a missing script, with its canonical body", () => {
    const scripts = scriptsFor(SLICES);
    delete scripts["e2e:status:integration"];
    assert.deepEqual(run({ packageJson: { scripts } }), [
      'package.json: "e2e:status:integration" is missing — it must be "node ./scripts/e2e-parallel.mjs --shards=1 --slice=status" (run with --fix)',
    ]);
  });

  test("rule 6: an extra script for a slice that does not exist", () => {
    const scripts = { ...scriptsFor(SLICES), "e2e:gone:integration": "x" };
    assert.deepEqual(run({ packageJson: { scripts } }), [
      'package.json: "e2e:gone:integration" belongs to no slice — add the slice to scripts/e2e/slices.json, or remove the script (run with --fix)',
    ]);
  });

  test("rule 6: a non-canonical body", () => {
    const scripts = {
      ...scriptsFor(SLICES),
      "e2e:combat:integration":
        "node ./scripts/e2e-parallel.mjs --only=combat-",
    };
    assert.deepEqual(run({ packageJson: { scripts } }), [
      'package.json: "e2e:combat:integration" must be "node ./scripts/e2e-parallel.mjs --shards=1 --slice=combat" (run with --fix)',
    ]);
  });

  test("rule 6: the listing and lookup scripts are not extras", () => {
    const scripts = {
      ...scriptsFor(SLICES),
      "e2e:slices": "node ./scripts/e2e-slice.mjs list",
      "e2e:which": "node ./scripts/e2e-slice.mjs which",
    };
    assert.deepEqual(run({ packageJson: { scripts } }), []);
  });

  test("rule 6: a slice with a standalone suite runs it first", () => {
    assert.deepEqual(
      canonicalScripts(
        slice("hero-builder", ["hero-builder-"], {
          standalone: "pnpm -F app test:e2e",
        }),
      ),
      {
        "e2e:hero-builder":
          "pnpm run e2e:hero-builder:standalone && pnpm run e2e:hero-builder:integration",
        "e2e:hero-builder:standalone": "pnpm -F app test:e2e",
        "e2e:hero-builder:integration":
          "node ./scripts/e2e-parallel.mjs --shards=1 --slice=hero-builder",
      },
    );
  });

  test("rule 7: a standalone naming a script its package lacks", () => {
    const slices = [
      ...SLICES.slice(0, 2),
      slice("torture", ["torture/"], {
        standalone: "pnpm -F @tf/app test:e2e",
      }),
    ];
    const workspacePackages = { "@tf/app": { scripts: { test: "vitest" } } };
    assert.deepEqual(run({ slices, workspacePackages }), [
      'slice "torture": standalone runs "test:e2e", which @tf/app has no script for — add the script or fix "standalone" in scripts/e2e/slices.json',
    ]);
    workspacePackages["@tf/app"].scripts["test:e2e"] = "playwright test";
    assert.deepEqual(run({ slices, workspacePackages }), []);
  });

  test("rule 7: a standalone naming a package outside the workspace", () => {
    const slices = [
      ...SLICES.slice(0, 2),
      slice("torture", ["torture/"], {
        standalone: "pnpm -F @tf/gone test:e2e",
      }),
    ];
    assert.deepEqual(run({ slices }), [
      'slice "torture": standalone names package "@tf/gone", which is not in the workspace — fix it in scripts/e2e/slices.json',
    ]);
  });

  test("rule 8: a name that is not kebab-case", () => {
    const slices = [slice("Combat", ["combat-"]), ...SLICES.slice(1)];
    assert.deepEqual(run({ slices }), [
      'scripts/e2e/slices.json: slice "Combat": the name must be kebab-case',
    ]);
  });

  test("rule 8: a name that collides with a reserved script", () => {
    const slices = [...SLICES, slice("which", ["status-"])];
    assert.deepEqual(run({ slices }), [
      'scripts/e2e/slices.json: slice "which": the name collides with the reserved script "e2e:which"',
    ]);
  });

  test("rule 9: a duration record for a slice that is gone", () => {
    const record = { wallSeconds: 60, specs: 1, passed: 1, failed: 0 };
    assert.deepEqual(
      run({ sliceDurations: { combat: record, gone: record } }),
      [
        'scripts/e2e/slice-durations.json: a record for "gone", which is no longer a slice — delete it',
      ],
    );
  });
});

describe("parsePnpmCommand", () => {
  test("reads the filter, with or without run", () => {
    assert.deepEqual(parsePnpmCommand("pnpm -F @tf/app test:e2e"), {
      package: "@tf/app",
      script: "test:e2e",
    });
    assert.deepEqual(parsePnpmCommand("pnpm --filter=@tf/app run test:e2e"), {
      package: "@tf/app",
      script: "test:e2e",
    });
    assert.deepEqual(parsePnpmCommand("pnpm run check"), {
      package: null,
      script: "check",
    });
  });

  test("anything it cannot read is null", () => {
    assert.equal(parsePnpmCommand("npx playwright test"), null);
    assert.equal(parsePnpmCommand("pnpm -F"), null);
  });
});

describe("fixPackageJson", () => {
  const manifest = (scripts) =>
    `${JSON.stringify({ name: "root", private: true, scripts, devDependencies: { x: "1" } }, null, 2)}\n`;

  test("rewrites slice scripts to canonical and nothing else", () => {
    const before = manifest({
      build: "./scripts/build.mjs",
      "e2e:combat": "pnpm run e2e:combat:integration",
      "e2e:combat:integration":
        "node ./scripts/e2e-parallel.mjs --only=combat-",
      "e2e:gone": "pnpm run e2e:gone:integration",
      "e2e:slices": "node ./scripts/e2e-slice.mjs list",
      journeys: "node ./scripts/journeys.mjs",
    });
    const after = fixPackageJson(before, SLICES);
    const parsed = JSON.parse(after);

    assert.deepEqual(Object.keys(parsed), [
      "name",
      "private",
      "scripts",
      "devDependencies",
    ]);
    assert.deepEqual(parsed.devDependencies, { x: "1" });
    assert.deepEqual(Object.keys(parsed.scripts), [
      "build",
      "e2e:combat",
      "e2e:combat:integration",
      "e2e:status",
      "e2e:status:integration",
      "e2e:torture",
      "e2e:torture:integration",
      "e2e:slices",
      "journeys",
    ]);
    // The unrelated scripts keep their bodies; the fix is canonical.
    assert.equal(parsed.scripts.build, "./scripts/build.mjs");
    assert.equal(
      parsed.scripts["e2e:slices"],
      "node ./scripts/e2e-slice.mjs list",
    );
    assert.deepEqual(
      run({ packageJson: parsed }),
      [],
      "the fixed package.json passes the check",
    );
    assert.ok(after.endsWith("}\n"), "the trailing newline is kept");
  });

  test("an already canonical file comes back byte for byte", () => {
    const text = manifest(scriptsFor(SLICES));
    assert.equal(fixPackageJson(text, SLICES), text);
  });

  test("a new slice's scripts go between the slices it sorts between", () => {
    const before = manifest(scriptsFor([SLICES[0], SLICES[2]]));
    const keys = Object.keys(
      JSON.parse(fixPackageJson(before, SLICES)).scripts,
    );
    assert.deepEqual(keys, [
      "build",
      "e2e:combat",
      "e2e:combat:integration",
      "e2e:status",
      "e2e:status:integration",
      "e2e:torture",
      "e2e:torture:integration",
    ]);
  });
});
