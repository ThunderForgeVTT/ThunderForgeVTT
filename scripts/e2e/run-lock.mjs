#!/usr/bin/env node
/**
 * `.e2e-running`: the marker that says an e2e run owns this checkout.
 *
 * # What it guards against
 *
 * An e2e run is a set of Vite dev servers watching this tree and backends
 * talking to a Postgres every checkout on the machine shares. Things that
 * look harmless break it from the side:
 *
 * - committing or pushing: the hooks run `pnpm verify`, whose formatters and
 *   generators can write into the tree the shards' Vite servers watch (the
 *   bindings step once rewrote `apps/web/src` and reloaded every page), and a
 *   push runs clippy, which competes with the suite for every core;
 * - `cargo test`: server tests write the same global settings rows the shards'
 *   backends read, in the shared Postgres — from *any* worktree;
 * - a second e2e run: it deletes the shard directory and drops the template
 *   database the first is using.
 *
 * The harness writes this file (pid, start time, arguments) when it starts and
 * removes it on exit and on signals. A file whose pid is dead — a run killed
 * with `kill -9` — is stale and ignored, so nobody has to delete it by hand.
 *
 * # From the command line
 *
 *     node scripts/e2e/run-lock.mjs check                  # this checkout
 *     node scripts/e2e/run-lock.mjs check --all-worktrees  # every worktree
 *
 * exits 1 with an explanation while a live run holds a lock, and 0 otherwise.
 * `THUNDERFORGE_IGNORE_E2E_LOCK=1` turns the refusal into a warning, for the
 * moment someone knows better (the run is theirs and already failed, say).
 */

import { execFileSync } from "node:child_process";
import { readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

export const LOCK_NAME = ".e2e-running";
export const OVERRIDE_ENV = "THUNDERFORGE_IGNORE_E2E_LOCK";

const DEFAULT_ROOT = join(fileURLToPath(import.meta.url), "..", "..", "..");

/**
 * The file an older harness wrote: a bare pid. Still read, so a checkout on a
 * branch from before this file existed is seen as running too.
 */
const LEGACY_LOCK_NAME = ".e2e-shards.lock";

/** This checkout's lock (`name` defaults to the current one), or null. */
export function readLock(root, name = LOCK_NAME) {
  let text;
  try {
    text = readFileSync(join(root, name), "utf-8").trim();
  } catch {
    return null;
  }
  try {
    const parsed = JSON.parse(text);
    if (typeof parsed === "number") return { pid: parsed };
    return parsed && Number.isInteger(parsed.pid) ? parsed : null;
  } catch {
    return null;
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

/** The lock if a live run holds it; null when absent or stale. */
export function liveLock(root) {
  for (const name of [LOCK_NAME, LEGACY_LOCK_NAME]) {
    const lock = readLock(root, name);
    if (lock && isLiveRun(lock.pid)) return lock;
  }
  return null;
}

/** Take the lock, or throw naming the run that holds it. */
export function acquireRunLock(root, args) {
  const held = readLock(root);
  if (held && held.pid !== process.pid) {
    if (isLiveRun(held.pid)) {
      throw new Error(
        `another e2e-parallel run is active in this checkout (${describeLock(held)}). ` +
          "Wait for it to finish.",
      );
    }
    process.stdout.write(
      `[e2e] Ignoring a stale ${LOCK_NAME} from pid ${held.pid}.\n`,
    );
  }
  const lock = {
    pid: process.pid,
    startedAt: new Date().toISOString(),
    args,
    cwd: root,
  };
  writeFileSync(join(root, LOCK_NAME), `${JSON.stringify(lock, null, 2)}\n`);
  return lock;
}

/**
 * Release the lock, but **only if it is still ours**.
 *
 * Teardown outlives the moment a run is judged finished, so a new run can
 * already have written its own pid here; deleting that unconditionally once
 * left the new run unprotected while the old one dropped its databases.
 */
export function releaseRunLock(root) {
  if (readLock(root)?.pid === process.pid) {
    rmSync(join(root, LOCK_NAME), { force: true });
  }
}

export function describeLock(lock) {
  const started = lock.startedAt
    ? `, started ${lock.startedAt} (${Math.round(
        (Date.now() - Date.parse(lock.startedAt)) / 60_000,
      )} min ago)`
    : "";
  const args = lock.args?.length ? `, args: ${lock.args.join(" ")}` : "";
  return `pid ${lock.pid}${started}${args}`;
}

/** Every worktree of the repository `root` belongs to, `root` included. */
function worktreePaths(root) {
  try {
    return execFileSync("git", ["worktree", "list", "--porcelain"], {
      cwd: root,
      encoding: "utf-8",
    })
      .split("\n")
      .filter((line) => line.startsWith("worktree "))
      .map((line) => line.slice("worktree ".length));
  } catch {
    return [root];
  }
}

function check(root, { allWorktrees, purpose }) {
  const here = liveLock(root);
  const self = realpathSync(root);
  const elsewhere = worktreePaths(root)
    .filter((path) => path !== self)
    .map((path) => ({ path, lock: liveLock(path) }))
    .filter(({ lock }) => lock);

  const blocking = [
    ...(here ? [{ path: root, lock: here }] : []),
    ...(allWorktrees ? elsewhere : []),
  ];
  const lines = blocking.map(
    ({ path, lock }) => `  ${path}: ${describeLock(lock)}`,
  );

  if (blocking.length === 0) {
    // Another checkout's run is not this checkout's problem, but it is load
    // on the same machine, and worth one line.
    for (const { path, lock } of elsewhere) {
      process.stderr.write(
        `e2e: note — an e2e run is active in ${path} (pid ${lock.pid}).\n`,
      );
    }
    return 0;
  }

  const message =
    `e2e: an e2e run is in progress — refusing ${purpose}.\n${lines.join("\n")}\n` +
    `  ${purpose[0].toUpperCase()}${purpose.slice(1)} now would break it: ` +
    (allWorktrees
      ? "server tests share the e2e stacks' Postgres and write the same settings rows.\n"
      : "the hooks' `pnpm verify` can write into the tree every shard's Vite is watching, and competes for CPU.\n") +
    `  Wait for the run to finish, or set ${OVERRIDE_ENV}=1 to proceed anyway.\n`;

  if (process.env[OVERRIDE_ENV] === "1") {
    process.stderr.write(
      message.replace("refusing", `${OVERRIDE_ENV}=1, allowing`),
    );
    return 0;
  }
  process.stderr.write(message);
  return 1;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command = "check", ...rest] = process.argv.slice(2);
  if (command !== "check") {
    process.stderr.write(
      "usage: run-lock.mjs check [--all-worktrees] [--purpose=TEXT]\n",
    );
    process.exit(2);
  }
  const purpose =
    rest.find((a) => a.startsWith("--purpose="))?.slice(10) ?? "this";
  process.exit(
    check(DEFAULT_ROOT, {
      allWorktrees: rest.includes("--all-worktrees"),
      purpose,
    }),
  );
}
