#!/usr/bin/env node
/**
 * Headless render check: boots the sandbox, loads each example map, and
 * fails if the canvas is a single flat colour.
 *
 * Runs against the compositor (a Playwright screenshot), NOT `gl.readPixels`.
 * That distinction was learned the hard way: wgpu does not request
 * `preserveDrawingBuffer`, so a `readPixels` landing after the compositor has
 * recycled the buffer returns transparent black regardless of what was drawn.
 * Readings taken that way alternate between the real clear colour and
 * (0,0,0,0) purely on timing, which is worse than no measurement — it looks
 * like hard evidence. The screenshot path is stable.
 *
 * Usage: pnpm -F @thunderforge/engine-sandbox check
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { launchGpuBrowser } from "./browser.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const PORT = 5181;
const BASE = `http://localhost:${PORT}`;

/** Bevy's ClearColor in `src/engine/src/lib.rs`, as 8-bit sRGB. */
const CLEAR_COLOR = [34, 40, 49];
/** Above this share of one colour, the canvas is considered blank. */
const FLAT_THRESHOLD = 0.98;

const server = spawn("pnpm", ["exec", "vite", "--port", String(PORT)], {
  cwd: ROOT,
  stdio: "ignore",
  detached: true,
});

const shutdown = () => {
  try {
    process.kill(-server.pid, "SIGTERM");
  } catch {
    /* already gone */
  }
};
process.on("exit", shutdown);
process.on("SIGINT", () => {
  shutdown();
  process.exit(130);
});

async function waitForServer(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(BASE);
      if (response.ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error("sandbox dev server did not start");
}

async function sample(page) {
  const png = await page.locator("#stage").screenshot();
  return page.evaluate(async (base64) => {
    const response = await fetch(`data:image/png;base64,${base64}`);
    const bitmap = await createImageBitmap(await response.blob());
    const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = surface.getContext("2d");
    context.drawImage(bitmap, 0, 0);
    const { data } = context.getImageData(0, 0, bitmap.width, bitmap.height);

    const counts = new Map();
    let sampled = 0;
    for (let i = 0; i < data.length; i += 16) {
      const key = `${data[i] >> 3},${data[i + 1] >> 3},${data[i + 2] >> 3}`;
      counts.set(key, (counts.get(key) ?? 0) + 1);
      sampled += 1;
    }
    let bestKey = "";
    let bestCount = 0;
    for (const [key, count] of counts) {
      if (count > bestCount) [bestKey, bestCount] = [key, count];
    }
    const [r, g, b] = bestKey.split(",").map((n) => (Number(n) << 3) + 4);
    return {
      distinct: counts.size,
      dominantFraction: bestCount / sampled,
      dominant: [r, g, b],
    };
  }, png.toString("base64"));
}

const near = (a, b, tolerance = 6) => a.every((v, i) => Math.abs(v - b[i]) <= tolerance);

await waitForServer();

// Plain `chromium.launch()` gets no GPU and the engine falls back to
// software rasterization at ~4fps. That does not change this check's
// pass/fail (a blank canvas is blank either way), but it does mean the
// check was not exercising the path the app ships on.
const { browser } = await launchGpuBrowser(chromium);
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
await page.goto(BASE);
await page.waitForFunction(() => document.querySelectorAll("canvas").length > 0, { timeout: 60_000 });
// The wasm engine is large; give it room to boot and present a first frame.
await page.waitForTimeout(20_000);

const maps = await fetch(`${BASE}/assets/maps/manifest.json`).then((r) => r.json());
const failures = [];

for (const map of maps) {
  await page.getByRole("button", { name: new RegExp(`^${map.name}`) }).click();
  await page.waitForTimeout(8_000);
  const result = await sample(page);
  const flat = result.dominantFraction > FLAT_THRESHOLD;
  const isClear = near(result.dominant, CLEAR_COLOR);

  const verdict = flat
    ? `BLANK${isClear ? " (engine clear colour)" : ""}`
    : "renders";
  console.log(
    `${flat ? "✗" : "✓"} ${map.name.padEnd(44)} ${verdict.padEnd(30)} ` +
      `dominant rgb(${result.dominant.join(",")}) ${(result.dominantFraction * 100).toFixed(1)}% · ${result.distinct} colours`,
  );
  if (flat) failures.push(map.name);
}

await browser.close();
shutdown();

if (failures.length > 0) {
  console.error(`\n${failures.length}/${maps.length} maps rendered nothing.`);
  process.exit(1);
}
console.log(`\nAll ${maps.length} maps rendered content.`);
