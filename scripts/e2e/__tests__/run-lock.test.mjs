/**
 * Unit tests for `scripts/e2e/run-lock.mjs`: one e2e run per machine.
 *
 * Every test uses a lock file in its own temporary directory, never the real
 * machine-wide one, so they are safe beside a live e2e run. A live holder is a
 * real process whose command line says `e2e-parallel` — what `isLiveRun`
 * checks — so waiting is proven against a process, not a mock.
 */

import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, test } from "node:test";

import {
  LOCK_FILE,
  OVERRIDE_ENV,
  acquireRunLock,
  check,
  isLiveRun,
  liveLock,
  machineLockPath,
  releaseRunLock,
  tryAcquireRunLock,
} from "../run-lock.mjs";

let dir;
let lockPath;
let checkoutA;
let checkoutB;
const holders = [];

/** A live process that `isLiveRun` takes for an e2e run. */
function fakeRun() {
  const child = spawn(
    process.execPath,
    ["-e", "setInterval(() => {}, 1000)", "fake-e2e-parallel-holder"],
    { stdio: "ignore" },
  );
  holders.push(child);
  return child;
}

/** A pid that has certainly exited. */
function deadPid() {
  return spawnSync(process.execPath, ["-e", "process.stdout.write(String(process.pid))"], {
    encoding: "utf-8",
  }).stdout.trim() * 1;
}

function writeLock(pid, cwd, path = lockPath) {
  writeFileSync(
    path,
    JSON.stringify({ pid, cwd, startedAt: new Date().toISOString(), args: [] }),
  );
}

async function untilLive(pid) {
  for (let i = 0; i < 100 && !isLiveRun(pid); i += 1) {
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.ok(isLiveRun(pid), "the fake holder never came up");
}

/** A stderr that remembers what was written. */
function sink() {
  const out = { text: "", write: (s) => (out.text += s) };
  return out;
}

beforeEach(() => {
  dir = realpathSync(mkdtempSync(join(tmpdir(), "run-lock-test-")));
  lockPath = join(dir, LOCK_FILE);
  checkoutA = join(dir, "checkout-a");
  checkoutB = join(dir, "checkout-b");
  mkdirSync(checkoutA);
  mkdirSync(checkoutB);
});

afterEach(() => {
  for (const child of holders.splice(0)) child.kill("SIGKILL");
  rmSync(dir, { recursive: true, force: true });
});

describe("where the lock lives", () => {
  test("outside any checkout, the same for every checkout", () => {
    assert.equal(machineLockPath({}), join("/tmp", LOCK_FILE));
    assert.equal(
      machineLockPath({ THUNDERFORGE_E2E_LOCK_DIR: "/x" }),
      join("/x", LOCK_FILE),
    );
  });
});

describe("taking and releasing it", () => {
  test("a free lock is taken and names this process and checkout", () => {
    const { lock } = tryAcquireRunLock(checkoutA, ["--slice=x"], {
      lockPath,
      worktrees: [],
    });
    assert.equal(lock.pid, process.pid);
    const onDisk = JSON.parse(readFileSync(lockPath, "utf-8"));
    assert.equal(onDisk.pid, process.pid);
    assert.equal(onDisk.cwd, checkoutA);
    assert.deepEqual(onDisk.args, ["--slice=x"]);
    releaseRunLock({ lockPath });
    assert.equal(existsSync(lockPath), false);
  });

  test("release leaves another run's lock alone", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB);
    releaseRunLock({ lockPath });
    assert.equal(JSON.parse(readFileSync(lockPath, "utf-8")).pid, holder.pid);
  });

  test("a lock whose pid is dead does not block", () => {
    writeLock(deadPid(), checkoutB);
    const result = tryAcquireRunLock(checkoutA, [], { lockPath, worktrees: [] });
    assert.equal(result.lock?.pid, process.pid);
  });

  test("a lock whose pid now belongs to something else does not block", () => {
    // This test process is alive but is not an e2e run: a reused pid.
    writeLock(process.ppid, checkoutB);
    const result = tryAcquireRunLock(checkoutA, [], { lockPath, worktrees: [] });
    assert.equal(result.lock?.pid, process.pid);
  });

  test("an unreadable lock does not block", () => {
    writeFileSync(lockPath, "not json");
    const result = tryAcquireRunLock(checkoutA, [], { lockPath, worktrees: [] });
    assert.equal(result.lock?.pid, process.pid);
  });
});

describe("a run from another checkout", () => {
  test("holds the lock against this one", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB);
    const result = tryAcquireRunLock(checkoutA, [], { lockPath, worktrees: [] });
    assert.equal(result.holder?.pid, holder.pid);
    assert.equal(result.holder?.cwd, checkoutB);
  });

  test("with --no-wait, fails naming the checkout and pid", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB);
    await assert.rejects(
      acquireRunLock(checkoutA, [], { lockPath, wait: false, worktrees: [] }),
      (error) =>
        error.message.includes(checkoutB) &&
        error.message.includes(`pid ${holder.pid}`),
    );
  });

  test("is waited for by default, and the lock is taken once it exits", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB);
    const lines = [];
    let exitedAt = null;
    setTimeout(() => {
      exitedAt = Date.now();
      holder.kill("SIGKILL"); // like `kill -9`: the lock file stays behind
    }, 300);
    const lock = await acquireRunLock(checkoutA, [], {
      lockPath,
      pollMs: 25,
      log: (line) => lines.push(line),
      worktrees: [],
    });
    assert.ok(exitedAt !== null, "took the lock while the holder was alive");
    assert.equal(lock.pid, process.pid);
    assert.match(lines[0], /^Waiting for the e2e run/);
    assert.ok(lines[0].includes(checkoutB) && lines[0].includes(`pid ${holder.pid}`));
    assert.equal(lines.filter((l) => l.startsWith("Waiting")).length, 1);
  });

  test("an older branch's in-checkout lock is waited for too", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB, join(checkoutB, ".e2e-running"));
    const result = tryAcquireRunLock(checkoutA, [], {
      lockPath,
      worktrees: [checkoutA, checkoutB],
    });
    assert.equal(result.holder?.pid, holder.pid);
  });
});

describe("the hooks' check", () => {
  test("nothing running: passes silently", () => {
    const stderr = sink();
    assert.equal(check(checkoutA, { lockPath, worktrees: [], stderr, env: {} }), 0);
    assert.equal(stderr.text, "");
  });

  test("a run from another checkout: a commit here proceeds, with a note", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB);
    const stderr = sink();
    const code = check(checkoutA, {
      purpose: "a commit",
      lockPath,
      worktrees: [],
      stderr,
      env: {},
    });
    assert.equal(code, 0);
    assert.match(stderr.text, /note — an e2e run is active/);
  });

  test("a run from another checkout blocks a push", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutB);
    const stderr = sink();
    const code = check(checkoutA, {
      anyCheckout: true,
      purpose: "a push",
      lockPath,
      worktrees: [],
      stderr,
      env: {},
    });
    assert.equal(code, 1);
    assert.ok(stderr.text.includes(checkoutB));
    assert.ok(stderr.text.includes(`pid ${holder.pid}`));
  });

  test("a run from this checkout blocks a commit, and the override still works", async () => {
    const holder = fakeRun();
    await untilLive(holder.pid);
    writeLock(holder.pid, checkoutA);
    assert.equal(liveLock(checkoutA, { lockPath })?.pid, holder.pid);
    assert.equal(liveLock(checkoutB, { lockPath }), null);
    const opts = { purpose: "a commit", lockPath, worktrees: [] };
    assert.equal(check(checkoutA, { ...opts, stderr: sink(), env: {} }), 1);
    const stderr = sink();
    assert.equal(
      check(checkoutA, { ...opts, stderr, env: { [OVERRIDE_ENV]: "1" } }),
      0,
    );
    assert.match(stderr.text, /allowing a commit/);
  });

  test("a stale lock blocks nothing", () => {
    writeLock(deadPid(), checkoutA);
    const stderr = sink();
    const opts = { lockPath, worktrees: [], stderr, env: {} };
    assert.equal(check(checkoutA, { ...opts, anyCheckout: true }), 0);
    assert.equal(stderr.text, "");
  });
});
