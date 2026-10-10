/**
 * Writes the browser fixtures (spec 086 T094, R26): one OTLP/JSON body per
 * signal the browser sends, from `packages/telemetry`'s own encoder, so the
 * gateway is tested against what a browser really posts.
 *
 *   node apps/telemetry-gateway/tests/fixtures/capture-browser.mts
 */

import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { encodeBatch } from "../../../../packages/telemetry/src/otlp/encode.ts";
import type { Batch } from "../../../../packages/telemetry/src/records.ts";

const here = dirname(fileURLToPath(import.meta.url));
const at = Date.UTC(2026, 9, 1, 12, 0, 0);

const batch: Batch = {
  resource: {
    "service.name": "thunderforge-web",
    "service.version": "1.4.0",
    "deployment.environment": "production",
    "thunderforge.tier": "anonymous",
    "thunderforge.instance.id": "0192f1a4-7b3c-7d2e-9f10-3a4b5c6d7e8f",
    "browser.family": "chrome",
    "browser.major": "131",
    "os.family": "linux",
    "device.mobile": false,
    "viewport.bucket": "1280-1919",
  },
  records: [
    {
      kind: "log",
      name: "page.view",
      time: at,
      severity: "info",
      attrs: {
        "event.name": "page.view",
        "session.id": "6b1f0c2d9e8a4b7c",
        route: "/worlds",
        "nav.type": "navigate",
      },
    },
    {
      kind: "log",
      name: "error",
      time: at + 1500,
      severity: "error",
      body: "TypeError: x is undefined",
      attrs: {
        "event.name": "error",
        "session.id": "6b1f0c2d9e8a4b7c",
        "error.source": "window",
        "error.type": "TypeError",
        "error.message": "x is undefined",
        "error.count": 1,
        "t.ms": 1500.5,
      },
    },
    {
      kind: "span",
      name: "engine.load",
      traceId: "4bf92f3577b34da6a3ce929d0e0e4736",
      spanId: "00f067aa0ba902b7",
      start: at,
      end: at + 2400,
      attrs: { "session.id": "6b1f0c2d9e8a4b7c", bytes: 18_400_000 },
    },
    {
      kind: "span",
      name: "download",
      traceId: "4bf92f3577b34da6a3ce929d0e0e4736",
      spanId: "53995c3f42cd8ad8",
      parentSpanId: "00f067aa0ba902b7",
      start: at,
      end: at + 1200,
      attrs: { "session.id": "6b1f0c2d9e8a4b7c" },
    },
  ],
};

const { posts } = encodeBatch(batch);
for (const post of posts) {
  const signal = post.path.slice("/v1/".length);
  const file = join(here, `browser-${signal}.json`);
  writeFileSync(file, JSON.stringify(JSON.parse(post.body), null, 2) + "\n");
  console.log(`wrote ${file}`);
}
