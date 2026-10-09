import assert from "node:assert/strict";
import { test } from "node:test";
import { SESSION_KEY, loadSession, memoryStorage } from "./session.ts";
import { createTelemetry } from "./telemetry.ts";
import type { Batch } from "./records.ts";

test("the key is thunderforge.telemetry", () => {
  assert.equal(SESSION_KEY, "thunderforge.telemetry");
  const storage = memoryStorage();
  const s = loadSession({
    storage,
    now: () => 5,
    random: () => 0.5,
    sampleRate: 1,
    decideSampling: true,
  });
  assert.match(s.id, /^[0-9a-f]{32}$/);
  assert.equal(JSON.parse(storage.getItem(SESSION_KEY)!).id, s.id);
});

test("sampled is decided once", () => {
  const storage = memoryStorage();
  const a = loadSession({
    storage,
    now: () => 1,
    random: () => 0.9,
    sampleRate: 0.1,
    decideSampling: true,
  });
  assert.equal(a.sampled, false);
  const b = loadSession({
    storage,
    now: () => 2,
    random: () => 0,
    sampleRate: 1,
    decideSampling: true,
  });
  assert.equal(b.sampled, false);
  assert.equal(b.id, a.id);
});

test("a storage that throws still gives a session", () => {
  const broken = {
    getItem: () => {
      throw new Error("blocked");
    },
    setItem: () => {
      throw new Error("blocked");
    },
  };
  const s = loadSession({
    storage: broken,
    now: () => 1,
    random: () => 0,
    sampleRate: 1,
    decideSampling: true,
  });
  assert.match(s.id, /^[0-9a-f]{32}$/);
});

test("funnel(step) sends each step once, across a reload", () => {
  const storage = memoryStorage();
  const sent: Batch[] = [];
  const make = () =>
    createTelemetry({
      service: "thunderforge-demo",
      version: "1",
      config: { enabled: true, endpoint: "https://x.example" },
      sink: { send: async (b) => void sent.push(b) },
      redact: (s) => s,
      storage,
      now: () => 1,
      random: () => 0,
      privacy: { gpc: false, dnt: false },
    });
  const t = make();
  t.funnel("demo_opened", { entry: "landing" });
  t.funnel("demo_opened");
  t.flush(false);
  make().funnel("demo_opened");
  const steps = sent.flatMap((b) => b.records.map((r) => r.attrs.step));
  assert.deepEqual(steps, ["demo_opened"]);
  assert.equal(sent[0].records[0].attrs.entry, "landing");
});
