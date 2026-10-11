#!/usr/bin/env node
/**
 * The e2e run lock: one run per checkout, and at most N per machine
 * (spec 089).
 *
 * # What it guards against
 *
 * Since spec 089 every checkout tests against its own stack
 * (`scripts/e2e/stack.mjs`) on ports nobody else holds, so two checkouts'
 * runs no longer meet in a database or on a port. What is still shared is the
 * machine: a run holds a few Vite servers, a backend per shard and a browser
 * each, and two of them is about what a workstation takes. So:
 *
 * - **one run per checkout**: `<checkout>/.e2e-running`. Two runs here would
 *   share one stack and drop each other's databases.
 * - **N runs per machine**: slot files outside every checkout,
 *
 *       /tmp/thunderforge-e2e.slot-<n>   (THUNDERFORGE_E2E_LOCK_DIR overrides /tmp)
 *
 *   `THUNDERFORGE_E2E_MAX_CONCURRENT` sets N (default 2).
 *
 * A fixed `/tmp` rather than `$TMPDIR` or `$XDG_RUNTIME_DIR`: those differ
 * between a terminal, a cron job and a sandboxed agent on the same machine, and
 * a slot two runs look for in different places limits nothing.
 *
 * Things that look harmless break a run from the side too: the hooks run
 * `pnpm verify`, whose formatters and generators can write into the tree the
 * shards' Vite servers watch, and a push runs clippy, which competes with the
 * suite for every core. (Not `cargo test`: it has its own database.)
 *
 * # How it behaves
 *
 * The harness takes this checkout's lock first, then holds it while it waits
 * for a slot, so a second run from here waits however many slots are free.
 * Every file is created exclusively and holds the pid, start time, arguments
 * and checkout; all are removed on exit and on signals. A waiting run says
 * which runs it waits on; `--no-wait` makes it fail instead. A file whose pid
 * is dead or no longer an e2e run — a run killed with `kill -9` — is stale: it
 * is replaced, never waited on, and nobody has to delete it by hand.
 *
 * # Older branches
 *
 * A run from a branch before spec 089 holds `/tmp/thunderforge-e2e.lock` and
 * tests against the shared dev Postgres, so while it is live it holds every
 * slot (FR-010). New runs read that file and never write it. Branches older
 * still wrote only `.e2e-running` (or `.e2e-shards.lock`) in their checkout;
 * the hooks read those in every worktree.
 *
 * # From the command line
 *
 *     node scripts/e2e/run-lock.mjs check                  # before a commit
 *     node scripts/e2e/run-lock.mjs check --any-checkout   # before a push
 *
 * exits 1 with an explanation while a live run blocks, and 0 otherwise. A
 * commit is refused during this checkout's run (its `pnpm verify` writes into
 * this tree). A push is refused during this checkout's run and during any
 * pre-089 run, which still uses shared state (FR-011); another checkout's run
 * since 089 touches nothing this checkout owns, and is only a note.
 * `THUNDERFORGE_IGNORE_E2E_LOCK=1` turns the refusal into a warning, for the
 * moment someone knows better (the run is theirs and already failed, say).
 */

import { execFileSync } from "node:child_process";
import {
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";

/** The pre-089 machine-wide lock: read, never written. */
export const LOCK_FILE = "thunderforge-e2e.lock";
export const SLOT_PREFIX = "thunderforge-e2e.slot-";
export const LOCK_DIR_ENV = "THUNDERFORGE_E2E_LOCK_DIR";
export const MAX_ENV = "THUNDERFORGE_E2E_MAX_CONCURRENT";
export const OVERRIDE_ENV = "THUNDERFORGE_IGNORE_E2E_LOCK";
export const DEFAULT_MAX = 2;

/** This checkout's lock. */
export const CHECKOUT_LOCK = ".e2e-running";
/** The in-checkout files any branch may have written: the current, then the oldest. */
export const LEGACY_LOCK_NAMES = [CHECKOUT_LOCK, ".e2e-shards.lock"];

const DEFAULT_ROOT = join(fileURLToPath(import.meta.url), "..", "..", "..");

/** Where the slots (and the pre-089 lock) live. */
export function lockDir(env = process.env) {
  return env[LOCK_DIR_ENV] || (process.platform === "win32" ? tmpdir() : "/tmp");
}

export function slotPath(dir, n) {
  return join(dir, `${SLOT_PREFIX}${n}`);
}

/** The pre-089 machine-wide lock file. */
export function machineLockPath(env = process.env) {
  return join(lockDir(env), LOCK_FILE);
}

/** How many runs the machine takes at once; anything not a positive integer is the default. */
export function maxConcurrent(env = process.env) {
  const text = String(env[MAX_ENV] ?? "");
  const n = Number(text);
  return /^\d+$/.test(text) && n > 0 ? n : DEFAULT_MAX;
}

/** Parse a lock file: JSON with a pid, or an older harness's bare pid. */
function readLockFile(path) {
  let text;
  try {
    text = readFileSync(path, "utf-8").trim();
  } catch {
    return null;
  }
  try {
    const parsed = JSON.parse(text);
    if (Number.isInteger(parsed)) return { pid: parsed, raw: text };
    return parsed && Number.isInteger(parsed.pid)
      ? { ...parsed, raw: text }
      : { pid: null, raw: text };
  } catch {
    return { pid: null, raw: text };
  }
}

/**
 * Is `pid` alive *and still an e2e run*?
 *
 * The second half matters because pids are reused: a lock left by a run that
 * was killed yesterday names a number some unrelated process may hold today.
 * `/proc` settles it on Linux; elsewhere a live pid is taken at its word.
 */
export function isLiveRun(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
  } catch (error) {
    // EPERM: it exists, it just is not ours to signal.
    if (error?.code !== "EPERM") return false;
  }
  try {
    const cmdline = readFileSync(`/proc/${pid}/cmdline`, "utf-8");
    return cmdline.includes("e2e-parallel");
  } catch {
    return true;
  }
}

function canonical(path) {
  try {
    return realpathSync(path);
  } catch {
    return path;
  }
}

function live(lock) {
  return lock && isLiveRun(lock.pid) ? lock : null;
}

/** The pre-089 machine-wide lock if a live run holds it; null when absent or stale. */
export function machineLock({ lockDir: dir = lockDir() } = {}) {
  return live(readLockFile(join(dir, LOCK_FILE)));
}

/** The live lock inside checkout `root`, from this branch or an older one, or null. */
export function checkoutLock(root) {
  for (const name of LEGACY_LOCK_NAMES) {
    const lock = live(readLockFile(join(root, name)));
    if (lock) return { cwd: root, ...lock };
  }
  return null;
}

/** Every live slot, whatever the cap: `{ ...lock, slot }`, lowest first. */
export function liveSlots({ lockDir: dir = lockDir() } = {}) {
  let names;
  try {
    names = readdirSync(dir);
  } catch {
    return [];
  }
  return names
    .filter((name) => name.startsWith(SLOT_PREFIX))
    .map((name) => Number(name.slice(SLOT_PREFIX.length)))
    .filter((n) => Number.isInteger(n) && n >= 0)
    .sort((a, b) => a - b)
    .flatMap((n) => {
      const lock = live(readLockFile(slotPath(dir, n)));
      return lock ? [{ ...lock, slot: n }] : [];
    });
}

/** The lock of a live run started from checkout `root`, or null. */
export function liveLock(root, { lockDir: dir = lockDir() } = {}) {
  const here = canonical(root);
  const isHere = (lock) => canonical(lock.cwd ?? "") === here;
  return (
    checkoutLock(root) ??
    liveSlots({ lockDir: dir }).find(isHere) ??
    [machineLock({ lockDir: dir })].find((lock) => lock && isHere(lock)) ??
    null
  );
}

/** Every worktree of the repository `root` belongs to, `root` included. */
export function worktreePaths(root) {
  try {
    return execFileSync("git", ["worktree", "list", "--porcelain"], {
      cwd: root,
      encoding: "utf-8",
      stdio: ["ignore", "pipe", "ignore"],
    })
      .split("\n")
      .filter((line) => line.startsWith("worktree "))
      .map((line) => line.slice("worktree ".length));
  } catch {
    return [root];
  }
}

/**
 * Create `path` exclusively with `text`. Returns `{ taken: true }`, or
 * `{ holder }` when a live run other than this process holds it. A stale file
 * is removed only if it still holds what was read as stale, then creation is
 * retried; a file this process left behind counts as ours.
 */
function createExclusive(path, text) {
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      writeFileSync(path, text, { flag: "wx" });
      return { taken: true };
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
    }
    const held = readLockFile(path);
    if (!held) continue; // released between the two calls
    if (held.pid === process.pid) return { taken: true };
    if (isLiveRun(held.pid)) return { holder: held };
    if (readLockFile(path)?.raw === held.raw) rmSync(path, { force: true });
  }
  const held = live(readLockFile(path));
  if (held && held.pid !== process.pid) return { holder: held };
  throw new Error(`could not take the e2e lock at ${path}`);
}

/**
 * One attempt at a run. Returns `{ lock }` (with its `slot`) when taken, or
 * `{ holders }`: the runs in the way.
 *
 * Order matters: this checkout's lock first, held while slots are full, so a
 * second run from here can never slip into a slot ahead of the first.
 */
export function tryAcquireRunLock(
  root,
  args = [],
  { lockDir: dir = lockDir(), max = maxConcurrent() } = {},
) {
  const cwd = canonical(root);
  const base = { pid: process.pid, startedAt: new Date().toISOString(), args, cwd };
  const text = (extra = {}) => `${JSON.stringify({ ...base, ...extra }, null, 2)}\n`;

  for (const name of LEGACY_LOCK_NAMES.slice(1)) {
    const older = live(readLockFile(join(cwd, name)));
    if (older && older.pid !== process.pid) return { holders: [{ cwd, ...older }] };
  }
  const own = createExclusive(join(cwd, CHECKOUT_LOCK), text());
  if (own.holder) return { holders: [{ cwd, ...own.holder }] };

  // A pre-089 run tests against the shared dev Postgres: it holds every slot.
  const old = machineLock({ lockDir: dir });
  if (old) return { holders: [old] };

  const taken = liveSlots({ lockDir: dir });
  const others = taken.filter((lock) => lock.pid !== process.pid);
  const mine = taken.find((lock) => lock.pid === process.pid);
  if (mine) return { lock: { ...base, slot: mine.slot } };
  if (others.length >= max) return { holders: others };

  for (let n = 0; n < max; n += 1) {
    if (others.some((lock) => lock.slot === n)) continue;
    const result = createExclusive(slotPath(dir, n), text({ slot: n }));
    if (result.taken) return { lock: { ...base, slot: n } };
  }
  // Another run took the last free slot between the listing and the create.
  return { holders: liveSlots({ lockDir: dir }).filter((l) => l.pid !== process.pid) };
}

/**
 * Take a run, waiting for whichever runs are in the way. `wait: false` throws
 * naming them instead, and gives back this checkout's lock.
 */
export async function acquireRunLock(
  root,
  args = [],
  {
    lockDir: dir = lockDir(),
    max = maxConcurrent(),
    wait = true,
    pollMs = 2_000,
    reportEveryMs = 5 * 60_000,
    log = (line) => process.stdout.write(`[e2e] ${line}\n`),
  } = {},
) {
  let lastReport = 0;
  let lastKey = null;
  for (;;) {
    const result = tryAcquireRunLock(root, args, { lockDir: dir, max });
    if (result.lock) {
      if (lastKey) log("A slot came free; starting.");
      return result.lock;
    }
    const { holders } = result;
    const what = waitingFor(root, holders, max);
    const list = holders.map((lock) => `  ${describeLock(lock)}`).join("\n");
    if (!wait) {
      releaseRunLock({ root, lockDir: dir });
      throw new Error(
        `${what[0].toUpperCase()}${what.slice(1)} is in use:\n${list}\n` +
          "Wait for one to finish, or drop --no-wait to wait automatically.",
      );
    }
    const key = holders.map((lock) => lock.pid).sort().join(",");
    const now = Date.now();
    if (key !== lastKey || now - lastReport >= reportEveryMs) {
      log(
        `${key === lastKey ? "Still waiting" : "Waiting"} for ${what}:\n${list}\n` +
          `Ctrl-C to give up; --no-wait to fail instead of waiting; ${MAX_ENV} ` +
          "sets how many runs the machine takes.",
      );
      lastReport = now;
      lastKey = key;
    }
    await sleep(pollMs);
  }
}

function waitingFor(root, holders, max) {
  const here = canonical(root);
  if (holders.some((lock) => canonical(lock.cwd ?? "") === here)) {
    return "the e2e run already going in this checkout";
  }
  if (holders.length === 1 && holders[0].slot === undefined) {
    return "an e2e slot: a run from before spec 089 holds them all";
  }
  return `an e2e slot (${holders.length} of ${max} held)`;
}

/**
 * Release this run's checkout lock and slot, but **only if they are still
 * ours**.
 *
 * Teardown outlives the moment a run is judged finished, so a new run can
 * already have taken them; deleting that unconditionally once left the new
 * run unprotected while the old one dropped its databases.
 */
export function releaseRunLock({ root = DEFAULT_ROOT, lockDir: dir = lockDir() } = {}) {
  const own = join(canonical(root), CHECKOUT_LOCK);
  if (readLockFile(own)?.pid === process.pid) rmSync(own, { force: true });
  let names = [];
  try {
    names = readdirSync(dir).filter((name) => name.startsWith(SLOT_PREFIX));
  } catch {
    // No directory, no slots.
  }
  for (const name of names) {
    const path = join(dir, name);
    if (readLockFile(path)?.pid === process.pid) rmSync(path, { force: true });
  }
}

export function describeLock(lock) {
  const where = lock.cwd ? `${lock.cwd}, ` : "";
  const started = lock.startedAt
    ? `, started ${lock.startedAt} (${Math.round(
        (Date.now() - Date.parse(lock.startedAt)) / 60_000,
      )} min ago)`
    : "";
  const args = lock.args?.length ? `, args: ${lock.args.join(" ")}` : "";
  return `${where}pid ${lock.pid}${started}${args}`;
}

/**
 * The hooks' question: is a run in the way of `purpose`? Returns the exit code
 * and writes the explanation to `stderr`.
 *
 * A commit (`anyCheckout: false`) is refused during this checkout's run. A
 * push (`anyCheckout: true`) is refused during this checkout's run and during
 * any pre-089 run: a live machine lock, or another worktree's lock whose pid
 * holds no slot. Every other live run is a one-line note.
 */
export function check(
  root,
  {
    anyCheckout = false,
    purpose = "this",
    lockDir: dir = lockDir(),
    worktrees,
    env = process.env,
    stderr = process.stderr,
  } = {},
) {
  const self = canonical(root);
  const isHere = (lock) => canonical(lock.cwd ?? "") === self;
  const slots = liveSlots({ lockDir: dir });
  const old = machineLock({ lockDir: dir });
  const inCheckouts = (worktrees ?? worktreePaths(root))
    .map((path) => checkoutLock(path))
    .filter(Boolean);

  const runs = [];
  for (const lock of [...inCheckouts, ...slots, old].filter(Boolean)) {
    if (!runs.some((run) => run.pid === lock.pid)) runs.push(lock);
  }
  const preSpec089 = (lock) =>
    lock.pid === old?.pid || !slots.some((slot) => slot.pid === lock.pid);
  const blocking = runs.filter(
    (lock) => isHere(lock) || (anyCheckout && preSpec089(lock)),
  );

  if (blocking.length === 0) {
    // Another checkout's run is not this commit's problem, but it is load
    // on the same machine, and worth one line.
    for (const lock of runs) {
      stderr.write(`e2e: note — an e2e run is active (${describeLock(lock)}).\n`);
    }
    return 0;
  }

  const lines = blocking.map((lock) => `  ${describeLock(lock)}`);
  const why = blocking.some(isHere)
    ? "the hooks' `pnpm verify` can write into the tree every shard's Vite is watching, and competes for CPU.\n"
    : "a run from before spec 089 shares the machine's databases, ports and mail containers, and the push's clippy competes with it for every core.\n";
  const message =
    `e2e: an e2e run is in progress — refusing ${purpose}.\n${lines.join("\n")}\n` +
    `  ${purpose[0].toUpperCase()}${purpose.slice(1)} now would break it: ${why}` +
    `  Wait for the run to finish, or set ${OVERRIDE_ENV}=1 to proceed anyway.\n`;

  if (env[OVERRIDE_ENV] === "1") {
    stderr.write(message.replace("refusing", `${OVERRIDE_ENV}=1, allowing`));
    return 0;
  }
  stderr.write(message);
  return 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command = "check", ...rest] = process.argv.slice(2);
  if (command !== "check") {
    process.stderr.write(
      "usage: run-lock.mjs check [--any-checkout] [--purpose=TEXT]\n",
    );
    process.exit(2);
  }
  const purpose =
    rest.find((a) => a.startsWith("--purpose="))?.slice(10) ?? "this";
  process.exit(
    check(DEFAULT_ROOT, {
      // `--all-worktrees` is the flag's name from before the lock was
      // machine-wide; still accepted.
      anyCheckout:
        rest.includes("--any-checkout") || rest.includes("--all-worktrees"),
      purpose,
    }),
  );
}
