import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import {
  budgetProblems,
  chunkName,
  closure,
  forbiddenReaches,
  measure,
  readBuild,
  staticImports,
} from "../check-bundle-budget.mjs";

/**
 * A throwaway build: `{ "entry/index-AAAAAAAA.js": { code, sources } }`.
 * Hashes are eight characters, as the bundler writes them.
 */
function build(files) {
  const root = mkdtempSync(path.join(os.tmpdir(), "bundle-"));
  for (const [relative, { code, sources = [] }] of Object.entries(files)) {
    const full = path.join(root, "assets", relative);
    mkdirSync(path.dirname(full), { recursive: true });
    writeFileSync(full, code);
    writeFileSync(`${full}.map`, JSON.stringify({ sources }));
  }
  return readBuild(root);
}

const EDITOR = "../node_modules/@codemirror/view/dist/index.js";

function app({ pageImportsEditor }) {
  return build({
    "entry/index-AAAAAAAA.js": {
      code: 'import{a}from"../chunks/react-BBBBBBBB.js";const p=()=>import("../chunks/ScenePage-CCCCCCCC.js");',
    },
    "chunks/react-BBBBBBBB.js": { code: "export const a=1;" },
    "chunks/ScenePage-CCCCCCCC.js": {
      code: pageImportsEditor
        ? 'import{a}from"./react-BBBBBBBB.js";import{e}from"./esm-DDDDDDDD.js";'
        : 'import{a}from"./react-BBBBBBBB.js";const e=()=>import("./Editor-EEEEEEEE.js");',
    },
    "chunks/Editor-EEEEEEEE.js": { code: 'import{e}from"./esm-DDDDDDDD.js";' },
    "chunks/esm-DDDDDDDD.js": {
      code: `export const e="${"x".repeat(4000)}";`,
      sources: [EDITOR],
    },
  });
}

const RULE = [
  {
    source: "node_modules/@codemirror/",
    only: ["Editor"],
    why: "Reach it through React.lazy.",
  },
];

test("a chunk is named without its hash", () => {
  assert.equal(chunkName("SceneDetailPage-UCFdi3t5.js"), "SceneDetailPage");
  assert.equal(chunkName("rolldown-runtime-81CNApCO.js"), "rolldown-runtime");
  assert.equal(chunkName("esm-xQ_8-xF5.js"), "esm");
});

test("static imports are read and dynamic ones are not", () => {
  const source =
    'import{a as b}from"./a-11111111.js";import"./b-22222222.js";export{c}from"../chunks/c-33333333.js";const d=()=>import("./d-44444444.js");';
  assert.deepEqual(staticImports(source).sort(), [
    "a-11111111.js",
    "b-22222222.js",
    "c-33333333.js",
  ]);
});

test("a closure follows static imports only", () => {
  const chunks = app({ pageImportsEditor: false });
  assert.deepEqual([...closure(chunks, "index-AAAAAAAA.js")].sort(), [
    "index-AAAAAAAA.js",
    "react-BBBBBBBB.js",
  ]);
});

test("a route is measured beyond the entry", () => {
  const chunks = app({ pageImportsEditor: false });
  const measured = measure(chunks, ["ScenePage"]);
  assert.equal(measured.entry.files, 2);
  // Only the page itself: react is the entry's, the editor is behind import().
  assert.equal(measured.routes.ScenePage.files, 1);
});

test("a static import of the editor puts it in the route and over budget", () => {
  const lazy = measure(app({ pageImportsEditor: false }), ["ScenePage"]);
  const eager = measure(app({ pageImportsEditor: true }), ["ScenePage"]);
  assert.equal(eager.routes.ScenePage.files, 2);
  assert.ok(eager.routes.ScenePage.raw > lazy.routes.ScenePage.raw + 4000);

  const budget = {
    entry: { brotli: 10_000 },
    routes: { ScenePage: { brotli: lazy.routes.ScenePage.brotli + 10 } },
  };
  assert.deepEqual(budgetProblems(lazy, budget), []);
  const [problem] = budgetProblems(eager, budget);
  assert.match(problem, /^ScenePage beyond the entry is \d+ bytes brotli/);
});

test("a budgeted route the build no longer has is a failure, not a zero", () => {
  const measured = measure(app({ pageImportsEditor: false }), ["GonePage"]);
  const problems = budgetProblems(measured, {
    entry: { brotli: 10_000 },
    routes: { GonePage: { brotli: 1 } },
  });
  assert.match(problems[0], /No chunk named GonePage/);
});

test("the forbidden rule names who imported the library the plain way", () => {
  assert.deepEqual(
    forbiddenReaches(app({ pageImportsEditor: false }), RULE),
    [],
  );
  const problems = forbiddenReaches(app({ pageImportsEditor: true }), RULE);
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^ScenePage imports node_modules\/@codemirror\//);
  assert.match(problems[0], /ScenePage-CCCCCCCC\.js → esm-DDDDDDDD\.js/);
});
