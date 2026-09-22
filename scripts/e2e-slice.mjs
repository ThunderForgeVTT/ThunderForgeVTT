#!/usr/bin/env node
/**
 * Ask the slice list questions (spec 060, ADR-107).
 *
 *   node scripts/e2e-slice.mjs which <path>…          # which slice proves these paths
 *   node scripts/e2e-slice.mjs which --diff[=<base>]  # …everything changed since <base>
 *   node scripts/e2e-slice.mjs which --staged         # …everything staged
 *   node scripts/e2e-slice.mjs list [<name>]          # every slice, or one in full
 *
 * Both take `--json`. `pnpm e2e:which` and `pnpm e2e:slices` are these.
 *
 * `which` exits 0 when every path is covered — by a slice, a cross-cutting
 * rule or the no-e2e rule — 3 when something is uncovered (everything else is
 * still printed), and 2 on a usage error, a bad base ref or no repository.
 * The answers themselves come from `scripts/e2e/lookup.mjs`; this file only
 * gathers the paths, which is the one part that needs git.
 */

import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { isAbsolute, join, relative, resolve } from "node:path";
import process from "node:process";

import { ROOT_DIR } from "./shared.mjs";
import { buildRows, formatDetail, formatTable, totals } from "./e2e/list.mjs";
import { exitCodeFor, formatLookup, lookupPaths } from "./e2e/lookup.mjs";
import { SliceError, loadSlices } from "./e2e/slices.mjs";
import { allSpecFiles } from "./e2e/specs.mjs";
import { readSliceDurations } from "./e2e/slice-durations.mjs";

const USAGE = `usage:
  node scripts/e2e-slice.mjs which <path>… [--json]
  node scripts/e2e-slice.mjs which --diff[=<base>] [--json]   (base defaults to origin/main)
  node scripts/e2e-slice.mjs which --staged [--json]
  node scripts/e2e-slice.mjs list [<name>] [--json]`;

/** A usage problem: print it and exit 2, never a stack trace. */
class UsageError extends Error {}

function main(argv) {
  const [command, ...rest] = argv;
  if (command === "which") return which(rest);
  if (command === "list") return list(rest);
  if (command === "--help" || command === "-h") {
    console.log(USAGE);
    return 0;
  }
  throw new UsageError(
    command ? `unknown command "${command}"` : "no command given",
  );
}

function which(args) {
  let json = false;
  let diff = null;
  let staged = false;
  const paths = [];
  for (const arg of args) {
    if (arg === "--json") json = true;
    else if (arg === "--staged") staged = true;
    else if (arg === "--diff") diff = "origin/main";
    else if (arg.startsWith("--diff=")) diff = arg.slice("--diff=".length);
    else if (arg.startsWith("-")) throw new UsageError(`unknown flag "${arg}"`);
    else paths.push(toRepoPath(arg));
  }
  const modes = [diff !== null, staged, paths.length > 0].filter(Boolean);
  if (modes.length === 0) {
    throw new UsageError("give paths, --diff[=<base>] or --staged");
  }
  if (modes.length > 1) {
    throw new UsageError("give only one of: paths, --diff, --staged");
  }
  if (diff === "") throw new UsageError("--diff= needs a base ref");

  const input =
    diff !== null ? diffPaths(diff) : staged ? stagedPaths() : paths;
  const result = lookupPaths(input, loadSlices(ROOT_DIR));
  if (json) {
    console.log(JSON.stringify(result, null, 2));
  } else if (input.length === 0) {
    console.log("No changed paths.");
  } else {
    console.log(formatLookup(result));
  }
  return exitCodeFor(result);
}

/**
 * Everything that differs from `base`: what the branch committed since it
 * forked (`base...HEAD`, so commits that landed on the base since are not
 * mistaken for ours), plus what is uncommitted or untracked. A rename
 * contributes both names — the old path's slices lost a file too.
 */
function diffPaths(base) {
  const committed = git(["diff", "--name-only", "-z", `${base}...HEAD`], {
    what: `base ref "${base}"`,
  });
  return [...splitZ(committed), ...workingTreePaths()];
}

function stagedPaths() {
  return splitZ(git(["diff", "--cached", "--name-only", "-z"]));
}

/**
 * Uncommitted and untracked files from `git status --porcelain -z`. Each
 * record is `XY path`; a rename or copy is followed by one more record, the
 * source path, with no status in front of it.
 */
function workingTreePaths() {
  const records = git(["status", "--porcelain=v1", "-z", "-uall"])
    .split("\0")
    .filter(Boolean);
  const found = [];
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index];
    found.push(record.slice(3));
    if (record[0] === "R" || record[0] === "C") {
      index += 1;
      if (records[index]) found.push(records[index]);
    }
  }
  return found;
}

function git(args, { what = null } = {}) {
  try {
    return execFileSync("git", args, {
      cwd: ROOT_DIR,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 64 * 1024 * 1024,
    });
  } catch (error) {
    const detail = String(error.stderr ?? error.message).trim();
    throw new UsageError(
      what ? `git cannot read ${what}: ${detail}` : `git failed: ${detail}`,
    );
  }
}

function splitZ(text) {
  return text.split("\0").filter(Boolean);
}

/** A path as typed, relative to the repository root. */
function toRepoPath(arg) {
  const absolute = isAbsolute(arg) ? arg : resolve(process.cwd(), arg);
  return relative(ROOT_DIR, absolute).replaceAll("\\", "/");
}

function list(args) {
  let json = false;
  const names = [];
  for (const arg of args) {
    if (arg === "--json") json = true;
    else if (arg.startsWith("-")) throw new UsageError(`unknown flag "${arg}"`);
    else names.push(arg);
  }
  if (names.length > 1) throw new UsageError("list takes at most one slice");

  const { slices } = loadSlices(ROOT_DIR);
  const specFiles = allSpecFiles();
  const context = {
    specFiles,
    measurements: readSliceDurations(ROOT_DIR),
    durations: readSpecDurations(),
  };

  if (names.length === 1) {
    const slice = slices.find((candidate) => candidate.name === names[0]);
    if (!slice) {
      throw new SliceError(
        `unknown slice "${names[0]}". Valid slices: ${slices.map((s) => s.name).join(", ")}`,
      );
    }
    // Rows are built over the whole list even for one slice: ownership is
    // judged against every slice, so a lone slice would claim specs that a
    // more specific entry elsewhere wins.
    const row = buildRows(slices, context).find((r) => r.name === slice.name);
    if (json) console.log(JSON.stringify({ ...slice, ...row }, null, 2));
    else console.log(formatDetail(slice, row, { slices, specFiles }));
    return 0;
  }

  const rows = buildRows(slices, context);
  const sums = totals(rows, specFiles.length);
  if (json)
    console.log(JSON.stringify({ slices: rows, totals: sums }, null, 2));
  else console.log(formatTable(rows, sums));
  return 0;
}

/**
 * Per-spec times for the estimate: this machine's own record if it has one
 * (`.e2e-shards-durations.local.json`, written by `--record-durations`),
 * otherwise the committed one the shard balancer uses. Missing or unreadable
 * files give `{}`, and the listing then marks every estimate incomplete.
 */
function readSpecDurations() {
  for (const name of [
    ".e2e-shards-durations.local.json",
    ".e2e-shards-durations.json",
  ]) {
    const file = join(ROOT_DIR, name);
    if (!existsSync(file)) continue;
    try {
      return JSON.parse(readFileSync(file, "utf8"));
    } catch {
      // A half-written local file is not worth failing a listing over; the
      // committed one is the next best thing.
    }
  }
  return {};
}

try {
  process.exitCode = main(process.argv.slice(2));
} catch (error) {
  if (error instanceof UsageError || error instanceof SliceError) {
    console.error(`e2e-slice: ${error.message}`);
    if (error instanceof UsageError) console.error(USAGE);
    process.exitCode = 2;
  } else {
    throw error;
  }
}
