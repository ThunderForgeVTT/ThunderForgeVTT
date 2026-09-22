/**
 * `scripts/e2e/slice-durations.json`: how long each slice took, measured.
 *
 * Written by `e2e-parallel.mjs --slice=<name> --record-durations` and read by
 * `e2e-slice.mjs list` (spec 060, research R10). One record per slice,
 * overwritten by each recorded run, in the shape
 * `contracts/slices-json.md` fixes.
 *
 * A file of its own, tracked, and separate from `.e2e-shards-durations.json`:
 * the per-spec file rebalances shards and must never ride along with feature
 * work, while this one is a claim ("combat takes 6m 40s") that belongs in the
 * commit that changes it, where a reviewer can see it.
 */

import { execFileSync } from "node:child_process";
import { readFileSync, renameSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { ROOT_DIR } from "../shared.mjs";

/** Where the measurements live, relative to the repository root. */
export const SLICE_DURATIONS_FILE = "scripts/e2e/slice-durations.json";

/**
 * A record's fields, in the order the contract prints them. Written in this
 * order whatever order the caller built the object in, so a re-recorded
 * slice changes only the numbers in a diff, never the layout.
 */
export const RECORD_FIELDS = [
  "wallSeconds",
  "specs",
  "passed",
  "failed",
  "flaky",
  "skipped",
  "measuredAt",
  "commit",
];

/**
 * Every recorded slice, keyed by name. `{}` when nothing has been recorded
 * yet, which is the normal state of a fresh checkout rather than an error.
 * A file that exists but does not parse still throws: a corrupt measurement
 * should be noticed, not read as "not measured".
 */
export function readSliceDurations(root = ROOT_DIR) {
  let text;
  try {
    text = readFileSync(join(root, SLICE_DURATIONS_FILE), "utf-8");
  } catch (error) {
    if (error.code === "ENOENT") return {};
    throw error;
  }
  return JSON.parse(text);
}

/**
 * Put `record` into `file` under `name`, keeping every other slice's record.
 *
 * Slice names sorted, two-space indentation, a trailing newline: the same
 * bytes whichever slice was recorded last, so the diff of a recorded run is
 * that slice's lines and nothing else. Written to a temporary file and
 * renamed, so an interrupted write cannot leave half a file behind.
 */
export function recordSliceDuration(file, name, record) {
  let existing = {};
  try {
    existing = JSON.parse(readFileSync(file, "utf-8"));
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  const all = { ...existing, [name]: record };
  const sorted = {};
  for (const key of Object.keys(all).sort()) {
    const entry = all[key];
    sorted[key] = Object.fromEntries([
      ...RECORD_FIELDS.filter((field) => field in entry).map((field) => [
        field,
        entry[field],
      ]),
      // Anything else a future record carries, after the known fields rather
      // than dropped.
      ...Object.keys(entry)
        .filter((field) => !RECORD_FIELDS.includes(field))
        .sort()
        .map((field) => [field, entry[field]]),
    ]);
  }
  const temporary = `${file}.${process.pid}.tmp`;
  writeFileSync(temporary, `${JSON.stringify(sorted, null, 2)}\n`);
  renameSync(temporary, file);
}

/**
 * The commit a measurement describes: `HEAD`'s short SHA, with `-dirty` when
 * the tree has changes, because a time measured on uncommitted code is a
 * time for code nobody else can check out. `null` outside a git checkout.
 *
 * The two files a recorded run writes are not counted: after the first slice
 * of a sequence they are always changed, and they describe measurements, not
 * the code measured. Counting them marked every slice after the first dirty.
 */
export function measuredCommit(root = ROOT_DIR) {
  const git = (args) =>
    execFileSync("git", args, {
      cwd: root,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "ignore"],
    }).trim();
  try {
    const sha = git(["rev-parse", "--short", "HEAD"]);
    const changes = git([
      "status",
      "--porcelain",
      "--",
      ".",
      `:!${SLICE_DURATIONS_FILE}`,
      ":!.e2e-shards-durations.json",
    ]);
    return changes ? `${sha}-dirty` : sha;
  } catch {
    return null;
  }
}

/** `YYYY-MM-DD` in local time: the day the person who ran it would name. */
export function measuredDate(date = new Date()) {
  const pad = (n) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}
