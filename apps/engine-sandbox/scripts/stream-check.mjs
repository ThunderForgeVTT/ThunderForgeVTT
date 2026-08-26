#!/usr/bin/env node
/**
 * Measures whether switching maps hitches, and by how much.
 *
 * Reads the engine's own per-frame trace (`frame_trace()`), not browser
 * timing. rAF is pinned to the display refresh, so it reads the same whether
 * the engine has 5% or 95% of the budget left; and a poll is itself work on
 * the frame loop, so the frame that stalls is the frame no poll runs during.
 * The engine records every frame and this reads the window afterwards.
 *
 * For each map: settle, clear the trace, switch, wait, then report the worst
 * frame and where it fell relative to `background_spawn` /
 * `background_loaded`.
 *
 * Usage: pnpm -F @thunderforge/engine-sandbox stream
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { assertGpuRendering, launchGpuBrowser } from "./browser.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const PORT = 5184;
const BASE = `http://localhost:${PORT}`;

/** One frame at 60Hz. A frame over this dropped at least one refresh. */
const FRAME_BUDGET_MS = 16.7;
/** Where a stall stops being a blip and starts being visible as a freeze. */
const VISIBLE_HITCH_MS = 100;
/** Time given to the switch: request, decode, upload, settle. */
const SETTLE_MS = 1_500;
/** Quiet frames recorded before each switch, to compare the switch against. */
const IDLE_MS = 1_500;
/** Ceiling on waiting for the post-load frames to land. Generous on purpose:
 *  the stall being measured has been observed above five seconds. */
const MAX_WAIT_MS = 60_000;

const server = spawn("pnpm", ["exec", "vite", "--port", String(PORT)], {
  cwd: ROOT, stdio: "ignore", detached: true,
});
const shutdown = () => { try { process.kill(-server.pid, "SIGTERM"); } catch {} };
process.on("exit", shutdown);
process.on("SIGINT", () => { shutdown(); process.exit(130); });

async function waitForServer(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try { if ((await fetch(BASE)).ok) return; } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error("sandbox dev server did not start");
}

const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
};

/** Frames carrying a given mark, by index into the trace. */
const indexOfMark = (trace, mark) =>
  trace.findIndex((sample) => sample.marks.some((m) => m.startsWith(mark)));

await waitForServer();
const { browser, channel } = await launchGpuBrowser(chromium);
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
await page.goto(BASE);
await page.waitForFunction(() => document.querySelectorAll("canvas").length > 0, { timeout: 60_000 });
// The wasm engine is large; give it room to boot and present a first frame.
await page.waitForTimeout(20_000);

const idle = await assertGpuRendering(page, "stream-check");
const maps = await page.evaluate(() => window.__stress.mapNames);
console.log(
  `${channel}, idling at ${idle.frame_time_ms.toFixed(1)}ms/frame ` +
    `(${idle.fps.toFixed(1)}fps).`,
);
console.log(`Measuring ${maps.length} map switches, ${SETTLE_MS / 1000}s each.\n`);

const rows = [];
for (const name of maps) {
  const trace = await page.evaluate(
    async ([mapName, idleMs, settle, maxWait]) => {
      // Let the previous switch finish, or its decode lands in this window
      // and gets attributed to this map.
      await new Promise((r) => setTimeout(r, 1200));
      window.__stress.clearFrameTrace();
      // Record quiet frames first: the switch has to be compared against
      // this engine on this machine right now, not against 16.7ms.
      await new Promise((r) => setTimeout(r, idleMs));
      window.__stress.loadMap(mapName);

      // A sample is only pushed once its frame *completes*, so a stall
      // longer than the wait window leaves no sample at all and reads as
      // "no hitch" — the harness would be silent exactly where the problem
      // is worst. Wait for the frames after `background_loaded` to actually
      // land instead of trusting a fixed timeout. (This poll cannot run
      // during the stall either: wasm Bevy is single-threaded, so the whole
      // page is frozen. It resumes once the frame ends, which is the point.)
      const deadline = Date.now() + maxWait;
      let trace = [];
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 250));
        trace = window.__stress.frameTrace();
        const loaded = trace.findIndex((s) =>
          s.marks.some((m) => m.startsWith("background_loaded")),
        );
        // Three settled frames after the load is enough to see the spike
        // and its recovery.
        if (loaded >= 0 && trace.length > loaded + 3) break;
      }
      await new Promise((r) => setTimeout(r, settle));
      return window.__stress.frameTrace();
    },
    [name, IDLE_MS, SETTLE_MS, MAX_WAIT_MS],
  );

  const spawnAt = indexOfMark(trace, "background_spawn");
  const loadedAt = indexOfMark(trace, "background_loaded");
  const durations = trace.map((s) => s.dtMs);

  // Baseline is the quiet frames before the switch was even requested.
  const baseline = median(durations.slice(0, Math.max(spawnAt, 1)));
  const worst = trace.reduce((a, b) => (b.dtMs > a.dtMs ? b : a), trace[0]);
  const worstAt = trace.indexOf(worst);
  // Everything the switch cost over and above running idle. A single long
  // frame and a run of mildly slow ones are both freezes to a user, and
  // only this sums both.
  const stallMs = durations
    .slice(Math.max(spawnAt, 0))
    .reduce((total, dt) => total + Math.max(0, dt - baseline), 0);
  const dropped = durations.filter((d) => d > baseline * 1.5).length;

  // If the frame after the load never completed, every number below is a
  // floor, not a measurement. Say so rather than reporting it as clean.
  const truncated = loadedAt < 0 || trace.length <= loadedAt + 1;

  rows.push({
    name, baseline, worst: worst.dtMs, worstAt, spawnAt, loadedAt, dropped, truncated,
    stallMs,
    frames: trace.length,
    // Frames between asking for the map and the bytes being ready.
    loadFrames: loadedAt >= 0 && spawnAt >= 0 ? loadedAt - spawnAt : null,
    worstRelativeToLoad: loadedAt >= 0 ? worstAt - loadedAt : null,
  });

  console.log(
    `${truncated ? "?" : worst.dtMs > VISIBLE_HITCH_MS ? "✗" : "✓"} ${name.padEnd(44)} ` +
      `baseline ${baseline.toFixed(1)}ms · worst ${worst.dtMs.toFixed(1)}ms` +
      (rows.at(-1).worstRelativeToLoad === null
        ? " (never loaded)"
        : ` @ load${rows.at(-1).worstRelativeToLoad >= 0 ? "+" : ""}${rows.at(-1).worstRelativeToLoad}f`) +
      ` · ${dropped} slow frame(s) · ${stallMs.toFixed(0)}ms lost` +
      (truncated ? " · INCOMPLETE (post-load frame never landed)" : ""),
  );
}

// The cache's whole claim is that returning to a map is free. Test it
// against the largest map measured, which is where a regression would cost
// the most.
const largest = rows.reduce((a, b) => (b.worst > a.worst ? b : a), rows[0]);
const other = rows.find((r) => r.name !== largest.name);

console.log(`\nRevisiting ${largest.name} after a switch to ${other.name}:`);
const revisit = await page.evaluate(
  async ([target, away, idleMs, maxWait]) => {
    const settle = async (name) => {
      window.__stress.loadMap(name);
      const deadline = Date.now() + maxWait;
      while (Date.now() < deadline) {
        await new Promise((r) => setTimeout(r, 250));
        const trace = window.__stress.frameTrace();
        const loaded = trace.findIndex((s) =>
          s.marks.some((m) => m.startsWith("background_loaded")),
        );
        if (loaded >= 0 && trace.length > loaded + 3) return;
      }
    };

    await settle(target);
    await settle(away);
    await new Promise((r) => setTimeout(r, 1200));
    window.__stress.clearFrameTrace();
    await new Promise((r) => setTimeout(r, idleMs));
    await settle(target);
    await new Promise((r) => setTimeout(r, 1000));
    return window.__stress.frameTrace();
  },
  [largest.name, other.name, IDLE_MS, MAX_WAIT_MS],
);

const revisitWorst = Math.max(...revisit.map((s) => s.dtMs));
const cacheHeld = revisitWorst < VISIBLE_HITCH_MS;
console.log(
  `${cacheHeld ? "✓" : "✗"} worst frame ${revisitWorst.toFixed(1)}ms ` +
    `(first visit was ${largest.worst.toFixed(1)}ms) — texture ` +
    `${cacheHeld ? "stayed resident" : "was re-uploaded; the cache is not holding it"}`,
);

await browser.close();
shutdown();

const worstOverall = rows.reduce((a, b) => (b.worst > a.worst ? b : a), rows[0]);
const hitching = rows.filter((r) => r.worst > VISIBLE_HITCH_MS);

console.log(`\nWorst first visit: ${worstOverall.name} — longest single frame ` +
  `${worstOverall.worst.toFixed(1)}ms against a ${worstOverall.baseline.toFixed(1)}ms baseline, ` +
  `${worstOverall.stallMs.toFixed(0)}ms lost in total.`);
console.log(
  `${hitching.length}/${rows.length} first visits stalled over ${VISIBLE_HITCH_MS}ms.`,
);
const loadWindows = rows.map((r) => r.loadFrames).filter((f) => f !== null);
if (loadWindows.length > 0) {
  console.log(
    `Frames between request and decode completing: ` +
      `${Math.min(...loadWindows)}-${Math.max(...loadWindows)}.`,
  );
}

// A revisit regression is a real failure; a slow first visit is the known,
// unsolved half (it needs a compressed texture format or an upload split
// across frames, not a cache).
process.exit(cacheHeld ? 0 : 1);
