import assert from "node:assert/strict";
import { test } from "node:test";
import { endpointOrigin, parseConfig, readConfig } from "./config.ts";

const on = { enabled: true, endpoint: "https://telemetry.thunderforge.dev" };

test("anything not a clear on is off", () => {
  for (const json of [
    null,
    "x",
    {},
    { enabled: false },
    { enabled: "true", endpoint: on.endpoint },
    { enabled: true },
    { enabled: true, endpoint: "ftp://x.example" },
    { enabled: true, endpoint: "not a url" },
    { enabled: true, endpoint: "javascript:alert(1)" },
  ]) {
    assert.equal(parseConfig(json), null, JSON.stringify(json));
  }
});

test("a trailing slash is stripped and the rate clamped", () => {
  const c = parseConfig({
    ...on,
    endpoint: "https://otel.example.org/otlp//",
    sampleRate: 7,
  });
  assert.equal(c?.endpoint, "https://otel.example.org/otlp");
  assert.equal(c?.sampleRate, 1);
  assert.equal(parseConfig({ ...on, sampleRate: -1 })?.sampleRate, 0);
  assert.equal(parseConfig(on)?.sampleRate, 1);
});

test("the tier is operator only when the config says so", () => {
  assert.equal(parseConfig(on)?.tier, "anonymous");
  assert.equal(parseConfig({ ...on, tier: "operator" })?.tier, "operator");
  assert.equal(parseConfig({ ...on, tier: "full" })?.tier, "anonymous");
});

test("readConfig: non-200, unparseable and a thrown fetch are off", async () => {
  const res = (status: number, body: string) =>
    (async () => new Response(body, { status })) as unknown as typeof fetch;
  assert.equal(await readConfig("/t", res(404, JSON.stringify(on))), null);
  assert.equal(await readConfig("/t", res(200, "{nope")), null);
  assert.equal(
    await readConfig("/t", (async () => {
      throw new Error("down");
    }) as unknown as typeof fetch),
    null,
  );
  assert.equal(
    (await readConfig("/t", res(200, JSON.stringify(on))))?.enabled,
    true,
  );
});

test("readConfig omits credentials and the cache", async () => {
  let init: RequestInit | undefined;
  await readConfig("/telemetry.json", (async (_u: string, i: RequestInit) => {
    init = i;
    return new Response("{}");
  }) as unknown as typeof fetch);
  assert.equal(init?.credentials, "omit");
  assert.equal(init?.cache, "no-store");
});

test("endpointOrigin", () => {
  assert.equal(
    endpointOrigin("https://otel.example.org/otlp"),
    "https://otel.example.org",
  );
  assert.equal(endpointOrigin(undefined), "");
});
