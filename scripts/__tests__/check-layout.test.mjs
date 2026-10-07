import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  crossAppImports,
  crossAppProblems,
  rustProblems,
  workspaceMembers,
} from "../check-layout.mjs";

/** A throwaway workspace: `{ member: "bin" | "lib" }`. */
function workspace(members) {
  const root = mkdtempSync(path.join(os.tmpdir(), "layout-"));
  const names = Object.keys(members);
  writeFileSync(
    path.join(root, "Cargo.toml"),
    `[workspace]\nmembers = [\n${names.map((name) => `    "${name}",`).join("\n")}\n]\n`,
  );
  for (const [name, kind] of Object.entries(members)) {
    const src = path.join(root, name, "src");
    mkdirSync(src, { recursive: true });
    writeFileSync(path.join(root, name, "Cargo.toml"), `[package]\nname = "x"\n`);
    writeFileSync(path.join(src, kind === "bin" ? "main.rs" : "lib.rs"), "");
  }
  return root;
}

function check(members, misfits = new Map()) {
  const root = workspace(members);
  try {
    return rustProblems(root, misfits);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
}

test("reads the members list, trailing spaces and all", () => {
  assert.deepEqual(
    workspaceMembers(`[workspace]\nmembers = [\n  "apps/a", \n  "crates/b"\n]\ndefault-members = ["apps/a"]\n`),
    ["apps/a", "crates/b"],
  );
});

test("a tree that follows the rule passes and counts its members", () => {
  const result = check({
    "apps/thunderforge": "bin",
    "crates/core": "lib",
    "packs/systems/dnd5e/server": "lib",
  });
  assert.deepEqual(result, { members: 3, problems: [] });
});

test("a member outside the homes is refused, by name, with the homes", () => {
  const { problems } = check({ "apps/thunderforge": "bin", "src/extra": "lib" });
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^src\/extra: /);
  assert.match(problems[0], /apps\/, crates\/, packs\/systems\//);
});

test("a library under apps/ and a binary under crates/ are refused", () => {
  const { problems } = check({ "apps/notabin": "lib", "crates/hasbin": "bin" });
  assert.equal(problems.length, 2);
  assert.match(problems[0], /^apps\/notabin: .*builds no binary/);
  assert.match(problems[1], /^crates\/hasbin: .*builds a binary/);
});

test("a named exception is let through, and a stale one is refused", () => {
  const misfits = new Map([["crates/hasbin", "ships a server"]]);
  assert.deepEqual(check({ "crates/hasbin": "bin" }, misfits).problems, []);
  const { problems } = check({ "crates/hasbin": "lib" }, misfits);
  assert.equal(problems.length, 1);
  assert.match(problems[0], /no longer breaks the rule/);
});

test("a [[bin]] table counts as a binary without a main.rs", () => {
  const root = workspace({ "crates/lib": "lib" });
  try {
    writeFileSync(
      path.join(root, "crates/lib/Cargo.toml"),
      `[package]\nname = "x"\n\n[[bin]]\nname = "tool"\npath = "src/tool.rs"\n`,
    );
    assert.equal(rustProblems(root, new Map()).problems.length, 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

const APPS = [
  ["apps/web/src/main.ts", `import { a } from "./a";\nimport "@scope/pkg";\n`],
  ["apps/other/src/main.ts", ""],
];

test("an app importing its own files and packages is fine", () => {
  assert.deepEqual(crossAppImports(APPS), []);
});

test("a relative import into another app is found", () => {
  const found = crossAppImports([
    ...APPS,
    ["apps/other/src/x.ts", `import { store } from "../../web/src/store";\n`],
  ]);
  assert.deepEqual(found, [
    {
      file: "apps/other/src/x.ts",
      specifier: "../../web/src/store",
      from: "other",
      to: "web",
    },
  ]);
});

test("a dynamic import and a package-name import are found too", () => {
  const found = crossAppImports(
    [
      ...APPS,
      ["apps/other/src/y.ts", `const m = await import("../../web/src/m");\n`],
      ["apps/other/src/z.ts", `import { t } from "@scope/web/theme";\n`],
    ],
    new Map([["@scope/web", "web"]]),
  );
  assert.deepEqual(
    found.map(({ file, to }) => [file, to]),
    [
      ["apps/other/src/y.ts", "web"],
      ["apps/other/src/z.ts", "web"],
    ],
  );
});

test("a path that climbs out of an app but into no app is not counted", () => {
  assert.deepEqual(
    crossAppImports([
      ...APPS,
      ["apps/web/e2e/types.d.ts", `type T = typeof import("../../src/engine/index");\n`],
      ["apps/web/src/p.ts", `import { h } from "../../../packages/heroes/src";\n`],
    ]),
    [],
  );
});

test("a cross-app import is refused unless its pair is listed, and a stale pair is refused", () => {
  const found = [
    { file: "apps/other/src/x.ts", specifier: "../../web/src/s", from: "other", to: "web" },
  ];
  const refused = crossAppProblems(found, new Map());
  assert.equal(refused.length, 1);
  assert.match(refused[0], /^apps\/other\/src\/x\.ts: .*apps\/web.*packages\//);
  assert.deepEqual(
    crossAppProblems(found, new Map([["other -> web", "until story 4"]])),
    [],
  );
  const stale = crossAppProblems([], new Map([["other -> web", "until story 4"]]));
  assert.match(stale[0], /no such import remains/);
});
