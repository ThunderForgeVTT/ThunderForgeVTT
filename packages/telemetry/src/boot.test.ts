import assert from "node:assert/strict";
import { test } from "node:test";
import { bootTelemetry } from "./boot.ts";
import type { TelemetryConfig } from "./config.ts";
import { noopTelemetry } from "./telemetry.ts";

const served = (body: unknown, status = 200) =>
  (async () =>
    new Response(JSON.stringify(body), { status })) as unknown as typeof fetch;

test("onConfig is told the served config before the chunk loads", async () => {
  const seen: (TelemetryConfig | null)[] = [];
  const order: string[] = [];
  await bootTelemetry({
    configUrl: "/telemetry.json",
    fetchImpl: served({
      enabled: true,
      endpoint: "https://otel.example.org",
      tier: "operator",
    }),
    onConfig: (c) => {
      seen.push(c);
      order.push("config");
    },
    load: async () => {
      order.push("load");
      return () => noopTelemetry;
    },
  });
  assert.equal(seen.length, 1);
  assert.equal(seen[0]?.tier, "operator");
  assert.deepEqual(order, ["config", "load"]);
});

test("off, or a failed fetch, is told as null and loads nothing", async () => {
  for (const fetchImpl of [
    served({ enabled: false }),
    served({}, 404),
    (async () => {
      throw new Error("offline");
    }) as unknown as typeof fetch,
  ]) {
    const seen: (TelemetryConfig | null)[] = [];
    let loads = 0;
    await bootTelemetry({
      configUrl: "/telemetry.json",
      fetchImpl,
      onConfig: (c) => seen.push(c),
      load: async () => {
        loads += 1;
        return () => noopTelemetry;
      },
    });
    assert.deepEqual(seen, [null]);
    assert.equal(loads, 0);
  }
});

test("a throwing onConfig does not stop the boot", async () => {
  let loads = 0;
  await bootTelemetry({
    configUrl: "/telemetry.json",
    fetchImpl: served({ enabled: true, endpoint: "https://x.example" }),
    onConfig: () => {
      throw new Error("listener");
    },
    load: async () => {
      loads += 1;
      return () => noopTelemetry;
    },
  });
  assert.equal(loads, 1);
});
