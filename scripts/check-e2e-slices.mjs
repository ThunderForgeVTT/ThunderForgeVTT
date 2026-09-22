#!/usr/bin/env node
/**
 * Every e2e spec belongs to a slice, and the slice list still describes the
 * tree (spec 060, FR-013 and FR-014; research R7; ADR-107).
 *
 * A slice is how a change is proven in minutes rather than hours: the
 * feature's own specs plus the neighbours that read what it writes. That only
 * works while the list is true. A spec added without a slice is a spec no
 * slice run will ever execute, and nothing fails to tell anyone — the full
 * suite still runs it, so it looks covered right up until the day someone
 * trusts the slice. The same goes for a prefix that stopped matching after a
 * rename, a `paths` glob that points at a moved directory, or a package
 * script that drifted from the name it carries. Each of these decays
 * silently, which is why it is a check rather than a convention.
 *
 * # The rules (R7)
 *
 *  1. every spec under `apps/web/e2e` (the runner's set: `torture/` in,
 *     `journeys/` out) has an owner;
 *  2. every exact `own` name and every neighbour names a spec that exists;
 *  3. every `own` prefix matches at least one spec;
 *  4. every `paths` and cross-cutting glob matches a tracked file;
 *  5. no spec is owned by two slices at the same specificity;
 *  6. the root `e2e:<slice>*` scripts are exactly the canonical ones (R4);
 *  7. a slice's `standalone` command names a package script that exists;
 *  8. slice names are kebab-case and not reserved;
 *  9. `slice-durations.json` has no record for a slice that is gone.
 *
 * Rule 8 and the shape of the file are `validateSlices`' business; this only
 * reports what it says. The ownership rules come from `ownershipOf`, so the
 * check and the runner cannot disagree about who owns a spec.
 *
 * # `--fix`
 *
 * Rewrites only the `e2e:<slice>*` scripts in the root `package.json`. They
 * carry nothing but the slice's name, so there is exactly one right body and
 * a script can write it. Membership is never guessed: which slice a new spec
 * belongs to is a judgement about the feature, and `slices.json` is left for
 * a person to edit.
 *
 * # Cost
 *
 * One `git ls-files`, one directory walk and a few JSON files, so it runs on
 * every commit (`.hooks/pre-commit`). The unit tests for the slice tooling run
 * after a clean pass, because they are just as cheap and are what proves these
 * rules still mean what this comment says.
 */

import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import {
  RESERVED_SLICE_NAMES,
  SLICES_FILE,
  SliceError,
  entryMatches,
  isExactEntry,
  matchPath,
  ownershipOf,
  specKey,
  validateSlices,
} from "./e2e/slices.mjs";
import {
  SLICE_DURATIONS_FILE,
  readSliceDurations,
} from "./e2e/slice-durations.mjs";
import { allSpecFiles } from "./e2e/specs.mjs";

const repoRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

const SPEC_DIR = "apps/web/e2e";

/** The scripts this feature owns that are not a slice's. */
const RESERVED_SCRIPTS = RESERVED_SLICE_NAMES.map((name) => `e2e:${name}`);

/**
 * What counts as a slice script. `e2e:<name>` and its two halves; anything
 * else under `e2e:` is left alone, so an unrelated script is never reported
 * as "extra" or removed by `--fix`.
 */
const SLICE_SCRIPT =
  /^e2e:([a-z0-9]+(?:-[a-z0-9]+)*)(?::(integration|standalone))?$/;

/**
 * The scripts a slice must have, in the order `--fix` writes them. The
 * aggregate first, then the halves in the order it runs them — the order the
 * hero builder's hand-written scripts already had.
 */
export function canonicalScripts(slice) {
  const integration = `node ./scripts/e2e-parallel.mjs --shards=1 --slice=${slice.name}`;
  const scripts = {};
  if (slice.standalone) {
    scripts[`e2e:${slice.name}`] =
      `pnpm run e2e:${slice.name}:standalone && pnpm run e2e:${slice.name}:integration`;
    scripts[`e2e:${slice.name}:standalone`] = slice.standalone;
  } else {
    scripts[`e2e:${slice.name}`] = `pnpm run e2e:${slice.name}:integration`;
  }
  scripts[`e2e:${slice.name}:integration`] = integration;
  return scripts;
}

/**
 * Every problem with the slice list, as one line each, naming the file and
 * the fix. An empty array means the list is true.
 *
 * Pure: the caller hands in everything read from disk, so the tests run it
 * against fixtures and the CLI runs it against the tree.
 *
 * - `document`: `slices.json`, parsed.
 * - `specFiles`: the runner's spec paths (`e2e/x.spec.ts`).
 * - `trackedFiles`: `git ls-files`, repository-relative.
 * - `packageJson`: the root `package.json`, parsed.
 * - `sliceDurations`: `slice-durations.json`, parsed; `{}` when absent.
 * - `workspacePackages`: package name → parsed `package.json`, for rule 7.
 */
export function checkSlices({
  document,
  specFiles,
  trackedFiles,
  packageJson,
  sliceDurations = {},
  workspacePackages = {},
}) {
  let list;
  try {
    list = validateSlices(document);
  } catch (error) {
    if (!(error instanceof SliceError)) throw error;
    // A list that is the wrong shape cannot be asked the other questions,
    // and guessing past the error would bury it under consequences of it.
    return [error.message];
  }
  const { slices, crossCutting } = list;
  return [
    ...ownershipProblems(slices, specFiles),
    ...entryProblems(slices, specFiles),
    ...globProblems(slices, crossCutting, trackedFiles),
    ...scriptProblems(slices, packageJson),
    ...standaloneProblems(slices, packageJson, workspacePackages),
    ...durationProblems(slices, sliceDurations),
  ];
}

/** Rules 1 and 5: each spec has exactly one owner. */
function ownershipProblems(slices, specFiles) {
  const problems = [];
  for (const file of specFiles) {
    let owner;
    try {
      owner = ownershipOf(file, slices);
    } catch (error) {
      if (!(error instanceof SliceError)) throw error;
      problems.push(`${SLICES_FILE}: ${error.message}`);
      continue;
    }
    if (owner === null) {
      problems.push(
        `${SPEC_DIR}/${specKey(file)} belongs to no slice — add it to "own" of a slice in ${SLICES_FILE}`,
      );
    }
  }
  return problems;
}

/** Rules 2 and 3: every entry still points at something. */
function entryProblems(slices, specFiles) {
  const keys = new Set(specFiles.map(specKey));
  const problems = [];
  for (const slice of slices) {
    for (const entry of slice.own) {
      if (isExactEntry(entry)) {
        if (!keys.has(entry)) {
          problems.push(
            `slice "${slice.name}": "${entry}" in "own" does not exist — fix or remove it in ${SLICES_FILE}`,
          );
        }
      } else if (!specFiles.some((file) => entryMatches(entry, file))) {
        problems.push(
          `slice "${slice.name}": prefix "${entry}" in "own" matches no spec — fix or remove it in ${SLICES_FILE}`,
        );
      }
    }
    for (const { spec } of slice.neighbours) {
      if (!keys.has(spec)) {
        problems.push(
          `slice "${slice.name}": neighbour "${spec}" does not exist`,
        );
        continue;
      }
      // A neighbour the slice already owns runs anyway; listing it says the
      // seam is with another feature when it is not, and it would hide the
      // day the spec moves to its real owner.
      let owner = null;
      try {
        owner = ownershipOf(spec, slices)?.slice ?? null;
      } catch {
        // A tie is reported once, by the ownership rule.
      }
      if (owner === slice.name) {
        problems.push(
          `slice "${slice.name}": neighbour "${spec}" is its own spec — remove it from "neighbours" in ${SLICES_FILE}`,
        );
      }
    }
  }
  return problems;
}

/** Rule 4: a glob that matches nothing is a path that moved. */
function globProblems(slices, crossCutting, trackedFiles) {
  const problems = [];
  const dead = (glob) => !matchesAny(glob, trackedFiles);
  for (const slice of slices) {
    for (const glob of slice.paths) {
      if (dead(glob)) {
        problems.push(
          `slice "${slice.name}": path "${glob}" matches no tracked file — fix or remove it in ${SLICES_FILE}`,
        );
      }
    }
  }
  for (const { glob } of crossCutting) {
    if (dead(glob)) {
      problems.push(
        `cross-cutting "${glob}" matches no tracked file — fix or remove it in ${SLICES_FILE}`,
      );
    }
  }
  return problems;
}

/**
 * Whether `glob` matches any of `files`. The literal head of the glob (the
 * part before its first wildcard) filters first, so a glob is compared with
 * the handful of files under its directory rather than the whole tree.
 * Extglob openers (`!(`, `@(`, `+(`) end the head too, or `a/!(b)/**`
 * would look for files under a literal `a/!(b)/`.
 */
function matchesAny(glob, files) {
  const head = glob.split(/[*?[{(!@+]/, 1)[0];
  return files.some(
    (file) => file.startsWith(head) && matchPath(file, [glob]) !== null,
  );
}

/** Rule 6: the root scripts are exactly the canonical ones. */
function scriptProblems(slices, packageJson) {
  const scripts = packageJson.scripts ?? {};
  const expected = Object.assign({}, ...slices.map(canonicalScripts));
  const problems = [];
  for (const [name, body] of Object.entries(expected)) {
    if (!(name in scripts)) {
      problems.push(
        `package.json: "${name}" is missing — it must be "${body}" (run with --fix)`,
      );
    } else if (scripts[name] !== body) {
      problems.push(
        `package.json: "${name}" must be "${body}" (run with --fix)`,
      );
    }
  }
  for (const name of Object.keys(scripts)) {
    if (isSliceScript(name) && !(name in expected)) {
      problems.push(
        `package.json: "${name}" belongs to no slice — add the slice to ${SLICES_FILE}, or remove the script (run with --fix)`,
      );
    }
  }
  return problems;
}

function isSliceScript(name) {
  return SLICE_SCRIPT.test(name) && !RESERVED_SCRIPTS.includes(name);
}

/**
 * Rule 7: the standalone half must run something. A `standalone` naming a
 * script that was renamed makes `pnpm e2e:<slice>` fail before it proves
 * anything, and the failure reads like a broken harness rather than a stale
 * list.
 */
function standaloneProblems(slices, packageJson, workspacePackages) {
  const problems = [];
  for (const slice of slices) {
    if (!slice.standalone) continue;
    const target = parsePnpmCommand(slice.standalone);
    if (target === null) {
      problems.push(
        `slice "${slice.name}": cannot tell which script standalone "${slice.standalone}" runs — write it as "pnpm -F <package> <script>" in ${SLICES_FILE}`,
      );
      continue;
    }
    const manifest =
      target.package === null ? packageJson : workspacePackages[target.package];
    if (manifest === undefined) {
      problems.push(
        `slice "${slice.name}": standalone names package "${target.package}", which is not in the workspace — fix it in ${SLICES_FILE}`,
      );
    } else if (!(target.script in (manifest.scripts ?? {}))) {
      problems.push(
        `slice "${slice.name}": standalone runs "${target.script}", which ${
          target.package ?? "the root package.json"
        } has no script for — add the script or fix "standalone" in ${SLICES_FILE}`,
      );
    }
  }
  return problems;
}

/**
 * `{ package, script }` from `pnpm -F <pkg> [run] <script>`,
 * `pnpm --filter <pkg> …` or `pnpm [run] <script>` (package `null`, the
 * root). Anything else is `null`: a standalone the check cannot read is one
 * it cannot vouch for.
 */
export function parsePnpmCommand(command) {
  const words = command.trim().split(/\s+/);
  if (words.shift() !== "pnpm") return null;
  let pkg = null;
  if (words[0] === "-F" || words[0] === "--filter") {
    words.shift();
    pkg = words.shift() ?? null;
    if (pkg === null) return null;
  } else if (words[0]?.startsWith("--filter=")) {
    pkg = words.shift().slice("--filter=".length);
  }
  if (words[0] === "run") words.shift();
  const script = words[0];
  if (!script || script.startsWith("-")) return null;
  return { package: pkg, script };
}

/** Rule 9: a measurement of a slice that is gone is a number nobody owns. */
function durationProblems(slices, sliceDurations) {
  const names = new Set(slices.map((slice) => slice.name));
  return Object.keys(sliceDurations)
    .filter((name) => !names.has(name))
    .map(
      (name) =>
        `${SLICE_DURATIONS_FILE}: a record for "${name}", which is no longer a slice — delete it`,
    );
}

/**
 * `package.json`'s text with the slice scripts made canonical, or the same
 * text when they already are.
 *
 * Every other key keeps its place and its value. A script that is missing is
 * written next to its slice's others, or, for a slice with none, between the
 * slices it sorts between, so the block stays in the order `slices.json`
 * keeps. The file is re-serialised with its own indentation; `package.json`
 * is what pnpm writes, so that round-trips byte for byte.
 */
export function fixPackageJson(text, slices) {
  const manifest = JSON.parse(text);
  const scripts = manifest.scripts ?? {};
  const expected = new Map(
    slices.map((slice) => [slice.name, canonicalScripts(slice)]),
  );
  const sliceOf = (name) =>
    isSliceScript(name) ? SLICE_SCRIPT.exec(name)[1] : null;

  // Existing keys, in order, with slice scripts corrected or dropped.
  const entries = [];
  for (const [name, body] of Object.entries(scripts)) {
    const owner = sliceOf(name);
    if (owner === null) entries.push([name, body]);
    else if (expected.get(owner)?.[name] !== undefined) {
      entries.push([name, expected.get(owner)[name]]);
    }
  }

  const sorted = [...expected.keys()].sort();
  for (const name of sorted) {
    for (const [script, body] of Object.entries(expected.get(name))) {
      if (entries.some(([key]) => key === script)) continue;
      entries.splice(insertionPoint(entries, name, sliceOf), 0, [script, body]);
    }
  }

  manifest.scripts = Object.fromEntries(entries);
  const indent = /^\{\n([ \t]+)"/.exec(text)?.[1] ?? "  ";
  return `${JSON.stringify(manifest, null, indent)}${text.endsWith("\n") ? "\n" : ""}`;
}

/**
 * Where a missing script of slice `name` goes: after the last script of
 * its own slice or of any slice sorting before it, else before the first of
 * any slice after it, else at the end.
 */
function insertionPoint(entries, name, sliceOf) {
  let after = -1;
  let before = -1;
  entries.forEach(([key], index) => {
    const owner = sliceOf(key);
    if (owner === null) return;
    if (owner <= name) {
      after = index;
    } else if (before === -1) {
      before = index;
    }
  });
  if (after !== -1) return after + 1;
  if (before !== -1) return before;
  return entries.length;
}

/** Every workspace package's manifest, by name, from the tracked files. */
function readWorkspacePackages(root, trackedFiles) {
  const packages = {};
  for (const file of trackedFiles) {
    if (file !== "package.json" && !file.endsWith("/package.json")) continue;
    const manifest = JSON.parse(readFileSync(path.join(root, file), "utf8"));
    if (manifest.name) packages[manifest.name] = manifest;
  }
  return packages;
}

function main() {
  const fix = process.argv.includes("--fix");
  const packageFile = path.join(repoRoot, "package.json");
  let document;
  try {
    document = JSON.parse(
      readFileSync(path.join(repoRoot, SLICES_FILE), "utf8"),
    );
  } catch (error) {
    process.stdout.write(`${SLICES_FILE}: cannot read (${error.message})\n`);
    process.exit(1);
  }

  if (fix) {
    try {
      const { slices } = validateSlices(document);
      const before = readFileSync(packageFile, "utf8");
      const after = fixPackageJson(before, slices);
      if (after !== before) {
        writeFileSync(packageFile, after);
        process.stdout.write("package.json: slice scripts rewritten\n");
      }
    } catch (error) {
      if (!(error instanceof SliceError)) throw error;
      // Nothing to write from a list that is the wrong shape; the check
      // below reports why.
    }
  }

  const trackedFiles = execFileSync("git", ["ls-files", "-z"], {
    cwd: repoRoot,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  })
    .split("\0")
    .filter(Boolean);
  const specFiles = allSpecFiles();
  const problems = checkSlices({
    document,
    specFiles,
    trackedFiles,
    packageJson: JSON.parse(readFileSync(packageFile, "utf8")),
    sliceDurations: readSliceDurations(repoRoot),
    workspacePackages: readWorkspacePackages(repoRoot, trackedFiles),
  });

  if (problems.length > 0) {
    for (const problem of problems) process.stdout.write(`${problem}\n`);
    process.stdout.write(
      `\ne2e slices: ${problems.length} problem${problems.length === 1 ? "" : "s"}\n`,
    );
    process.exit(1);
  }
  process.stdout.write(
    `e2e slices: ${specFiles.length} specs in ${document.slices.length} slices, 0 orphans\n`,
  );

  // The glob, not the directory: `node --test <dir>` stopped meaning "the
  // tests in it" in Node 24.
  const tests = spawnSync(
    process.execPath,
    ["--test", "--test-reporter=dot", "scripts/e2e/__tests__/*.test.mjs"],
    { cwd: repoRoot, stdio: "inherit" },
  );
  process.exit(tests.status ?? 1);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main();
}
