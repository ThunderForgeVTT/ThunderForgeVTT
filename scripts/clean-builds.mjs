#!/usr/bin/env node
/**
 * Trim build output that only costs disk: old incremental sessions, stale
 * cargo artifacts, and finished agent worktrees.
 *
 * # Why this exists
 *
 * Build output reached more than a terabyte on the development machine. The
 * main checkout's `target/` was 730 GiB — `debug/incremental` 416 GB across
 * 1,913 sessions, `debug/deps` 217 GB — and finished worktrees under
 * `.claude/worktrees/` held about 517 GB more. `cargo clean` is the only tool
 * cargo ships for it, and it throws away the warm cache along with the waste.
 *
 * # Dry run by default
 *
 *     node scripts/clean-builds.mjs                # print the plan, delete nothing
 *     node scripts/clean-builds.mjs --apply        # delete what the plan names
 *     node scripts/clean-builds.mjs --root=<path>  # another checkout
 *     node scripts/clean-builds.mjs --hint         # one line if a trim is due, for other scripts
 *
 * Options: `--incremental-days=3`, `--deps-days=7`, `--skip-worktrees`,
 * `--skip-target`.
 *
 * # What it deletes, and why each is only ever a rebuild
 *
 * 1. **Incremental sessions** (`target/<profile>/incremental/<crate>/s-*`, and
 *    the same under `target/<triple>/`). rustc reads only the newest finished
 *    session of a crate directory; the others are history. Each crate directory
 *    keeps its newest session and any session touched in the last
 *    `--incremental-days`. A deleted session means rustc compiles that crate
 *    without incremental reuse once.
 *
 * 2. **Cargo units** (`deps/`, `build/`, `.fingerprint/`, and hashed files in
 *    the profile directory). This is what `cargo-sweep --time` does, without
 *    installing it: every file cargo makes for one unit carries the unit's
 *    16-hex-digit metadata hash in its name (`libserde-1e974e170a81bde0.rlib`,
 *    `.fingerprint/serde-1e974e170a81bde0/`, `build/serde-4edf…/`). The unit's
 *    last use is the newest timestamp among its `.fingerprint` directory's
 *    files, and a unit older than `--deps-days` goes as a whole: every file and
 *    directory with that hash. Its fingerprint goes **first**, so a trim that
 *    stops halfway leaves a unit cargo sees as never built, not one it
 *    believes is fresh with its outputs missing.
 *
 *    cargo-sweep reads the access time. This machine's filesystem is mounted
 *    `noatime`, where an access time is only ever the creation time, so this
 *    takes the later of access and modification. Either way the consequence is
 *    the same and is only a cost: a unit that has not been *rebuilt* in a week
 *    but is still in use is deleted and rebuilt once.
 *
 * 3. **Worktrees** under `.claude/worktrees/` that are finished: registered
 *    with git, clean (no modified or untracked files — ignored build output
 *    does not count), HEAD an ancestor of `main`, not locked, no live e2e run,
 *    and no running process with its working directory inside. Removed with
 *    `git worktree remove` (never `--force`) and their branch with
 *    `git branch -d` (never `-D`), so git refuses anything unmerged too. Every
 *    worktree kept is printed with every reason it was kept.
 *
 * # What it never touches
 *
 * Anything outside `<root>/target/` and `<root>/.claude/worktrees/`: no
 * `node_modules`, no `dist/`, no databases. Each deletion re-checks its path
 * against those two roots before it happens.
 *
 * # When it refuses
 *
 * While a live e2e run holds `.e2e-running` in the root (the run's backends
 * and engine build read `target/`), and while a cargo, rustc or clippy
 * process is working in the root. Deleting under a running compiler is the
 * one way this could produce a wrong build rather than a slow one.
 */

import { execFileSync } from "node:child_process";
import {
  existsSync,
  lstatSync,
  readdirSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  rmSync,
} from "node:fs";
import { join, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { describeLock, liveLock } from "./e2e/run-lock.mjs";

const DEFAULT_ROOT = resolve(fileURLToPath(import.meta.url), "..", "..");
const DAY_MS = 24 * 60 * 60 * 1000;

/** `--hint` thresholds: well above a warm tree, well below the terabyte. */
export const HINT_SESSION_LIMIT = 600;
export const HINT_DEPS_ENTRY_LIMIT = 20_000;

// --- helpers ---------------------------------------------------------------

function listDir(path) {
  try {
    return readdirSync(path, { withFileTypes: true });
  } catch {
    return [];
  }
}

function isInside(child, parent) {
  const rel = relative(parent, child);
  return rel !== "" && !rel.startsWith("..") && !rel.startsWith(sep) && rel !== "..";
}

export function formatBytes(bytes) {
  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(unit === 0 ? 0 : 1)} ${units[unit]}`;
}

/**
 * Disk used by a file or tree, counting each inode once (the profile
 * directory's binaries are hard links into `deps/`).
 */
function diskUsage(path, seen = new Set()) {
  let stat;
  try {
    stat = lstatSync(path, { bigint: false });
  } catch {
    return 0;
  }
  const key = `${stat.dev}:${stat.ino}`;
  if (seen.has(key)) return 0;
  seen.add(key);
  let total = stat.blocks * 512;
  if (stat.isDirectory()) {
    for (const entry of listDir(path)) total += diskUsage(join(path, entry.name), seen);
  }
  return total;
}

/** `du`, for the before-and-after totals: faster than walking in node. */
function duBytes(path) {
  if (!existsSync(path)) return 0;
  try {
    const out = execFileSync("du", ["-s", "-B1", path], {
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    return Number(out.split(/\s/)[0]) || 0;
  } catch (error) {
    // `du` exits 1 when a file vanished mid-walk, and still prints a total.
    const out = String(error.stdout ?? "");
    return Number(out.split(/\s/)[0]) || 0;
  }
}

function lastTouched(path) {
  try {
    const stat = lstatSync(path);
    return Math.max(stat.mtimeMs, stat.atimeMs);
  } catch {
    return 0;
  }
}

function git(cwd, args) {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf-8",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

// --- profiles --------------------------------------------------------------

/** `target/debug`, `target/release`, `target/wasm32-unknown-unknown/debug`, ... */
export function profileDirs(targetDir) {
  const profiles = [];
  const looksLikeProfile = (path) =>
    existsSync(join(path, ".fingerprint")) ||
    existsSync(join(path, "deps")) ||
    existsSync(join(path, "incremental"));
  for (const entry of listDir(targetDir)) {
    if (!entry.isDirectory()) continue;
    const path = join(targetDir, entry.name);
    if (looksLikeProfile(path)) {
      profiles.push(path);
      continue;
    }
    for (const inner of listDir(path)) {
      if (inner.isDirectory() && looksLikeProfile(join(path, inner.name))) {
        profiles.push(join(path, inner.name));
      }
    }
  }
  return profiles;
}

// --- 1. incremental ----------------------------------------------------------

/** `s-<time>-<random>-<svh|working>` -> `s-<time>-<random>` (the lock's stem). */
function sessionStem(name) {
  return name.split("-").slice(0, 3).join("-");
}

export function planIncremental(targetDir, { days, now = Date.now() }) {
  const cutoff = now - days * DAY_MS;
  const doomed = [];
  let sessions = 0;
  for (const profile of profileDirs(targetDir)) {
    const incremental = join(profile, "incremental");
    for (const crate of listDir(incremental)) {
      if (!crate.isDirectory()) continue;
      const crateDir = join(incremental, crate.name);
      const entries = listDir(crateDir);
      const dirs = entries
        .filter((entry) => entry.isDirectory() && entry.name.startsWith("s-"))
        .map((entry) => ({
          name: entry.name,
          path: join(crateDir, entry.name),
          touched: lastTouched(join(crateDir, entry.name)),
        }))
        .sort((a, b) => b.touched - a.touched || b.name.localeCompare(a.name));
      sessions += dirs.length;
      const locks = new Set(
        entries.filter((e) => e.isFile() && e.name.endsWith(".lock")).map((e) => e.name),
      );
      dirs.forEach((session, index) => {
        if (index === 0 || session.touched >= cutoff) return;
        const paths = [session.path];
        const lock = `${sessionStem(session.name)}.lock`;
        if (locks.has(lock)) paths.push(join(crateDir, lock));
        doomed.push({ label: relative(targetDir, session.path), paths });
      });
    }
  }
  return { sessions, doomed };
}

// --- 2. cargo units ----------------------------------------------------------

/** The unit hash in a cargo-made name, the way cargo-sweep reads it. */
export function unitHash(name) {
  const stem = name.split(".")[0];
  const dash = stem.lastIndexOf("-");
  if (dash < 0) return null;
  const hash = stem.slice(dash + 1);
  return /^[0-9a-f]{16}$/.test(hash) ? hash : null;
}

export function planUnits(targetDir, { days, now = Date.now() }) {
  const cutoff = now - days * DAY_MS;
  const doomed = [];
  let units = 0;
  for (const profile of profileDirs(targetDir)) {
    const fingerprints = join(profile, ".fingerprint");
    const stale = new Map();
    for (const entry of listDir(fingerprints)) {
      if (!entry.isDirectory()) continue;
      const hash = unitHash(entry.name);
      if (!hash) continue;
      units += 1;
      const dir = join(fingerprints, entry.name);
      let used = lastTouched(dir);
      for (const file of listDir(dir)) used = Math.max(used, lastTouched(join(dir, file.name)));
      if (used < cutoff) stale.set(hash, { name: entry.name, fingerprint: dir });
    }
    if (stale.size === 0) continue;
    // Fingerprint first (see the module comment), then everything else the
    // unit made, wherever cargo put it.
    const byHash = new Map([...stale].map(([hash, unit]) => [hash, [unit.fingerprint]]));
    for (const where of [join(profile, "deps"), join(profile, "build"), profile]) {
      for (const entry of listDir(where)) {
        const hash = unitHash(entry.name);
        if (hash && byHash.has(hash)) byHash.get(hash).push(join(where, entry.name));
      }
    }
    for (const [hash, paths] of byHash) {
      doomed.push({ label: relative(targetDir, join(profile, stale.get(hash).name)), paths });
    }
  }
  return { units, doomed };
}

// --- 3. worktrees ------------------------------------------------------------

/** pid -> realpath of cwd, for every process we can see. */
function processCwds() {
  const cwds = [];
  for (const entry of listDir("/proc")) {
    if (!/^\d+$/.test(entry.name)) continue;
    try {
      cwds.push({ pid: Number(entry.name), cwd: readlinkSync(`/proc/${entry.name}/cwd`) });
    } catch {
      // Gone, or not ours to read.
    }
  }
  return cwds;
}

function processName(pid) {
  try {
    return readFileSync(`/proc/${pid}/comm`, "utf-8").trim();
  } catch {
    return "?";
  }
}

function registeredWorktrees(root) {
  const text = git(root, ["worktree", "list", "--porcelain"]);
  const worktrees = [];
  let current = null;
  for (const line of text.split("\n")) {
    if (line.startsWith("worktree ")) {
      current = { path: line.slice(9), locked: false, branch: null, head: null };
      worktrees.push(current);
    } else if (!current) {
      continue;
    } else if (line.startsWith("HEAD ")) current.head = line.slice(5);
    else if (line.startsWith("branch ")) current.branch = line.slice(7).replace(/^refs\/heads\//, "");
    else if (line === "locked" || line.startsWith("locked ")) current.locked = true;
    else if (line === "detached") current.branch = null;
  }
  return worktrees;
}

export function planWorktrees(root, { mainRef = "main" } = {}) {
  const dir = join(root, ".claude", "worktrees");
  const keep = [];
  const remove = [];
  if (!existsSync(dir)) return { keep, remove };
  const registered = new Map(
    registeredWorktrees(root).map((wt) => {
      let real = wt.path;
      try {
        real = realpathSync(wt.path);
      } catch {
        // prunable: listed but gone
      }
      return [real, wt];
    }),
  );
  const cwds = processCwds().filter(({ pid }) => pid !== process.pid);
  let mainExists = true;
  try {
    git(root, ["rev-parse", "--verify", "--quiet", `${mainRef}^{commit}`]);
  } catch {
    mainExists = false;
  }

  for (const entry of listDir(dir)) {
    if (!entry.isDirectory()) continue;
    const path = realpathSync(join(dir, entry.name));
    const reasons = [];
    const wt = registered.get(path);
    if (!wt) {
      keep.push({ path, reasons: ["not a registered git worktree (left alone, not guessed at)"] });
      continue;
    }
    if (wt.locked) reasons.push("locked");
    const users = cwds.filter(({ cwd }) => cwd === path || isInside(cwd, path));
    if (users.length > 0) {
      const named = users.slice(0, 5).map(({ pid }) => `${processName(pid)}[${pid}]`);
      reasons.push(`in use: ${users.length} process(es) have their cwd inside (${named.join(", ")}${users.length > 5 ? ", …" : ""})`);
    }
    const lock = liveLock(path);
    if (lock) reasons.push(`a live e2e run holds it (${describeLock(lock)})`);
    try {
      const status = git(path, ["status", "--porcelain", "--untracked-files=normal"]);
      const changed = status.split("\n").filter(Boolean);
      if (changed.length > 0) reasons.push(`not clean: ${changed.length} modified or untracked path(s)`);
    } catch (error) {
      reasons.push(`git status failed: ${String(error.stderr ?? error.message).trim()}`);
    }
    if (!mainExists) {
      reasons.push(`no \`${mainRef}\` to compare with`);
    } else if (!wt.head) {
      reasons.push("no HEAD");
    } else {
      try {
        git(root, ["merge-base", "--is-ancestor", wt.head, mainRef]);
      } catch {
        reasons.push(`HEAD ${wt.head.slice(0, 7)} is not an ancestor of ${mainRef}`);
      }
    }
    (reasons.length === 0 ? remove : keep).push({ path, branch: wt.branch, reasons });
  }
  return { keep, remove };
}

// --- refusals ----------------------------------------------------------------

const COMPILERS = new Set(["cargo", "rustc", "clippy-driver", "cargo-clippy", "rustdoc"]);

/** Compilers working in `root` — but not in a worktree under it, which has its own `target/`. */
export function compilersIn(root) {
  const worktrees = join(root, ".claude", "worktrees");
  const target = join(root, "target");
  const found = [];
  for (const { pid, cwd } of processCwds()) {
    const name = processName(pid);
    if (!COMPILERS.has(name)) continue;
    let targetDir = null;
    try {
      const environ = readFileSync(`/proc/${pid}/environ`, "utf-8").split("\0");
      const value = environ.find((kv) => kv.startsWith("CARGO_TARGET_DIR="));
      if (value) targetDir = resolve(cwd, value.slice("CARGO_TARGET_DIR=".length));
    } catch {
      // unreadable: judge by cwd alone
    }
    const inRoot = (cwd === root || isInside(cwd, root)) && !isInside(cwd, worktrees);
    const usesTarget = targetDir && (targetDir === target || isInside(targetDir, target));
    if (inRoot || usesTarget) found.push({ pid, name, cwd });
  }
  return found;
}

// --- hint --------------------------------------------------------------------

/**
 * One line when a trim is due, or null. Counts directory entries only — no
 * sizes, no walk — so the e2e pre-flight and `make dev` can afford it.
 */
export function buildOutputHint(
  root = DEFAULT_ROOT,
  { sessionLimit = HINT_SESSION_LIMIT, depsLimit = HINT_DEPS_ENTRY_LIMIT } = {},
) {
  const target = join(root, "target");
  let sessions = 0;
  let deps = 0;
  for (const profile of profileDirs(target)) {
    for (const crate of listDir(join(profile, "incremental"))) {
      if (!crate.isDirectory()) continue;
      for (const entry of listDir(join(profile, "incremental", crate.name))) {
        if (entry.name.startsWith("s-") && entry.isDirectory()) sessions += 1;
      }
    }
    deps += listDir(join(profile, "deps")).length;
  }
  if (sessions < sessionLimit && deps < depsLimit) return null;
  return (
    `build output is piling up (${sessions} incremental sessions, ${deps} files in deps/) — ` +
    "`make clean-builds` shows what can go, `make clean-builds ARGS=--apply` trims it."
  );
}

// --- main --------------------------------------------------------------------

function parseArgs(argv) {
  const args = {
    apply: false,
    hint: false,
    root: DEFAULT_ROOT,
    incrementalDays: 3,
    depsDays: 7,
    target: true,
    worktrees: true,
    verbose: false,
  };
  for (const arg of argv) {
    let match;
    if (arg === "--apply") args.apply = true;
    else if (arg === "--dry-run") args.apply = false;
    else if (arg === "--hint") args.hint = true;
    else if (arg === "--verbose") args.verbose = true;
    else if (arg === "--skip-target") args.target = false;
    else if (arg === "--skip-worktrees") args.worktrees = false;
    else if ((match = /^--root=(.+)$/.exec(arg))) args.root = resolve(match[1]);
    else if ((match = /^--incremental-days=(\d+(?:\.\d+)?)$/.exec(arg))) args.incrementalDays = Number(match[1]);
    else if ((match = /^--deps-days=(\d+(?:\.\d+)?)$/.exec(arg))) args.depsDays = Number(match[1]);
    else throw new Error(`Unknown argument: ${arg} (see the comment at the top of scripts/clean-builds.mjs)`);
  }
  args.root = realpathSync(args.root);
  return args;
}

/** Build output only: inside `target/`, and never a `node_modules` or `dist`. */
function guardedRemove(path, target) {
  const real = resolve(path);
  if (!isInside(real, target)) {
    throw new Error(`refusing to delete ${real}: outside ${target}`);
  }
  const segments = relative(target, real).split(sep);
  if (segments.includes("node_modules") || segments.includes("dist")) {
    throw new Error(`refusing to delete ${real}: node_modules or dist`);
  }
  rmSync(real, { recursive: true, force: true });
}

function printPlan(title, doomed, { verbose, limit = 8 }) {
  const seen = new Set();
  const bytes = doomed.reduce(
    (sum, item) => sum + item.paths.reduce((s, p) => s + diskUsage(p, seen), 0),
    0,
  );
  console.log(`  ${title}: ${doomed.length} to delete, ${formatBytes(bytes)}`);
  const shown = verbose ? doomed : doomed.slice(0, limit);
  for (const item of shown) console.log(`    - ${item.label}`);
  if (shown.length < doomed.length) console.log(`    … ${doomed.length - shown.length} more (--verbose lists them)`);
  return bytes;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.hint) {
    const hint = buildOutputHint(args.root);
    if (hint) console.log(`note: ${hint}`);
    return 0;
  }

  const { root } = args;
  const target = join(root, "target");
  const worktreesDir = join(root, ".claude", "worktrees");
  console.log(`clean-builds: ${root} (${args.apply ? "APPLY" : "dry run — nothing is deleted; --apply deletes"})`);

  const lock = liveLock(root);
  if (lock) {
    console.error(`clean-builds: refusing — an e2e run is live here (${describeLock(lock)}).`);
    return 2;
  }
  const compilers = args.target ? compilersIn(root) : [];
  if (compilers.length > 0) {
    console.error("clean-builds: refusing — a compiler is working in this checkout:");
    for (const c of compilers) console.error(`  ${c.name}[${c.pid}] in ${c.cwd}`);
    return 2;
  }

  const started = performance.now();
  const before = {
    target: args.target ? duBytes(target) : 0,
    worktrees: args.worktrees ? duBytes(worktreesDir) : 0,
  };
  console.log(`\nbefore: target/ ${formatBytes(before.target)}, .claude/worktrees/ ${formatBytes(before.worktrees)}`);

  const incremental = args.target ? planIncremental(target, { days: args.incrementalDays }) : { sessions: 0, doomed: [] };
  const units = args.target ? planUnits(target, { days: args.depsDays }) : { units: 0, doomed: [] };
  const worktrees = args.worktrees ? planWorktrees(root) : { keep: [], remove: [] };

  console.log("\nplan:");
  if (args.target) {
    console.log(`  (${incremental.sessions} incremental sessions; keeping each crate's newest and any from the last ${args.incrementalDays} day(s))`);
    printPlan("incremental sessions", incremental.doomed, args);
    console.log(`  (${units.units} cargo units; deleting those unused for ${args.depsDays} day(s))`);
    printPlan("cargo units", units.doomed, args);
  }
  if (args.worktrees) {
    console.log(`  worktrees: ${worktrees.remove.length} to remove, ${worktrees.keep.length} kept`);
    for (const wt of worktrees.remove) console.log(`    - remove ${relative(root, wt.path)}${wt.branch ? ` (branch ${wt.branch})` : ""}`);
    for (const wt of worktrees.keep) {
      console.log(`    = keep   ${relative(root, wt.path)}`);
      for (const reason of wt.reasons) console.log(`        because ${reason}`);
    }
  }

  if (!args.apply) {
    console.log(`\ndry run: nothing deleted (${Math.round(performance.now() - started)} ms). Re-run with --apply.`);
    return 0;
  }

  const failures = [];
  for (const item of [...incremental.doomed, ...units.doomed]) {
    for (const path of item.paths) {
      try {
        guardedRemove(path, target);
      } catch (error) {
        failures.push(`${path}: ${error.message}`);
      }
    }
  }
  for (const wt of worktrees.remove) {
    try {
      if (!isInside(wt.path, worktreesDir)) throw new Error("outside .claude/worktrees");
      git(root, ["worktree", "remove", wt.path]);
      console.log(`removed worktree ${relative(root, wt.path)}`);
    } catch (error) {
      failures.push(`worktree ${wt.path}: ${String(error.stderr ?? error.message).trim()}`);
      continue;
    }
    if (wt.branch) {
      try {
        git(root, ["branch", "-d", wt.branch]);
        console.log(`deleted merged branch ${wt.branch}`);
      } catch (error) {
        console.log(`kept branch ${wt.branch}: ${String(error.stderr ?? error.message).trim()}`);
      }
    }
  }

  const after = {
    target: args.target ? duBytes(target) : 0,
    worktrees: args.worktrees ? duBytes(worktreesDir) : 0,
  };
  console.log(`\nafter:  target/ ${formatBytes(after.target)}, .claude/worktrees/ ${formatBytes(after.worktrees)}`);
  console.log(
    `freed:  ${formatBytes(before.target - after.target + before.worktrees - after.worktrees)} in ${Math.round(performance.now() - started)} ms`,
  );
  if (failures.length > 0) {
    console.error(`\n${failures.length} deletion(s) failed:`);
    for (const failure of failures) console.error(`  ${failure}`);
    return 1;
  }
  return 0;
}

if (process.argv[1] && import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  process.exit(main());
}
