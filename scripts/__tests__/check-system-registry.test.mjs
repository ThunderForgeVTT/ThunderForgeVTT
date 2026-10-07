import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { registryViolations } from "../check-system-registry.mjs";

/** A throwaway repository with one bundled system, `shoes`, and these files. */
function repo(files) {
  const root = mkdtempSync(path.join(os.tmpdir(), "registry-"));
  const all = { "packs/systems/shoes/system.json": "{}", ...files };
  for (const [file, contents] of Object.entries(all)) {
    mkdirSync(path.join(root, path.dirname(file)), { recursive: true });
    writeFileSync(path.join(root, file), contents);
  }
  return root;
}

function failuresIn(files) {
  const root = repo(files);
  try {
    return registryViolations(root, new Map()).failures;
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

const RUST = 'fn bars(id: &str) -> u8 { match id { "shoes" => 1, _ => 0 } }\n';
const WEB = 'export const sheet = system === "shoes" ? Shoes : Base;\n';

// Every root shared code lives under. The first two of each language are the
// ones the check always had; the rest are spec 066, FR-006.
for (const [file, source] of [
  ["crates/thunderforge-server/src/graphql.rs", RUST],
  ["apps/thunderforge/src/main.rs", RUST],
  ["apps/thunderforge-mapforge-server/src/main.rs", RUST],
  ["crates/thunderforge-engine/src/tokens/bars.rs", RUST],
  ["crates/thunderforge-core/src/models/token.rs", RUST],
  ["crates/thunderforge-canvas-core/src/system_rules.rs", RUST],
  ["apps/web/src/pages/world/Sheet.tsx", WEB],
  ["apps/hero-builder/src/main.tsx", WEB],
  ["packages/heroes/src/build.ts", WEB],
]) {
  test(`${file} may not quote a system id`, () => {
    assert.deepEqual(failuresIn({ [file]: source }), [
      `${file}:1 names "shoes"`,
    ]);
  });
}

test("a shared file named for a system is refused without quoting it", () => {
  assert.deepEqual(
    failuresIn({ "packages/heroes/src/ShoesPanel.tsx": "export {};\n" }),
    ['packages/heroes/src/ShoesPanel.tsx is named for "shoes"'],
  );
});

test("a pack naming itself is the point", () => {
  assert.deepEqual(
    failuresIn({
      "packs/systems/shoes/server/src/lib.rs":
        'pub const SYSTEM_ID: &str = "shoes";\n',
      "packs/systems/shoes/web/src/ActorSheet.tsx":
        'export const id = "shoes";\n',
    }),
    [],
  );
});

test("tests may name a system, in either language", () => {
  assert.deepEqual(
    failuresIn({
      "crates/thunderforge-engine/src/bars.rs":
        'pub fn bars() {}\n\n#[cfg(test)]\nmod tests {\n    const ID: &str = "shoes";\n}\n',
      "crates/thunderforge-core/src/token_tests.rs": RUST,
      "packages/heroes/src/build.test.ts": WEB,
      "packages/heroes/src/__tests__/build.ts": WEB,
    }),
    [],
  );
});

test("the linkage modules are exempt, and only by that name", () => {
  assert.deepEqual(
    failuresIn({
      "apps/thunderforge/src/system_packs.rs": 'const ALL: [&str; 1] = ["shoes"];\n',
      "crates/thunderforge-server/src/test_packs.rs":
        'const ALL: [&str; 1] = ["shoes"];\n',
    }),
    [],
  );
});

test("code after a test module is still scanned", () => {
  const source =
    '#[cfg(test)]\nmod tests {\n    const ID: &str = "shoes";\n}\n\n' + RUST;
  // The line it reports counts from the stripped source, so only the file and
  // the id are asserted here.
  const failures = failuresIn({
    "crates/thunderforge-engine/src/bars.rs": source,
  });
  assert.equal(failures.length, 1);
  assert.match(
    failures[0],
    /^crates\/thunderforge-engine\/src\/bars\.rs:\d+ names "shoes"$/,
  );
});
