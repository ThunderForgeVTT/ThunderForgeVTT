import assert from "node:assert/strict";
import { test } from "node:test";
import { errorsOnly, privacyOf } from "./privacy.ts";
import { createTelemetry } from "./telemetry.ts";
import { memoryStorage } from "./session.ts";
import type { Batch } from "./records.ts";

test("privacyOf reads GPC and DNT strictly", () => {
  assert.deepEqual(privacyOf({ globalPrivacyControl: true }), {
    gpc: true,
    dnt: false,
  });
  assert.deepEqual(privacyOf({ doNotTrack: "1" }), { gpc: false, dnt: true });
  assert.deepEqual(
    privacyOf({ doNotTrack: "0", globalPrivacyControl: "yes" }),
    {
      gpc: false,
      dnt: false,
    },
  );
  assert.equal(errorsOnly(privacyOf(undefined)), false);
});

for (const privacy of [
  { gpc: true, dnt: false },
  { gpc: false, dnt: true },
]) {
  test(`GPC or DNT limits the session to errors (${JSON.stringify(privacy)})`, () => {
    const sent: Batch[] = [];
    const t = createTelemetry({
      service: "thunderforge-demo",
      version: "1",
      config: { enabled: true, endpoint: "https://x.example", sampleRate: 1 },
      sink: { send: async (b) => void sent.push(b) },
      redact: (s) => s,
      storage: memoryStorage(),
      now: () => 1000,
      random: () => 0,
      privacy,
    });
    t.event("page_view", { route: "/" });
    t.funnel("demo_opened");
    t.event("web_vital", { metric: "LCP", value: 1 });
    t.span("page.load", 0, 1);
    assert.equal(t.traceparent(), null);
    t.error("onerror", new Error("boom"));
    t.event("engine.load_failed", { reason: "no_webgl2" });
    t.flush(false);
    const names = sent.flatMap((b) => b.records.map((r) => r.name));
    assert.deepEqual(names, ["error", "engine.load_failed"]);
  });
}
