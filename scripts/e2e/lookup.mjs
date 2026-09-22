/**
 * The lookup: which slice proves a change to these paths (spec 060, R5).
 *
 * `scripts/e2e-slice.mjs which` asks git for the paths and hands them here.
 * Everything in this module is pure — no git, no file reads — so the unit
 * tests can pass a path list and a fixture slice list and check every answer
 * without a repository around them.
 *
 * Each path gets exactly one kind of answer, tried in this order:
 *
 * 1. **crossCutting** — the path matches a cross-cutting rule, so any slice
 *    could break and the answer is the full suite. First, because a path that
 *    is both a combat source file and the GraphQL schema is the schema: naming
 *    combat would be an answer that is right about the part that matters least.
 * 2. **spec** — the path is an e2e spec, so its owner runs it, and so does every
 *    slice that borrows it as a neighbour (a change to the spec can break the
 *    seam they rely on it for).
 * 3. **slice** — the path matches some slices' `paths` globs.
 * 4. **noE2e** — docs, specs and Markdown, which no browser ever loads.
 * 5. **uncovered** — nothing claims it. Said out loud (FR-012), never guessed.
 */

import { matchPath, ownershipOf, specKey } from "./slices.mjs";

/**
 * Paths whose change needs no e2e. Built in rather than declared in
 * `slices.json`, so that nobody can make an awkward source file disappear by
 * adding one line to a list: this set changes only by changing this code,
 * which a reviewer reads as a decision.
 */
export const NO_E2E_GLOBS = [
  "docs/**",
  "specs/**",
  "**/*.md",
  ".specify/**",
  "marketing/**",
];

/**
 * Whether `path` matches any of `globs`, trying only the globs whose literal
 * head (everything before the first wildcard, brace or extglob) the path
 * starts with. `matchesGlob` compiles each glob on every call, so without this
 * a `which` over a large diff spends seconds comparing an engine file with
 * every web glob; with it, the head rules most globs out by a string compare.
 */
function matchesAnyGlob(path, globs) {
  return globs.some(
    (glob) => path.startsWith(literalHead(glob)) && matchPath(path, [glob]),
  );
}

const HEADS = new Map();

function literalHead(glob) {
  let head = HEADS.get(glob);
  if (head === undefined) {
    head = glob.split(/[*?[{(!@+]/, 1)[0];
    HEADS.set(glob, head);
  }
  return head;
}

/** Where the e2e specs live, from the repository root. */
const SPEC_DIR = "apps/web/e2e/";
/** Journeys end in `.spec.ts` but run on their own instance, never in a slice. */
const JOURNEY_DIR = "apps/web/e2e/journeys/";
const SPEC_SUFFIX = ".spec.ts";

/**
 * Whether a repository path is an e2e spec the runner runs — the same file
 * set `allSpecFiles()` walks, decided by name so that a spec deleted in the
 * diff is still recognised as one.
 */
export function isSpecPath(path) {
  return (
    path.startsWith(SPEC_DIR) &&
    !path.startsWith(JOURNEY_DIR) &&
    path.endsWith(SPEC_SUFFIX)
  );
}

/**
 * A path as the lookup compares it: forward slashes, relative to the
 * repository root, no leading `./`. Callers pass what a person typed.
 */
export function normalisePath(path) {
  let normalised = path.replaceAll("\\", "/");
  while (normalised.startsWith("./")) normalised = normalised.slice(2);
  return normalised;
}

/**
 * The answer for one path. `list` is what `loadSlices` returns:
 * `{ crossCutting, slices }`.
 *
 * Returns `{ path, kind, slices, why }`. For `spec`, `owner` names the owning
 * slice (`null` for an orphan spec, which is `uncovered` instead) and
 * `borrowers` the slices that run it as a neighbour.
 */
export function lookupPath(rawPath, list) {
  const path = normalisePath(rawPath);
  const { crossCutting = [], slices } = list;

  const rule = crossCutting.find(({ glob }) => matchesAnyGlob(path, [glob]));
  if (rule) {
    return { path, kind: "crossCutting", slices: [], why: rule.why };
  }

  if (isSpecPath(path)) {
    const key = specKey(path);
    const owner = ownershipOf(key, slices)?.slice ?? null;
    const borrowers = slices
      .filter((slice) => slice.neighbours.some(({ spec }) => spec === key))
      .map((slice) => slice.name)
      .filter((name) => name !== owner);
    if (owner === null) {
      // An orphan spec is the coverage check's failure, but the lookup can
      // still be asked about it (a new spec, before the list is edited). It
      // is uncovered: nothing would run it.
      return {
        path,
        kind: "uncovered",
        slices: borrowers,
        why: 'a spec that belongs to no slice — add it to a slice\'s "own"',
      };
    }
    return {
      path,
      kind: "spec",
      slices: [owner, ...borrowers.sort()],
      owner,
      borrowers: borrowers.sort(),
      why: "",
    };
  }

  const claimed = slices
    .filter((slice) => matchesAnyGlob(path, slice.paths ?? []))
    .map((slice) => slice.name);
  if (claimed.length > 0) {
    return { path, kind: "slice", slices: claimed, why: "" };
  }

  if (matchesAnyGlob(path, NO_E2E_GLOBS)) {
    return { path, kind: "noE2e", slices: [], why: "no e2e needed" };
  }

  return {
    path,
    kind: "uncovered",
    slices: [],
    why: "no slice or rule matches",
  };
}

/**
 * The answer for a set of paths: each path's answer, then the verdict.
 *
 * `run` is the union of every named slice, each once, in name order so the
 * suggested command is stable. A borrower of an orphan spec is still named:
 * it runs that file, so it is worth running. `fullSuite` is true when any
 * path is cross-cutting — the slices are still listed, because they are the
 * fast first check before the full run.
 */
export function lookupPaths(paths, list) {
  const unique = [...new Set(paths.map(normalisePath))].filter(Boolean);
  const answers = unique.map((path) => lookupPath(path, list));
  const run = [...new Set(answers.flatMap((answer) => answer.slices))].sort();
  return {
    paths: answers,
    run,
    fullSuite: answers.some((answer) => answer.kind === "crossCutting"),
    uncovered: answers
      .filter((answer) => answer.kind === "uncovered")
      .map((answer) => answer.path),
  };
}

/**
 * 3 when anything is uncovered, else 0 (contracts/cli.md). 3 rather than 1 so
 * that a hook or an agent can tell "the lookup has a question for you" from
 * "the lookup broke".
 */
export function exitCodeFor(result) {
  return result.uncovered.length > 0 ? 3 : 0;
}

/** The full-suite command the verdict points at. */
export const FULL_SUITE_COMMAND = "node ./scripts/e2e-parallel.mjs";

/** The human-readable report: one line per path, then the verdict. */
export function formatLookup(result) {
  const lines = [];
  const width = Math.min(
    60,
    Math.max(0, ...result.paths.map((answer) => answer.path.length)),
  );
  for (const answer of result.paths) {
    lines.push(`${answer.path.padEnd(width)}  ${describeAnswer(answer)}`);
  }
  if (result.paths.length > 0) lines.push("");

  if (result.run.length > 0) {
    lines.push(
      `Run: ${result.run.map((name) => `pnpm run e2e:${name}`).join(" && ")}`,
    );
  } else if (!result.fullSuite && result.uncovered.length === 0) {
    lines.push("Run: nothing — no e2e needed");
  }
  if (result.fullSuite) {
    const lead = result.run.length > 0 ? "But" : "Run";
    lines.push(
      `${lead}: a cross-cutting path changed → run the full suite (${FULL_SUITE_COMMAND}) before merging.`,
    );
  }
  if (result.uncovered.length > 0) {
    const count = result.uncovered.length;
    lines.push(
      `Uncovered: ${count} path${count === 1 ? "" : "s"} no slice proves — say how they were tested, or give them to a slice's "paths" in scripts/e2e/slices.json.`,
    );
  }
  return lines.join("\n");
}

function describeAnswer(answer) {
  switch (answer.kind) {
    case "crossCutting":
      return `FULL SUITE — ${answer.why}`;
    case "spec":
      return [
        `${answer.owner} (owner)`,
        ...answer.borrowers.map((name) => `${name} (neighbour)`),
      ].join(" · ");
    case "slice":
      return answer.slices.join(" · ");
    case "noE2e":
      return "no e2e needed";
    default:
      return `UNCOVERED — ${answer.why}`;
  }
}
