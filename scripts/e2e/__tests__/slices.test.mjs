/**
 * Unit tests for `scripts/e2e/slices.mjs` (spec 060, T005).
 *
 * Against in-memory fixtures, never the real tree: these prove the rules
 * themselves, so they must not start passing or failing because someone
 * added a spec. Whether the real list obeys the rules is the coverage
 * check's job.
 */

import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, test } from "node:test";

import {
  SliceError,
  loadSlices,
  matchPath,
  ownerOf,
  ownershipOf,
  resolveSlice,
  sliceLanes,
  validateSlices,
} from "../slices.mjs";

/** A slice with every required key, so each test states only what it tests. */
function slice(name, own, neighbours = [], extra = {}) {
  return {
    name,
    summary: `${name} (specs/000-${name})`,
    own,
    neighbours: neighbours.map((spec) => ({ spec, seam: `borrows ${spec}` })),
    paths: [],
    ...extra,
  };
}

// The runner's spelling: relative to `apps/web`.
const SPECS = [
  "e2e/combat-attack.spec.ts",
  "e2e/combat-hit-points.spec.ts",
  "e2e/interactive-doors.spec.ts",
  "e2e/interactive-lighting.spec.ts",
  "e2e/scene-lighting.spec.ts",
  "e2e/scene-management.spec.ts",
  "e2e/status-display.spec.ts",
  "e2e/torture/lots-of-tokens.spec.ts",
  "e2e/torture/many-scenes.spec.ts",
];

describe("ownerOf", () => {
  test("an exact name beats a prefix", () => {
    const slices = [
      slice("lighting", ["scene-lighting.spec.ts"]),
      slice("scenes", ["scene-"]),
    ];
    assert.equal(ownerOf("e2e/scene-lighting.spec.ts", slices), "lighting");
    assert.equal(ownerOf("e2e/scene-management.spec.ts", slices), "scenes");
    assert.deepEqual(ownershipOf("e2e/scene-lighting.spec.ts", slices), {
      slice: "lighting",
      entry: "scene-lighting.spec.ts",
      exact: true,
    });
  });

  test("a longer prefix beats a shorter one", () => {
    const slices = [
      slice("interactive", ["interactive-"]),
      slice("lighting", ["interactive-light"]),
    ];
    assert.equal(
      ownerOf("e2e/interactive-lighting.spec.ts", slices),
      "lighting",
    );
    assert.equal(
      ownerOf("e2e/interactive-doors.spec.ts", slices),
      "interactive",
    );
  });

  test("a tie between two slices throws, naming both", () => {
    const slices = [slice("alpha", ["combat-"]), slice("beta", ["combat-"])];
    assert.throws(
      () => ownerOf("e2e/combat-attack.spec.ts", slices),
      (error) =>
        error instanceof SliceError &&
        error.message.includes('"alpha"') &&
        error.message.includes('"beta"'),
    );
  });

  test("a tie below the winning entry is not a tie", () => {
    const slices = [
      slice("alpha", ["scene-"]),
      slice("beta", ["scene-"]),
      slice("lighting", ["scene-lighting.spec.ts"]),
    ];
    assert.equal(ownerOf("e2e/scene-lighting.spec.ts", slices), "lighting");
  });

  test("a prefix selects torture/ files", () => {
    const slices = [slice("torture", ["torture/"])];
    assert.equal(ownerOf("e2e/torture/many-scenes.spec.ts", slices), "torture");
    assert.equal(ownerOf("e2e/scene-management.spec.ts", slices), null);
  });

  test("every spelling of a path gets the same owner", () => {
    const slices = [slice("combat", ["combat-"])];
    for (const spelling of [
      "combat-attack.spec.ts",
      "e2e/combat-attack.spec.ts",
      "apps/web/e2e/combat-attack.spec.ts",
    ]) {
      assert.equal(ownerOf(spelling, slices), "combat");
    }
  });
});

describe("resolveSlice", () => {
  const slices = [
    slice("combat", ["combat-"], ["status-display.spec.ts"]),
    slice("status", ["status-"], ["combat-hit-points.spec.ts"]),
    slice("torture", ["torture/"]),
  ];

  test("returns owned and neighbour specs, sorted, in the runner's spelling", () => {
    assert.deepEqual(resolveSlice("combat", slices, SPECS), [
      "e2e/combat-attack.spec.ts",
      "e2e/combat-hit-points.spec.ts",
      "e2e/status-display.spec.ts",
    ]);
    assert.deepEqual(resolveSlice("torture", slices, SPECS), [
      "e2e/torture/lots-of-tokens.spec.ts",
      "e2e/torture/many-scenes.spec.ts",
    ]);
  });

  test("deduplicates a neighbour that is also owned", () => {
    const overlapping = [
      slice("combat", ["combat-"], ["combat-attack.spec.ts"]),
    ];
    assert.deepEqual(resolveSlice("combat", overlapping, SPECS), [
      "e2e/combat-attack.spec.ts",
      "e2e/combat-hit-points.spec.ts",
    ]);
  });

  test("an unknown slice throws and lists the valid names", () => {
    assert.throws(
      () => resolveSlice("combt", slices, SPECS),
      (error) =>
        error instanceof SliceError &&
        error.message.includes('"combt"') &&
        error.message.includes("combat, status, torture"),
    );
  });

  test("a neighbour that does not exist throws", () => {
    const broken = [slice("combat", ["combat-"], ["combat-old.spec.ts"])];
    assert.throws(
      () => resolveSlice("combat", broken, SPECS),
      /neighbour "combat-old.spec.ts" does not exist/,
    );
  });
});

describe("validateSlices", () => {
  const valid = () => ({
    $comment: "fixture",
    crossCutting: [{ glob: "src/app/schema.graphql", why: "every client" }],
    slices: [slice("alpha", ["alpha-"]), slice("beta", ["beta-"])],
  });

  test("accepts a well-formed list", () => {
    const { slices, crossCutting } = validateSlices(valid());
    assert.equal(slices.length, 2);
    assert.equal(crossCutting.length, 1);
  });

  test("rejects an unknown key on a slice, such as `neighbors`", () => {
    const document = valid();
    document.slices[0].neighbors = [];
    assert.throws(() => validateSlices(document), /unknown key "neighbors"/);
  });

  test("rejects an unknown key on a seam and at the top level", () => {
    const seam = valid();
    seam.slices[0].neighbours = [{ spec: "x.spec.ts", seem: "typo" }];
    assert.throws(() => validateSlices(seam), SliceError);
    const top = valid();
    top.slice = [];
    assert.throws(() => validateSlices(top), /unknown key "slice"/);
  });

  test("rejects an unsorted list", () => {
    const document = valid();
    document.slices.reverse();
    assert.throws(() => validateSlices(document), /sorted by name/);
  });

  test("rejects a duplicate, non-kebab or reserved name", () => {
    const duplicate = valid();
    duplicate.slices[1].name = "alpha";
    assert.throws(() => validateSlices(duplicate), /used twice/);
    const camel = valid();
    camel.slices[0].name = "Alpha";
    assert.throws(() => validateSlices(camel), /kebab-case/);
    const reserved = valid();
    reserved.slices = [slice("which", ["which-"])];
    assert.throws(() => validateSlices(reserved), /reserved/);
  });

  test("rejects a neighbour given as a prefix", () => {
    const document = valid();
    document.slices[0].neighbours = [{ spec: "beta-", seam: "prefix" }];
    assert.throws(() => validateSlices(document), /exact spec name/);
  });
});

describe("loadSlices", () => {
  test("reads scripts/e2e/slices.json under the given root", () => {
    const root = mkdtempSync(join(tmpdir(), "slices-"));
    try {
      mkdirSync(join(root, "scripts/e2e"), { recursive: true });
      const file = join(root, "scripts/e2e/slices.json");
      writeFileSync(
        file,
        JSON.stringify({ slices: [slice("alpha", ["alpha-"])] }),
      );
      assert.equal(loadSlices(root).slices[0].name, "alpha");
      writeFileSync(file, "{ not json");
      assert.throws(() => loadSlices(root), /not valid JSON/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});

describe("sliceLanes", () => {
  test("uses the runner's lane predicates", () => {
    assert.deepEqual(sliceLanes(["e2e/combat-attack.spec.ts"]), ["default"]);
    assert.deepEqual(
      sliceLanes([
        "e2e/instance-setup.spec.ts",
        "e2e/engine-limits.spec.ts",
        "e2e/github-apps.spec.ts",
        "e2e/combat-attack.spec.ts",
      ]),
      ["default", "measured", "first-run", "github-apps"],
    );
  });
});

describe("matchPath", () => {
  test("returns the first matching glob, or null", () => {
    const globs = [
      "apps/web/src/features/combat/**",
      "src/server/src/combat/**",
    ];
    assert.equal(
      matchPath("src/server/src/combat/attack/mod.rs", globs),
      "src/server/src/combat/**",
    );
    assert.equal(matchPath("src/server/src/auth/mod.rs", globs), null);
  });
});
