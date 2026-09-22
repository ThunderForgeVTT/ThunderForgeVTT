/**
 * The declared slice list, and the questions every tool asks of it.
 *
 * A slice is a named feature area and the e2e specs that prove it (spec 060,
 * ADR-107). The list lives in `scripts/e2e/slices.json`, and this module is
 * the only thing that reads it: the runner's `--slice`, the lookup
 * (`e2e-slice.mjs which`), the listing and the coverage check all come here.
 * Two readers of one JSON file would sooner or later disagree about what
 * "owns" means, and the whole point of the list is that there is one answer.
 *
 * Everything here is pure. Nothing touches git or the network; the only I/O
 * is reading `slices.json` in `loadSlices` and walking the spec directory
 * through `allSpecFiles` when a caller does not pass its own file list. The
 * callers that need `git ls-files` or a diff (the check, the lookup) do that
 * themselves and hand the result in, which is also what lets the unit tests
 * run against in-memory fixtures instead of the real tree.
 */

import { readFileSync } from "node:fs";
import { join, posix } from "node:path";

import { ROOT_DIR } from "../shared.mjs";
import {
  allSpecFiles,
  isFirstRunSpec,
  isGithubAppsSpec,
  isPerfSpec,
} from "./specs.mjs";

/** Where the list lives, relative to the repository root. */
export const SLICES_FILE = "scripts/e2e/slices.json";

/**
 * Slice names that would collide with a script this feature already owns.
 * `e2e:slices` and `e2e:which` are the listing and the lookup, so a slice
 * called `slices` would make `pnpm e2e:slices` mean two things.
 */
export const RESERVED_SLICE_NAMES = ["slices", "which"];

/** The lanes a spec can land in, in the order the listing prints them. */
export const LANES = ["default", "measured", "first-run", "github-apps"];

const KEBAB = /^[a-z0-9]+(-[a-z0-9]+)*$/;
const SPEC_SUFFIX = ".spec.ts";

/**
 * Keys each object may carry. Anything else is an error, because the likely
 * "anything else" is a misspelling — `neighbors` for `neighbours` — and a
 * misspelt key that is silently ignored is a neighbour that silently never
 * runs.
 */
const DOCUMENT_KEYS = ["$comment", "crossCutting", "slices"];
const SLICE_KEYS = [
  "name",
  "summary",
  "own",
  "neighbours",
  "paths",
  "standalone",
];
const SEAM_KEYS = ["spec", "seam"];
const CROSS_CUTTING_KEYS = ["glob", "why"];

/**
 * Anything wrong with the list or a request made of it.
 *
 * A class of its own so a CLI can tell "you asked for something the list
 * cannot answer" (exit 2, print the message) from a bug (let it throw).
 */
export class SliceError extends Error {
  constructor(message) {
    super(message);
    this.name = "SliceError";
  }
}

/**
 * Read and validate `scripts/e2e/slices.json` under `root`.
 *
 * Returns `{ crossCutting, slices }`. `slices` is what `ownerOf`,
 * `resolveSlice` and the rest take.
 */
export function loadSlices(root = ROOT_DIR) {
  const file = join(root, SLICES_FILE);
  let text;
  try {
    text = readFileSync(file, "utf8");
  } catch (error) {
    throw new SliceError(`${SLICES_FILE}: cannot read (${error.message})`);
  }
  let document;
  try {
    document = JSON.parse(text);
  } catch (error) {
    throw new SliceError(`${SLICES_FILE}: not valid JSON (${error.message})`);
  }
  return validateSlices(document);
}

/**
 * The shape rules of `contracts/slices-json.md`, over an already-parsed
 * document. Separate from `loadSlices` so tests can feed it fixtures.
 *
 * This checks what the file *says*, not whether it matches the tree: a
 * prefix that selects nothing or a glob that matches no tracked file is the
 * coverage check's business, because only it has the file lists to hand.
 */
export function validateSlices(document) {
  const where = SLICES_FILE;
  if (!isObject(document)) {
    throw new SliceError(`${where}: the top level must be an object`);
  }
  rejectUnknownKeys(document, DOCUMENT_KEYS, where);
  if (!Array.isArray(document.slices)) {
    throw new SliceError(`${where}: "slices" must be an array`);
  }
  const crossCutting = document.crossCutting ?? [];
  if (!Array.isArray(crossCutting)) {
    throw new SliceError(`${where}: "crossCutting" must be an array`);
  }
  crossCutting.forEach((rule, index) => {
    const at = `${where}: crossCutting[${index}]`;
    if (!isObject(rule)) throw new SliceError(`${at} must be an object`);
    rejectUnknownKeys(rule, CROSS_CUTTING_KEYS, at);
    requireString(rule, "glob", at);
    requireString(rule, "why", at);
  });

  const seen = new Set();
  let previous = null;
  for (const [index, slice] of document.slices.entries()) {
    const at = `${where}: slices[${index}]`;
    if (!isObject(slice)) throw new SliceError(`${at} must be an object`);
    rejectUnknownKeys(slice, SLICE_KEYS, `${at} ("${slice.name}")`);
    requireString(slice, "name", at);
    const named = `${where}: slice "${slice.name}"`;
    if (!KEBAB.test(slice.name)) {
      throw new SliceError(`${named}: the name must be kebab-case`);
    }
    if (RESERVED_SLICE_NAMES.includes(slice.name)) {
      throw new SliceError(
        `${named}: the name collides with the reserved script "e2e:${slice.name}"`,
      );
    }
    if (seen.has(slice.name)) {
      throw new SliceError(`${named}: the name is used twice`);
    }
    seen.add(slice.name);
    // Sorted so that adding a slice is a one-place diff and two agents adding
    // slices at once conflict only when they pick neighbouring names.
    if (previous !== null && slice.name < previous) {
      throw new SliceError(
        `${named}: slices must be sorted by name, and "${slice.name}" comes before "${previous}"`,
      );
    }
    previous = slice.name;

    requireString(slice, "summary", named);
    requireStringArray(slice, "own", named);
    if (slice.own.length === 0) {
      throw new SliceError(`${named}: "own" must name at least one spec`);
    }
    requireStringArray(slice, "paths", named);
    if (!Array.isArray(slice.neighbours)) {
      throw new SliceError(`${named}: "neighbours" must be an array`);
    }
    slice.neighbours.forEach((neighbour, n) => {
      const nat = `${named}: neighbours[${n}]`;
      if (!isObject(neighbour)) {
        throw new SliceError(`${nat} must be an object`);
      }
      rejectUnknownKeys(neighbour, SEAM_KEYS, nat);
      requireString(neighbour, "spec", nat);
      requireString(neighbour, "seam", nat);
      if (!neighbour.spec.endsWith(SPEC_SUFFIX)) {
        throw new SliceError(
          `${nat}: a neighbour is an exact spec name ending in ${SPEC_SUFFIX}, not "${neighbour.spec}"`,
        );
      }
    });
    if ("standalone" in slice) requireString(slice, "standalone", named);
  }
  return { crossCutting, slices: document.slices };
}

/**
 * A spec's name as `own` and `neighbours` spell it: relative to
 * `apps/web/e2e`, so `torture/lots-of-tokens.spec.ts`.
 *
 * Accepts the runner's form (`e2e/x.spec.ts`, relative to `apps/web`), the
 * repository form (`apps/web/e2e/x.spec.ts`) or the bare name, so a caller
 * holding any of them gets the same answer.
 */
export function specKey(specPath) {
  let key = specPath.replaceAll("\\", "/");
  if (key.startsWith("apps/web/")) key = key.slice("apps/web/".length);
  if (key.startsWith("e2e/")) key = key.slice("e2e/".length);
  return key;
}

/** Whether an `own` entry is an exact spec name rather than a prefix. */
export function isExactEntry(entry) {
  return entry.endsWith(SPEC_SUFFIX);
}

/** Whether one `own` entry selects `specPath`. */
export function entryMatches(entry, specPath) {
  const key = specKey(specPath);
  return isExactEntry(entry) ? key === entry : key.startsWith(entry);
}

/**
 * Which slice owns `specPath`, and through which entry.
 *
 * Returns `{ slice, entry, exact }`, or `null` when no slice owns it. The
 * most specific entry wins: an exact name beats any prefix, and a longer
 * prefix beats a shorter one (R2). Two *different* slices matching at the
 * same specificity is a tie, and a tie throws, naming both — there is no
 * right answer to pick silently, and the lookup's whole promise is one owner.
 */
export function ownershipOf(specPath, slices) {
  const matches = [];
  for (const slice of slices) {
    for (const entry of slice.own) {
      if (!entryMatches(entry, specPath)) continue;
      const score = isExactEntry(entry) ? Infinity : entry.length;
      matches.push({ slice: slice.name, entry, score });
    }
  }
  if (matches.length === 0) return null;
  // Only a tie at the *winning* specificity matters. Two slices sharing a
  // prefix is harmless when a third names the file exactly.
  const top = Math.max(...matches.map((match) => match.score));
  const winners = matches.filter((match) => match.score === top);
  const rival = winners.find((match) => match.slice !== winners[0].slice);
  if (rival) {
    throw new SliceError(
      `${specKey(specPath)} is owned by both "${winners[0].slice}" ("${winners[0].entry}") and "${rival.slice}" ("${rival.entry}") — make one entry more specific`,
    );
  }
  const { slice, entry } = winners[0];
  return { slice, entry, exact: top === Infinity };
}

/** The name of the slice that owns `specPath`, or `null`. */
export function ownerOf(specPath, slices) {
  return ownershipOf(specPath, slices)?.slice ?? null;
}

/**
 * The spec files `--slice=<name>` runs: the slice's own specs and its
 * neighbours, sorted, each once.
 *
 * Paths come back exactly as `specFiles` spells them — by default
 * `allSpecFiles()`, so `e2e/combat-panel.spec.ts`, relative to `apps/web`,
 * which is what the runner hands Playwright. Exact paths rather than
 * patterns, because `--only`'s substring match is the trap this avoids:
 * `--only=lighting` also selects `engine-lighting-limits`.
 */
export function resolveSlice(name, slices, specFiles = allSpecFiles()) {
  const slice = slices.find((candidate) => candidate.name === name);
  if (!slice) {
    throw new SliceError(
      `unknown slice "${name}". Valid slices: ${slices.map((s) => s.name).join(", ")}`,
    );
  }
  const chosen = new Set(
    specFiles.filter((file) => ownerOf(file, slices) === name),
  );
  for (const { spec } of slice.neighbours) {
    const file = specFiles.find((candidate) => specKey(candidate) === spec);
    if (!file) {
      throw new SliceError(
        `slice "${name}": neighbour "${spec}" does not exist`,
      );
    }
    chosen.add(file);
  }
  return [...chosen].sort();
}

/**
 * The runner lanes these specs fall in, in `LANES` order.
 *
 * The same partition `e2e-parallel.mjs` makes: first-run and GitHub-apps
 * specs are taken out first, then measured specs, then the rest run sharded.
 * Informational — the listing prints it so that "this slice triggers a
 * release build" is visible before anyone runs it.
 */
export function sliceLanes(paths) {
  const lanes = new Set();
  for (const file of paths) {
    if (isFirstRunSpec(file)) lanes.add("first-run");
    else if (isGithubAppsSpec(file)) lanes.add("github-apps");
    else if (isPerfSpec(file)) lanes.add("measured");
    else lanes.add("default");
  }
  return LANES.filter((lane) => lanes.has(lane));
}

/**
 * The first of `globs` that matches the repository path `path`, or `null`.
 *
 * `path.posix.matchesGlob`, so `**` crosses directories and the answer does
 * not depend on the platform's separator.
 */
export function matchPath(path, globs) {
  const normalised = path.replaceAll("\\", "/");
  return globs.find((glob) => posix.matchesGlob(normalised, glob)) ?? null;
}

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function rejectUnknownKeys(object, allowed, where) {
  for (const key of Object.keys(object)) {
    if (!allowed.includes(key)) {
      throw new SliceError(
        `${where}: unknown key "${key}" (allowed: ${allowed.join(", ")})`,
      );
    }
  }
}

function requireString(object, key, where) {
  if (typeof object[key] !== "string" || object[key].length === 0) {
    throw new SliceError(`${where}: "${key}" must be a non-empty string`);
  }
}

function requireStringArray(object, key, where) {
  const value = object[key];
  if (!Array.isArray(value) || value.some((item) => typeof item !== "string")) {
    throw new SliceError(`${where}: "${key}" must be an array of strings`);
  }
}
