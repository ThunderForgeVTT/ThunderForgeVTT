#!/usr/bin/env node
/**
 * The e2e run lock: one e2e run per *machine*, not per checkout.
 *
 * # What it guards against
 *
 * Every checkout on the machine shares what an e2e run uses: the shard
 * databases (`thunderforge_e2e_<shard>`) and their template on one Postgres
 * server, the shards' ports, and the mailpit containers. A lock kept inside
 * each checkout let a run started from a second worktree drop the first
 * worktree's live databases mid-run and fail to start against its mail
 * containers. So the lock lives outside every checkout, at a path named for
 * those shared resources rather than for a repository path:
 *
 *     /tmp/thunderforge-e2e.lock     (THUNDERFORGE_E2E_LOCK_DIR overrides /tmp)
 *
 * A fixed `/tmp` rather than `$TMPDIR` or `$XDG_RUNTIME_DIR`: those differ
 * between a terminal, a cron job and a sandboxed agent on the same machine, and
 * a lock two runs look for in different places locks nothing.
 *
 * Things that look harmless break a run from the side too:
 *
 * - committing or pushing: the hooks run `pnpm verify`, whose formatters and
 *   generators can write into the tree the shards' Vite servers watch (the
 *   bindings step once rewrote `apps/web/src` and reloaded every page), and a
 *   push runs clippy, which competes with the suite for every core;
 * - (not `cargo test`, any more: it has its own database, `thunderforge_test`,
 *   and refuses the development and shard databases — `test_support.rs`).
 *
 * # How it behaves
 *
 * The harness creates the file exclusively (pid, start time, arguments, and the
 * checkout it runs from) when it starts and removes it on exit and on signals.
 * A second run, from any checkout, **waits** for the first and says which
 * checkout and pid it is waiting on; `--no-wait` makes it fail instead. A file
 * whose pid is dead or no longer an e2e run — a run killed with `kill -9` — is
 * stale: it is replaced, never waited on, and nobody has to delete it by hand.
 *
 * A checkout on a branch from before this file still writes its lock inside
 * the checkout (`.e2e-running`, or the older `.e2e-shards.lock`). Those are
 * read too, in every worktree of the repository, so such a run is waited on
 * and seen by the hooks as well.
 *
 * # From the command line
 *
 *     node scripts/e2e/run-lock.mjs check                  # a run from this checkout
 *     node scripts/e2e/run-lock.mjs check --any-checkout   # a run from any checkout
 *
 * exits 1 with an explanation while a live run blocks, and 0 otherwise. The
 * pre-commit hook uses the first (its `pnpm verify` writes into this tree);
 * the pre-push hook the second (its clippy competes with any run).
 * `THUNDERFORGE_IGNORE_E2E_LOCK=1` turns the refusal into a warning, for the
 * moment someone knows better (the run is theirs and already failed, say).
 * It never lets two e2e runs share the machine.
 */

import { execFileSync } from "node:child_process";
import {
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";

export const LOCK_FILE = "thunderforge-e2e.lock";
export const LOCK_DIR_ENV = "THUNDERFORGE_E2E_LOCK_DIR";
export const OVERRIDE_ENV = "THUNDERFORGE_IGNORE_E2E_LOCK";

/** The in-checkout files older branches write: the current, then the oldest. */
export const LEGACY_LOCK_NAMES = [".e2e-running", ".e2e-shards.lock"];

const DEFAULT_ROOT = join(fileURLToPath(import.meta.url), "..", "..", "..");

/** The machine-wide lock file. */
export function machineLockPath(env = process.env) {
  const dir =
    env[LOCK_DIR_ENV] || (process.platform === "win32" ? tmpdir() : "/tmp");
  return join(dir, LOCK_FILE);
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

/** The machine-wide lock if a live run holds it; null when absent or stale. */
export function machineLock({ lockPath = machineLockPath() } = {}) {
  const lock = readLockFile(lockPath);
  return lock && isLiveRun(lock.pid) ? lock : null;
}

/** A live lock an older branch left inside `root`, or null. */
export function legacyLock(root) {
  for (const name of LEGACY_LOCK_NAMES) {
    const lock = readLockFile(join(root, name));
    if (lock && isLiveRun(lock.pid)) return { ...lock, cwd: root };
  }
  return null;
}

/** The lock of a live run started from checkout `root`, or null. */
export function liveLock(root, { lockPath = machineLockPath() } = {}) {
  const lock = machineLock({ lockPath });
  if (lock && canonical(lock.cwd ?? "") === canonical(root)) return lock;
  return legacyLock(root);
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

/** Every live run on the machine that this lock knows of. */
export function liveRuns(
  root,
  { lockPath = machineLockPath(), worktrees = worktreePaths(root) } = {},
) {
  const runs = [];
  const lock = machineLock({ lockPath });
  if (lock) runs.push(lock);
  for (const path of worktrees) {
    const legacy = legacyLock(path);
    if (legacy && legacy.pid !== lock?.pid) runs.push(legacy);
  }
  return runs;
}

/**
 * One attempt at the lock. Returns `{ lock }` when taken, `{ holder }` when a
 * live run holds it.
 *
 * Creation is exclusive (`wx`), so two runs starting together cannot both
 * win. A stale file is removed only if it still holds what was read as stale,
 * then creation is retried.
 */
export function tryAcquireRunLock(
  root,
  args = [],
  { lockPath = machineLockPath(), worktrees } = {},
) {
  const legacy = (worktrees ?? worktreePaths(root))
    .map((path) => legacyLock(path))
    .find((lock) => lock && lock.pid !== process.pid);
  if (legacy) return { holder: legacy };

  const lock = {
    pid: process.pid,
    startedAt: new Date().toISOString(),
    args,
    cwd: canonical(root),
  };
  const text = `${JSON.stringify(lock, null, 2)}\n`;
  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      writeFileSync(lockPath, text, { flag: "wx" });
      return { lock };
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
    }
    const held = readLockFile(lockPath);
    if (!held) continue; // released between the two calls
    if (held.pid !== process.pid && isLiveRun(held.pid)) return { holder: held };
    // Stale (or our own pid from a run that never released): replace it.
    if (readLockFile(lockPath)?.raw === held.raw) {
      rmSync(lockPath, { force: true });
    }
  }
  const held = readLockFile(lockPath);
  if (held && isLiveRun(held.pid)) return { holder: held };
  throw new Error(`could not take the e2e lock at ${lockPath}`);
}

/**
 * Take the lock, waiting for whichever run holds it. `wait: false` throws
 * naming the holder instead.
 */
export async function acquireRunLock(
  root,
  args = [],
  {
    lockPath = machineLockPath(),
    wait = true,
    pollMs = 2_000,
    reportEveryMs = 5 * 60_000,
    log = (line) => process.stdout.write(`[e2e] ${line}\n`),
    worktrees,
  } = {},
) {
  let lastReport = 0;
  let lastHolder = null;
  for (;;) {
    const result = tryAcquireRunLock(root, args, { lockPath, worktrees });
    if (result.lock) {
      if (lastHolder) log("The other e2e run finished; starting.");
      return result.lock;
    }
    const { holder } = result;
    if (!wait) {
      throw new Error(
        `another e2e run is active on this machine (${describeLock(holder)}). ` +
          "Every checkout shares its databases, ports and mail containers; " +
          "wait for it to finish, or drop --no-wait to wait automatically.",
      );
    }
    const now = Date.now();
    if (holder.pid !== lastHolder?.pid || now - lastReport >= reportEveryMs) {
      log(
        `${lastHolder?.pid === holder.pid ? "Still waiting" : "Waiting"} for ` +
          `the e2e run already on this machine (${describeLock(holder)}): ` +
          "every checkout shares the thunderforge_e2e_* databases, the ports " +
          "and the mail containers. Ctrl-C to give up; --no-wait to fail " +
          "instead of waiting.",
      );
      lastReport = now;
      lastHolder = holder;
    }
    await sleep(pollMs);
  }
}

/**
 * Release the lock, but **only if it is still ours**.
 *
 * Teardown outlives the moment a run is judged finished, so a new run can
 * already have taken the lock; deleting that unconditionally once left the new
 * run unprotected while the old one dropped its databases.
 */
export function releaseRunLock({ lockPath = machineLockPath() } = {}) {
  if (readLockFile(lockPath)?.pid === process.pid) {
    rmSync(lockPath, { force: true });
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
 */
export function check(
  root,
  {
    anyCheckout = false,
    purpose = "this",
    lockPath = machineLockPath(),
    worktrees,
    env = process.env,
    stderr = process.stderr,
  } = {},
) {
  const self = canonical(root);
  const runs = liveRuns(root, { lockPath, worktrees });
  const isHere = (lock) => canonical(lock.cwd ?? "") === self;
  const blocking = runs.filter((lock) => anyCheckout || isHere(lock));

  if (blocking.length === 0) {
    // Another checkout's run is not this commit's problem, but it is load
    // on the same machine, and worth one line.
    for (const lock of runs) {
      stderr.write(`e2e: note — an e2e run is active (${describeLock(lock)}).\n`);
    }
    return 0;
  }

  const lines = blocking.map((lock) => `  ${describeLock(lock)}`);
  const message =
    `e2e: an e2e run is in progress — refusing ${purpose}.\n${lines.join("\n")}\n` +
    `  ${purpose[0].toUpperCase()}${purpose.slice(1)} now would break it: ` +
    (anyCheckout
      ? "a run from any checkout shares this machine's Postgres server and its cores.\n"
      : "the hooks' `pnpm verify` can write into the tree every shard's Vite is watching, and competes for CPU.\n") +
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
