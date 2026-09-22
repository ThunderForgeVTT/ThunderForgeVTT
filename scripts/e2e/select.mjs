/**
 * Which spec files one run of `e2e-parallel.mjs` is asked to run.
 *
 * Two ways to ask, and they mean different things on purpose:
 *
 * - `--only=<a,b>` is a substring match anywhere in the path. It is for
 *   triage and for exercising the harness, and its looseness is the point:
 *   `--only=world-cache` names a family without listing it.
 * - `--slice=<name>` is the exact list `slices.json` declares for a feature
 *   (spec 060). It never matches by substring, because that looseness is the
 *   trap a slice exists to avoid: `--only=lighting` also selects
 *   `engine-lighting-limits`, and so pulls a release build into what was
 *   meant to be a quick run.
 *
 * Moved out of the runner (T008) so the difference can be tested without a
 * stack. The runner used this filter in two places — once to decide the
 * engine profile, once to partition the lanes — and both now call here, so
 * they cannot drift apart either.
 */

import { SliceError, loadSlices, resolveSlice } from "./slices.mjs";
import { allSpecFiles } from "./specs.mjs";

/** `--only`'s comma-separated patterns, or `null` when it was not given. */
export function onlyPatterns(only) {
  if (only == null) return null;
  return only
    .split(",")
    .map((pattern) => pattern.trim())
    .filter(Boolean);
}

/**
 * The files `--only=<only>` selects from `files`: every file whose path
 * contains any pattern. No `--only` selects everything.
 *
 * Exactly the filter the runner has always applied, kept byte-for-byte in
 * behaviour (FR-020): a triage habit built on it must not change meaning
 * because slices arrived.
 */
export function filterOnly(files, only) {
  const patterns = onlyPatterns(only);
  return files.filter(
    (file) => !patterns || patterns.some((pattern) => file.includes(pattern)),
  );
}

/**
 * Why these flags cannot run together, or `null` when they can.
 *
 * `--slice` with `--only` would be two answers to "which files", and with
 * `--all` it would move a slice's measured specs into the sharded lane,
 * which is the one thing its recorded time must not depend on. A slice is
 * an `e2e` concept; the playtest suite has no slices.
 */
export function sliceConflict(args) {
  if (!args.slice) return null;
  if (args.only != null) return "--slice cannot be combined with --only";
  if (args.all) return "--slice cannot be combined with --all";
  if (args.suite !== "e2e") {
    return `--slice runs e2e specs; it cannot be combined with --suite=${args.suite}`;
  }
  return null;
}

/**
 * The exact files `--slice=<name>` runs, as `allSpecFiles()` spells them.
 *
 * Throws `SliceError` for an unknown name (listing the valid ones) or a list
 * that does not load; the runner turns that into exit 2.
 */
export function sliceSpecs(name, { root, specFiles } = {}) {
  const { slices } = loadSlices(root);
  return resolveSlice(name, slices, specFiles ?? allSpecFiles());
}

/**
 * The files this run will run, before the lanes are partitioned.
 *
 * `args.sliceSpecs` is the slice's list, resolved once while the arguments
 * were checked; without it, the `--only` filter over the suite.
 */
export function selectedSpecs(args) {
  if (args.sliceSpecs) return args.sliceSpecs;
  return filterOnly(allSpecFiles(args.suite), args.only);
}

export { SliceError };
