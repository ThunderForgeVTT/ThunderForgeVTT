#!/usr/bin/env node
/**
 * Checks that token art renders — the CC-BY-SA FreeOrion fixtures in
 * `examples/space/`, loaded as a scene background plus six tokens.
 *
 * Guards the engine half of `photoUrl` support, which the canvas-core unit
 * tests cannot reach: they cover the fit arithmetic
 * (`token_art::fit_within_footprint`), not whether a `Sprite` was ever
 * given an image. Asserts three things:
 *
 *  1. Every art file is actually fetched and returns 200. Bevy also probes
 *     `<name>.meta` before each image and expects a 404 — those are
 *     excluded, not failures.
 *  2. The canvas is not blank.
 *  3. Nothing regressed the plain colour-swatch token, which is what a
 *     token with no `photoUrl` still renders as.
 *
 * Usage: pnpm -F @thunderforge/engine-sandbox space
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { assertGpuRendering, launchGpuBrowser } from "./browser.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const PORT = 5188;
const BASE = `http://localhost:${PORT}`;

/** Above this share of one colour, the canvas drew nothing worth seeing. */
const FLAT_THRESHOLD = 0.98;

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

await waitForServer();
const { browser } = await launchGpuBrowser(chromium);
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });

const responses = [];
page.on("response", (r) => {
  if (r.url().includes("/assets/space/")) {
    responses.push([r.status(), r.url().split("/assets/")[1]]);
  }
});

await page.goto(BASE);
await page.waitForFunction(() => document.querySelectorAll("canvas").length > 0, { timeout: 60_000 });
await page.waitForTimeout(20_000);
await assertGpuRendering(page, "space-check");

await page.evaluate(() => window.__stress.loadSpaceDemo());
await page.waitForTimeout(6_000);

const failures = [];

const art = responses.filter(([, url]) => !url.endsWith(".meta"));
const broken = art.filter(([status]) => status !== 200);
console.log(`${art.length} art file(s) requested, ${broken.length} failed`);
for (const [status, url] of broken) console.log(`  ✗ ${status}  ${url}`);
if (art.length === 0) failures.push("no art was requested at all");
if (broken.length > 0) failures.push(`${broken.length} art file(s) did not return 200`);

const png = await page.locator("#stage").screenshot();
const shape = await page.evaluate(async (base64) => {
  const blob = await (await fetch(`data:image/png;base64,${base64}`)).blob();
  const bitmap = await createImageBitmap(blob);
  const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
  const ctx = surface.getContext("2d");
  ctx.drawImage(bitmap, 0, 0);
  const { data } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);

  const counts = new Map();
  let sampled = 0;
  // The engine's flat token blue (0.282, 0.565, 0.996 in linear sRGB).
  // Its presence proves a token with no `photoUrl` still draws as a
  // colour swatch rather than an empty sprite.
  let swatch = 0;
  for (let i = 0; i < data.length; i += 16) {
    const [r, g, b] = [data[i], data[i + 1], data[i + 2]];
    const key = `${r >> 3},${g >> 3},${b >> 3}`;
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (Math.abs(r - 96) < 24 && Math.abs(g - 154) < 24 && b > 200) swatch += 1;
    sampled += 1;
  }
  const dominant = Math.max(...counts.values()) / sampled;
  return { dominant, distinct: counts.size, swatch };
}, png.toString("base64"));

console.log(
  `canvas: ${shape.distinct} colours, dominant ${(shape.dominant * 100).toFixed(1)}%, ` +
    `${shape.swatch} colour-swatch sample(s)`,
);
if (shape.dominant > FLAT_THRESHOLD) failures.push("canvas is a flat colour");
if (shape.swatch === 0) failures.push("no plain colour-swatch token drawn — the no-art path regressed");

await browser.close();
shutdown();

if (failures.length > 0) {
  console.error(`\n${failures.length} failure(s):`);
  for (const failure of failures) console.error(`  ✗ ${failure}`);
  process.exit(1);
}
console.log("\n✓ token art renders, and the no-art path still works");
