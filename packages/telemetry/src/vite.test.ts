import assert from "node:assert/strict";
import { test } from "node:test";
import {
  OFF_JSON,
  connectSrcFor,
  previewConfig,
  servedJson,
  servedTelemetry,
} from "./vite.ts";

test("connect-src has the contract's three rows", () => {
  assert.equal(
    connectSrcFor({
      enabled: true,
      endpoint: "https://telemetry.thunderforge.dev",
    }),
    "connect-src 'self' data: blob: https://telemetry.thunderforge.dev",
  );
  assert.equal(
    connectSrcFor({ enabled: true, endpoint: "https://otel.example.org/otlp" }),
    "connect-src 'self' data: blob: https://otel.example.org",
  );
  assert.equal(connectSrcFor(null), "connect-src 'self' data: blob:");
});

test('unset or broken env serves exactly {"enabled":false}', () => {
  assert.equal(servedJson(previewConfig(undefined)), OFF_JSON);
  assert.equal(servedJson(previewConfig("{nope")), OFF_JSON);
  assert.equal(OFF_JSON, '{"enabled":false}');
});

function serve(env: Record<string, string | undefined>, url: string) {
  const plugin = servedTelemetry({
    paths: ["/telemetry.json", "/demo/telemetry.json"],
    env,
  });
  let mw: ((req: unknown, res: unknown, next: () => void) => void) | undefined;
  const server = { middlewares: { use: (f: typeof mw) => (mw = f) } };
  (plugin.configurePreviewServer as (s: unknown) => void)(server);
  const out: {
    status?: number;
    headers: Record<string, string>;
    body?: string;
    next: boolean;
  } = {
    headers: {},
    next: false,
  };
  mw!(
    { url },
    {
      setHeader: (k: string, v: string) => (out.headers[k] = v),
      set statusCode(s: number) {
        out.status = s;
      },
      end: (b: string) => (out.body = b),
    },
    () => (out.next = true),
  );
  return { out, plugin };
}

test("the plugin serves the file at both paths, no-store", () => {
  const env = {
    THUNDERFORGE_PREVIEW_TELEMETRY:
      '{"enabled":true,"endpoint":"http://127.0.0.1:4318","tier":"operator"}',
  };
  for (const url of ["/telemetry.json", "/demo/telemetry.json?x=1"]) {
    const { out } = serve(env, url);
    assert.equal(out.status, 200);
    assert.equal(out.headers["Cache-Control"], "no-store");
    assert.equal(JSON.parse(out.body!).endpoint, "http://127.0.0.1:4318");
  }
  assert.equal(serve(env, "/other").out.next, true);
  assert.equal(serve({}, "/telemetry.json").out.body, OFF_JSON);
});

test("the plugin sets server and preview headers", () => {
  const { plugin } = serve({}, "/x");
  const cfg = (
    plugin.config as () => Record<string, { headers: Record<string, string> }>
  )();
  assert.equal(
    cfg.server.headers["Content-Security-Policy"],
    "connect-src 'self' data: blob:",
  );
  assert.equal(
    cfg.preview.headers["Content-Security-Policy"],
    "connect-src 'self' data: blob:",
  );
});
