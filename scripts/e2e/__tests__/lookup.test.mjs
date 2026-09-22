/**
 * Unit tests for `scripts/e2e/lookup.mjs` (spec 060, T016).
 *
 * Against an in-memory list, never `slices.json`: these prove the order the
 * lookup asks its questions in (research R5), so they must not change answer
 * when someone gives a file to a slice. Whether the real list leaves anything
 * uncovered is `which --json` over `git ls-files`, not a unit test.
 */

import assert from "node:assert/strict";
import { describe, test } from "node:test";

import {
  FULL_SUITE_COMMAND,
  exitCodeFor,
  formatLookup,
  isSpecPath,
  lookupPath,
  lookupPaths,
  normalisePath,
} from "../lookup.mjs";

/** A slice with every required key, so each test states only what it tests. */
function slice(name, own, { neighbours = [], paths = [] } = {}) {
  return {
    name,
    summary: `${name} (specs/000-${name})`,
    own,
    neighbours: neighbours.map((spec) => ({ spec, seam: `borrows ${spec}` })),
    paths,
  };
}

const LIST = {
  crossCutting: [
    { glob: "src/app/schema.graphql", why: "every client speaks this schema" },
    // Deliberately overlaps the status slice's glob below: the precedence
    // test needs a path both could claim.
    { glob: "apps/web/src/components/ui/**", why: "shared UI primitives" },
  ],
  slices: [
    slice("combat", ["combat-"], {
      neighbours: ["status-display.spec.ts"],
      paths: ["src/server/src/combat/**"],
    }),
    slice("status", ["status-"], {
      paths: [
        "apps/web/src/components/StatusPanel/**",
        "apps/web/src/components/**",
        "src/server/src/status_display.rs",
      ],
    }),
    slice("tokens", ["token-"], {
      neighbours: ["status-display.spec.ts"],
      paths: ["src/server/src/status_display.rs"],
    }),
  ],
};

describe("isSpecPath and normalisePath", () => {
  test("only apps/web/e2e specs outside journeys are specs", () => {
    assert.equal(isSpecPath("apps/web/e2e/status-display.spec.ts"), true);
    assert.equal(isSpecPath("apps/web/e2e/torture/x.torture.spec.ts"), true);
    assert.equal(isSpecPath("apps/web/e2e/journeys/a.journey.spec.ts"), false);
    assert.equal(isSpecPath("apps/web/e2e/fixtures/auth.ts"), false);
    assert.equal(isSpecPath("apps/web/src/x.spec.ts"), false);
  });

  test("a typed ./ or backslash path reads as the repository path", () => {
    assert.equal(normalisePath("./docs/a.md"), "docs/a.md");
    assert.equal(normalisePath("docs\\a.md"), "docs/a.md");
  });
});

describe("lookupPath", () => {
  test("cross-cutting answers the full suite, with its reason", () => {
    const answer = lookupPath("src/app/schema.graphql", LIST);
    assert.equal(answer.kind, "crossCutting");
    assert.equal(answer.why, "every client speaks this schema");
    assert.deepEqual(answer.slices, []);
  });

  test("cross-cutting beats a slice glob that also matches", () => {
    const path = "apps/web/src/components/ui/button.tsx";
    assert.equal(lookupPath(path, LIST).kind, "crossCutting");
  });

  test("a spec names its owner, then every borrower", () => {
    const answer = lookupPath("apps/web/e2e/status-display.spec.ts", LIST);
    assert.equal(answer.kind, "spec");
    assert.equal(answer.owner, "status");
    assert.deepEqual(answer.borrowers, ["combat", "tokens"]);
    assert.deepEqual(answer.slices, ["status", "combat", "tokens"]);
  });

  test("a spec no slice owns is uncovered, but its borrowers are kept", () => {
    const list = {
      crossCutting: [],
      slices: [
        slice("combat", ["combat-"], { neighbours: ["orphan.spec.ts"] }),
      ],
    };
    const answer = lookupPath("apps/web/e2e/orphan.spec.ts", list);
    assert.equal(answer.kind, "uncovered");
    assert.deepEqual(answer.slices, ["combat"]);
    assert.match(answer.why, /belongs to no slice/);
  });

  test("a path glob names every slice that claims it", () => {
    const answer = lookupPath("src/server/src/status_display.rs", LIST);
    assert.equal(answer.kind, "slice");
    assert.deepEqual(answer.slices, ["status", "tokens"]);
  });

  test("a slice's paths are asked before the no-e2e rule", () => {
    const list = {
      crossCutting: [],
      slices: [slice("lore", ["lore-"], { paths: ["packs/lore/**"] })],
    };
    assert.equal(lookupPath("packs/lore/README.md", list).kind, "slice");
  });

  test("documentation needs no e2e", () => {
    for (const path of [
      "docs/CONTRIBUTING.md",
      "specs/060-a-slice-for-every-feature/tasks.md",
      "src/server/README.md",
      ".specify/templates/plan-template.md",
      "marketing/perf.json",
    ]) {
      assert.equal(lookupPath(path, LIST).kind, "noE2e", path);
    }
  });

  test("anything else is uncovered", () => {
    const answer = lookupPath("scripts/dev.mjs", LIST);
    assert.equal(answer.kind, "uncovered");
    assert.deepEqual(answer.slices, []);
  });
});

describe("lookupPaths", () => {
  test("the run is each slice once, in name order", () => {
    const result = lookupPaths(
      [
        "src/server/src/status_display.rs",
        "apps/web/e2e/status-display.spec.ts",
        "src/server/src/combat/turn.rs",
        "./src/server/src/combat/turn.rs",
      ],
      LIST,
    );
    assert.equal(result.paths.length, 3, "a repeated path is asked once");
    assert.deepEqual(result.run, ["combat", "status", "tokens"]);
    assert.equal(result.fullSuite, false);
    assert.deepEqual(result.uncovered, []);
    assert.equal(exitCodeFor(result), 0);
  });

  test("one cross-cutting path asks for the full suite, slices still listed", () => {
    const result = lookupPaths(
      ["src/app/schema.graphql", "src/server/src/combat/turn.rs"],
      LIST,
    );
    assert.equal(result.fullSuite, true);
    assert.deepEqual(result.run, ["combat"]);
    const text = formatLookup(result);
    assert.match(text, /Run: pnpm run e2e:combat/);
    assert.ok(text.includes(`But: a cross-cutting path changed`), text);
    assert.ok(text.includes(FULL_SUITE_COMMAND), text);
  });

  test("an uncovered path exits 3 and is counted", () => {
    const result = lookupPaths(["scripts/dev.mjs", "docs/a.md"], LIST);
    assert.deepEqual(result.uncovered, ["scripts/dev.mjs"]);
    assert.equal(exitCodeFor(result), 3);
    assert.match(formatLookup(result), /Uncovered: 1 path no slice proves/);
  });

  test("only documentation says nothing needs running", () => {
    const result = lookupPaths(["docs/a.md"], LIST);
    assert.deepEqual(result.run, []);
    assert.equal(exitCodeFor(result), 0);
    assert.match(formatLookup(result), /Run: nothing — no e2e needed/);
  });

  test("a spec reads as owner and neighbours in the report", () => {
    const result = lookupPaths(["apps/web/e2e/status-display.spec.ts"], LIST);
    assert.match(
      formatLookup(result),
      /status \(owner\) · combat \(neighbour\) · tokens \(neighbour\)/,
    );
  });
});
