/**
 * Unit tests for `scripts/e2e/run-lock.mjs`: one e2e run per checkout, and at
 * most N per machine (spec 089).
 *
 * Every test uses its own temporary lock directory and checkouts, never the
 * real `/tmp` slots, so they are safe beside a live e2e run. A live holder is a
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
  CHECKOUT_LOCK,
  LOCK_FILE,
  OVERRIDE_ENV,
  acquireRunLock,
  check,
  isLiveRun,
  liveLock,
  lockDir as lockDirFor,
  machineLockPath,
  maxConcurrent,
  releaseRunLock,
  slotPath,
  tryAcquireRunLock,
} from "../run-lock.mjs";

let dir;
let lockDir;
let checkoutA;
let checkoutB;
let checkoutC;
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

async function liveHolder() {
  const holder = fakeRun();
  for (let i = 0; i < 100 && !isLiveRun(holder.pid); i += 1) {
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
  assert.ok(isLiveRun(holder.pid), "the fake holder never came up");
  return holder;
}

/** A pid that has certainly exited. */
function deadPid() {
  return (
    spawnSync(
      process.execPath,
      ["-e", "process.stdout.write(String(process.pid))"],
      { encoding: "utf-8" },
    ).stdout.trim() * 1
  );
}

function writeLock(path, pid, cwd) {
  writeFileSync(
    path,
    JSON.stringify({ pid, cwd, startedAt: new Date().toISOString(), args: [] }),
  );
}

/** A run from `cwd` holding slot `n`, as another process would leave it. */
function holdSlot(n, pid, cwd) {
  writeLock(slotPath(lockDir, n), pid, cwd);
  writeLock(join(cwd, CHECKOUT_LOCK), pid, cwd);
}

/** A stderr that remembers what was written. */
function sink() {
  const out = { text: "", write: (s) => (out.text += s) };
  return out;
}

const opts = () => ({ lockDir, max: 2 });

beforeEach(() => {
  dir = realpathSync(mkdtempSync(join(tmpdir(), "run-lock-test-")));
  lockDir = join(dir, "locks");
  checkoutA = join(dir, "checkout-a");
  checkoutB = join(dir, "checkout-b");
  checkoutC = join(dir, "checkout-c");
  for (const path of [lockDir, checkoutA, checkoutB, checkoutC]) {
    mkdirSync(path);
  }
});

afterEach(() => {
  for (const child of holders.splice(0)) child.kill("SIGKILL");
  rmSync(dir, { recursive: true, force: true });
});

describe("where the locks live", () => {
  test("slots outside any checkout, the same for every checkout", () => {
    assert.equal(lockDirFor({}), "/tmp");
    assert.equal(lockDirFor({ THUNDERFORGE_E2E_LOCK_DIR: "/x" }), "/x");
    assert.equal(slotPath("/x", 1), "/x/thunderforge-e2e.slot-1");
    assert.equal(machineLockPath({}), join("/tmp", LOCK_FILE));
  });

  test("the cap defaults to 2 and takes THUNDERFORGE_E2E_MAX_CONCURRENT", () => {
    assert.equal(maxConcurrent({}), 2);
    assert.equal(maxConcurrent({ THUNDERFORGE_E2E_MAX_CONCURRENT: "3" }), 3);
    assert.equal(maxConcurrent({ THUNDERFORGE_E2E_MAX_CONCURRENT: "0" }), 2);
    assert.equal(maxConcurrent({ THUNDERFORGE_E2E_MAX_CONCURRENT: "x" }), 2);
  });
});

describe("taking and releasing", () => {
  test("a free run takes the checkout lock and slot 0, and names itself", () => {
    const { lock } = tryAcquireRunLock(checkoutA, ["--slice=x"], opts());
    assert.equal(lock.slot, 0);
    const slot = JSON.parse(readFileSync(slotPath(lockDir, 0), "utf-8"));
    assert.equal(slot.pid, process.pid);
    assert.equal(slot.cwd, checkoutA);
    assert.deepEqual(slot.args, ["--slice=x"]);
    assert.ok(existsSync(join(checkoutA, CHECKOUT_LOCK)));
    releaseRunLock({ root: checkoutA, lockDir });
    assert.equal(existsSync(slotPath(lockDir, 0)), false);
    assert.equal(existsSync(join(checkoutA, CHECKOUT_LOCK)), false);
  });

  test("never writes the pre-089 machine lock", () => {
    tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(existsSync(join(lockDir, LOCK_FILE)), false);
  });

  test("release leaves another run's locks alone", async () => {
    const holder = await liveHolder();
    holdSlot(0, holder.pid, checkoutA);
    releaseRunLock({ root: checkoutA, lockDir });
    assert.ok(existsSync(slotPath(lockDir, 0)));
    assert.ok(existsSync(join(checkoutA, CHECKOUT_LOCK)));
  });

  test("an unreadable slot does not block", () => {
    writeFileSync(slotPath(lockDir, 0), "not json");
    writeFileSync(slotPath(lockDir, 1), "not json");
    const result = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(result.lock?.pid, process.pid);
  });
});

describe("slots (T021)", () => {
  test("two runs from two checkouts take slots 0 and 1", async () => {
    const holder = await liveHolder();
    holdSlot(0, holder.pid, checkoutB);
    const { lock } = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(lock.slot, 1);
  });

  test("a third waits, and names both holders", async () => {
    const one = await liveHolder();
    const two = await liveHolder();
    holdSlot(0, one.pid, checkoutB);
    holdSlot(1, two.pid, checkoutC);
    const result = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(result.lock, undefined);
    assert.deepEqual(
      result.holders.map((h) => h.pid).sort(),
      [one.pid, two.pid].sort(),
    );
    // Held while waiting, so a second run from here queues behind this one.
    const own = JSON.parse(readFileSync(join(checkoutA, CHECKOUT_LOCK), "utf-8"));
    assert.equal(own.pid, process.pid);
  });

  test("with --no-wait, fails naming both holders", async () => {
    const one = await liveHolder();
    const two = await liveHolder();
    holdSlot(0, one.pid, checkoutB);
    holdSlot(1, two.pid, checkoutC);
    await assert.rejects(
      acquireRunLock(checkoutA, [], { ...opts(), wait: false }),
      (error) =>
        error.message.includes(checkoutB) &&
        error.message.includes(checkoutC) &&
        error.message.includes(`pid ${one.pid}`) &&
        error.message.includes(`pid ${two.pid}`),
    );
    // Failing gives this checkout's lock back.
    assert.equal(existsSync(join(checkoutA, CHECKOUT_LOCK)), false);
  });

  test("waits by default, and starts once a holder exits", async () => {
    const one = await liveHolder();
    const two = await liveHolder();
    holdSlot(0, one.pid, checkoutB);
    holdSlot(1, two.pid, checkoutC);
    const lines = [];
    let exitedAt = null;
    setTimeout(() => {
      exitedAt = Date.now();
      two.kill("SIGKILL"); // like `kill -9`: the slot file stays behind
    }, 300);
    const lock = await acquireRunLock(checkoutA, [], {
      ...opts(),
      pollMs: 25,
      log: (line) => lines.push(line),
    });
    assert.ok(exitedAt !== null, "took a slot while both holders were alive");
    assert.equal(lock.slot, 1);
    assert.match(lines[0], /^Waiting for an e2e slot/);
    assert.ok(lines[0].includes(checkoutB) && lines[0].includes(checkoutC));
    assert.equal(lines.filter((l) => l.startsWith("Waiting")).length, 1);
  });

  test("a dead holder's slot is taken", async () => {
    const live = await liveHolder();
    holdSlot(0, live.pid, checkoutB);
    holdSlot(1, deadPid(), checkoutC);
    const { lock } = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(lock.slot, 1);
  });

  test("a slot whose pid now belongs to something else is taken", () => {
    const other = spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], {
      stdio: "ignore",
    });
    holders.push(other);
    writeLock(slotPath(lockDir, 0), other.pid, checkoutB);
    const { lock } = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(lock.slot, 0);
  });
});

describe("an older branch's run (T022, FR-010)", () => {
  test("a live pre-089 machine lock holds every slot", async () => {
    const old = await liveHolder();
    writeLock(join(lockDir, LOCK_FILE), old.pid, checkoutB);
    const result = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(result.lock, undefined);
    assert.deepEqual(
      result.holders.map((h) => h.pid),
      [old.pid],
    );
  });

  test("a stale pre-089 machine lock holds nothing", () => {
    writeLock(join(lockDir, LOCK_FILE), deadPid(), checkoutB);
    const { lock } = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(lock.slot, 0);
  });
});

describe("the same checkout (T023, FR-008)", () => {
  test("a second run here waits on .e2e-running even with slots free", async () => {
    const first = await liveHolder();
    writeLock(join(checkoutA, CHECKOUT_LOCK), first.pid, checkoutA);
    const result = tryAcquireRunLock(checkoutA, [], { ...opts(), max: 4 });
    assert.equal(result.lock, undefined);
    assert.equal(result.holders[0].pid, first.pid);
    assert.equal(existsSync(slotPath(lockDir, 0)), false);
  });

  test("the oldest branches' .e2e-shards.lock counts too", async () => {
    const first = await liveHolder();
    writeLock(join(checkoutA, ".e2e-shards.lock"), first.pid, checkoutA);
    const result = tryAcquireRunLock(checkoutA, [], opts());
    assert.equal(result.holders?.[0].pid, first.pid);
  });

  test("liveLock finds this checkout's run, not another's", async () => {
    const holder = await liveHolder();
    holdSlot(0, holder.pid, checkoutA);
    assert.equal(liveLock(checkoutA, { lockDir })?.pid, holder.pid);
    assert.equal(liveLock(checkoutB, { lockDir }), null);
  });
});

describe("the hooks' check (Decision 8)", () => {
  const checkOpts = (extra) => ({
    lockDir,
    worktrees: [checkoutA, checkoutB],
    env: {},
    ...extra,
  });

  test("nothing running: passes silently", () => {
    const stderr = sink();
    assert.equal(check(checkoutA, checkOpts({ stderr })), 0);
    assert.equal(stderr.text, "");
  });

  test("another checkout's 089 run: a commit and a push proceed, with a note", async () => {
    const holder = await liveHolder();
    holdSlot(0, holder.pid, checkoutB);
    for (const anyCheckout of [false, true]) {
      const stderr = sink();
      assert.equal(check(checkoutA, checkOpts({ anyCheckout, stderr })), 0);
      assert.match(stderr.text, /note — an e2e run is active/);
    }
  });

  test("a pre-089 run from anywhere blocks a push, not a commit", async () => {
    const old = await liveHolder();
    writeLock(join(lockDir, LOCK_FILE), old.pid, checkoutB);
    const push = sink();
    assert.equal(
      check(checkoutA, checkOpts({ anyCheckout: true, purpose: "a push", stderr: push })),
      1,
    );
    assert.ok(push.text.includes(`pid ${old.pid}`));
    assert.equal(check(checkoutA, checkOpts({ stderr: sink() })), 0);
  });

  test("an older branch's in-checkout run elsewhere blocks a push", async () => {
    const old = await liveHolder();
    // In a checkout, with no slot: a run from before the machine-wide lock.
    writeLock(join(checkoutB, CHECKOUT_LOCK), old.pid, checkoutB);
    assert.equal(
      check(checkoutA, checkOpts({ anyCheckout: true, stderr: sink() })),
      1,
    );
  });

  test("this checkout's run blocks a commit, and the override still works", async () => {
    const holder = await liveHolder();
    holdSlot(0, holder.pid, checkoutA);
    assert.equal(
      check(checkoutA, checkOpts({ purpose: "a commit", stderr: sink() })),
      1,
    );
    const stderr = sink();
    assert.equal(
      check(
        checkoutA,
        checkOpts({ purpose: "a commit", stderr, env: { [OVERRIDE_ENV]: "1" } }),
      ),
      0,
    );
    assert.match(stderr.text, /allowing a commit/);
  });

  test("a stale lock blocks nothing", () => {
    holdSlot(0, deadPid(), checkoutA);
    const stderr = sink();
    assert.equal(check(checkoutA, checkOpts({ anyCheckout: true, stderr })), 0);
    assert.equal(stderr.text, "");
  });
});
