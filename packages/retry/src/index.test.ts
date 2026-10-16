import assert from "node:assert/strict";
import { test } from "node:test";
import { backoffDelay, retry } from "./index.ts";

const curve = { baseMs: 100, maxMs: 1000 };

test("the delay doubles from the base and stops at the cap", () => {
  assert.deepEqual(
    [0, 1, 2, 3, 4, 5].map((n) => backoffDelay(n, curve)),
    [100, 200, 400, 800, 1000, 1000],
  );
});

test("full jitter picks a point between zero and the delay", () => {
  const full = { ...curve, jitter: "full" as const };
  assert.equal(
    backoffDelay(2, full, () => 0),
    0,
  );
  assert.equal(
    backoffDelay(2, full, () => 0.5),
    200,
  );
  assert.equal(
    backoffDelay(2, full, () => 0.999),
    399,
  );
});

test("a retried failure waits the curve, then the success comes back", async () => {
  const waits: number[] = [];
  let tries = 0;
  const result = await retry(
    async () => {
      tries += 1;
      if (tries < 3) throw new Error("dropped");
      return "ok";
    },
    {
      ...curve,
      retries: 5,
      shouldRetry: () => true,
      sleep: async (ms) => void waits.push(ms),
    },
  );
  assert.equal(result, "ok");
  assert.deepEqual(waits, [100, 200]);
});

test("a failure not worth retrying is thrown at once", async () => {
  let tries = 0;
  const refused = new Error("refused");
  await assert.rejects(
    retry(
      async () => {
        tries += 1;
        throw refused;
      },
      { ...curve, retries: 5, shouldRetry: () => false, sleep: async () => {} },
    ),
    refused,
  );
  assert.equal(tries, 1);
});

test("spent retries throw the last error", async () => {
  let tries = 0;
  await assert.rejects(
    retry(
      async () => {
        tries += 1;
        throw new Error(`try ${tries}`);
      },
      { ...curve, retries: 2, shouldRetry: () => true, sleep: async () => {} },
    ),
    /try 3/,
  );
  assert.equal(tries, 3);
});
