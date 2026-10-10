import assert from "node:assert/strict";
import { test } from "node:test";
import {
  ALLOWED_ATTRIBUTES,
  EVENT_NAMES,
  filterAttributes,
} from "./allowList.ts";

test("ALLOWED_ATTRIBUTES is exactly the contract's union", () => {
  assert.deepEqual(
    [...ALLOWED_ATTRIBUTES],
    [
      "event.name",
      "session.id",
      "t.ms",
      "route",
      "referrer.origin",
      "utm.source",
      "utm.medium",
      "utm.campaign",
      "nav.type",
      "step",
      "entry",
      "cta",
      "placement",
      "action",
      "root_field",
      "section",
      "error.source",
      "error.type",
      "error.message",
      "error.stack",
      "error.count",
      "reason",
      "stage",
      "metric",
      "value",
      "rating",
      "duration.ms",
      "dns.ms",
      "connect.ms",
      "ttfb.ms",
      "dom.ms",
      "load.ms",
      "transfer.bytes",
      "fps.p5",
      "fps.p50",
      "frame_ms.p95",
      "tokens.bucket",
      "internal_errors",
      "dropped",
    ],
  );
});

test("EVENT_NAMES is exactly the contract's table", () => {
  assert.deepEqual(
    [...EVENT_NAMES],
    [
      "page_view",
      "funnel",
      "demo.action",
      "demo.not_in_demo",
      "cta_clicked",
      "scroll_depth",
      "error",
      "engine.load_failed",
      "web_vital",
      "long_task",
      "nav_timing",
      "engine.frames",
      "telemetry.internal",
      "sheet_import.step",
    ],
  );
});

test("an unknown attribute is dropped", () => {
  assert.deepEqual(
    filterAttributes({
      route: "/w/:id",
      "user.email": "a@b.c",
      value: Number.NaN,
      x: undefined,
    }),
    { route: "/w/:id" },
  );
});
