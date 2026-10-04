import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DISCOVERED_WEB_ENTRIES,
  LINKAGES,
  packProblems,
} from "../check-packs.mjs";

const [LINKAGE, TEST_LINKAGE] = LINKAGES;

/** A tree as `{ path: contents }`, and the two arguments the check takes. */
function tree(files) {
  return [Object.keys(files), (file) => files[file] ?? ""];
}

/** A pack shaped like Roll for Shoes: a manifest, a server crate, a web half. */
function wholePack(extra = {}) {
  return {
    "packs/systems/README.md": "the contract",
    "packs/systems/shoes/system.json": "{}",
    "packs/systems/shoes/README.md": "",
    "packs/systems/shoes/server/Cargo.toml":
      '[package]\nname = "shoes-server"\n\n[lib]\nname = "shoes_server"\n',
    "packs/systems/shoes/server/src/lib.rs": "",
    "packs/systems/shoes/web/package.json": "{}",
    "packs/systems/shoes/web/src/ActorSheet.tsx": "",
    "packs/systems/shoes/web/src/components/Roll.tsx": "",
    [LINKAGE]: "use shoes_server as _;\n",
    [TEST_LINKAGE]: "use shoes_server as _;\n",
    ...extra,
  };
}

test("a pack with a manifest, a linked server crate and a discovered sheet passes", () => {
  assert.deepEqual(packProblems(...tree(wholePack())), []);
});

test("a pack that is only a manifest passes", () => {
  assert.deepEqual(
    packProblems(...tree({ "packs/systems/plain/system.json": "{}" })),
    [],
  );
});

test("a system directory with no system.json is refused", () => {
  const problems = packProblems(
    ...tree({ "packs/systems/ghost/README.md": "" }),
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^packs\/systems\/ghost: .*system\.json/);
});

test("an engine crate is refused, and the message says why", () => {
  const problems = packProblems(
    ...tree(wholePack({ "packs/systems/shoes/engine/Cargo.toml": "" })),
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^packs\/systems\/shoes\/engine: /);
  assert.match(problems[0], /ADR-062/);
});

test("an entry the contract does not list is refused by name", () => {
  const problems = packProblems(
    ...tree(wholePack({ "packs/systems/shoes/module/main.mjs": "" })),
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^packs\/systems\/shoes\/module: /);
});

test("a data file the pack's own crate reads is allowed", () => {
  const files = wholePack({
    "packs/systems/shoes/stat-blocks.json": "[]",
    "packs/systems/shoes/server/src/lib.rs":
      'include_str!("../../stat-blocks.json")',
  });
  assert.deepEqual(packProblems(...tree(files)), []);
});

test("a data file nothing in the pack's crate reads is refused", () => {
  const problems = packProblems(
    ...tree(wholePack({ "packs/systems/shoes/stat-blocks.json": "[]" })),
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^packs\/systems\/shoes\/stat-blocks\.json: /);
});

test("a web directory the host finds nothing in is refused, naming what it looks for", () => {
  const files = wholePack();
  delete files["packs/systems/shoes/web/src/ActorSheet.tsx"];
  const problems = packProblems(...tree(files));
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^packs\/systems\/shoes\/web: /);
  for (const entry of DISCOVERED_WEB_ENTRIES)
    assert.ok(problems[0].includes(entry), entry);
});

for (const [entry, file] of [
  ["a stat block source", "web/src/StatBlocks.ts"],
  ["a panel", "web/src/panels/clocks.tsx"],
]) {
  test(`${entry} alone is enough for a web directory`, () => {
    const files = wholePack({ [`packs/systems/shoes/${file}`]: "" });
    delete files["packs/systems/shoes/web/src/ActorSheet.tsx"];
    assert.deepEqual(packProblems(...tree(files)), []);
  });
}

test("a panel in a subdirectory is not one the host finds", () => {
  const files = wholePack({
    "packs/systems/shoes/web/src/panels/deep/clocks.tsx": "",
  });
  delete files["packs/systems/shoes/web/src/ActorSheet.tsx"];
  assert.equal(packProblems(...tree(files)).length, 1);
});

test("a server crate the application does not link is refused", () => {
  const problems = packProblems(
    ...tree(wholePack({ [LINKAGE]: "use other_server as _;\n" })),
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^packs\/systems\/shoes\/server: /);
  assert.ok(problems[0].includes("use shoes_server as _;"));
  assert.ok(problems[0].includes(LINKAGE));
});

test("a commented-out linkage line does not count", () => {
  const problems = packProblems(
    ...tree(wholePack({ [LINKAGE]: "// use shoes_server as _;\n" })),
  );
  assert.equal(problems.length, 1);
});

test("the crate's name comes from its package when it declares no lib name", () => {
  const files = wholePack({
    "packs/systems/shoes/server/Cargo.toml":
      '[package]\nname = "shoes-server"\n',
  });
  assert.deepEqual(packProblems(...tree(files)), []);
});

test("a server directory with no Cargo.toml is refused", () => {
  const files = wholePack();
  delete files["packs/systems/shoes/server/Cargo.toml"];
  const problems = packProblems(...tree(files));
  assert.equal(problems.length, 1);
  assert.match(problems[0], /Cargo\.toml/);
});

test("a server crate the server library's tests do not link is refused", () => {
  const problems = packProblems(...tree(wholePack({ [TEST_LINKAGE]: "" })));
  assert.equal(problems.length, 1);
  assert.ok(problems[0].includes(TEST_LINKAGE));
});
