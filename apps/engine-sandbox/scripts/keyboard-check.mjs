/**
 * Verifies the canvas keyboard-routing fix in a real browser.
 *
 * Three assertions, in order:
 *  1. Without routing, keys pressed while the canvas lacks focus do nothing
 *     (reproduces the bug).
 *  2. With routing installed, the same keys move the player token.
 *  3. With routing installed and a text field focused, they do nothing again
 *     (typing in chat must not walk a token across the map).
 *
 * Movement is observed by screenshot digest, not gl.readPixels — wgpu does
 * not preserve the drawing buffer, so readPixels is unreliable here.
 */
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { launchGpuBrowser } from "./browser.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = "/home/mbruno/development/thunderforge/ThunderForgeVTT/apps/engine-sandbox";
const PORT = 5182;
const BASE = `http://localhost:${PORT}`;

const server = spawn("pnpm", ["exec", "vite", "--port", String(PORT)], {
  cwd: ROOT, stdio: "ignore", detached: true,
});
const shutdown = () => { try { process.kill(-server.pid, "SIGTERM"); } catch {} };
process.on("exit", shutdown);

async function waitForServer(timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try { if ((await fetch(BASE)).ok) return; } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error("sandbox dev server did not start");
}

/**
 * Centre of luminance of the rendered stage, in pixels.
 *
 * Reported per axis on purpose. A single row-major index collapses both
 * axes into one number where a vertical move counts for `width` times more
 * than a horizontal one — a real sideways movement then reads as noise
 * against any threshold tuned on vertical movement.
 */
async function digest(page) {
  const png = await page.locator("#stage").screenshot();
  return page.evaluate(async (base64) => {
    const blob = await (await fetch(`data:image/png;base64,${base64}`)).blob();
    const bitmap = await createImageBitmap(blob);
    const surface = new OffscreenCanvas(bitmap.width, bitmap.height);
    const ctx = surface.getContext("2d");
    ctx.drawImage(bitmap, 0, 0);
    const { data, width } = ctx.getImageData(0, 0, bitmap.width, bitmap.height);
    let sum = 0, wx = 0, wy = 0;
    for (let i = 0; i < data.length; i += 4) {
      const lum = data[i] + data[i + 1] + data[i + 2];
      const pixel = i / 4;
      sum += lum;
      wx += lum * (pixel % width);
      wy += lum * Math.floor(pixel / width);
    }
    sum = Math.max(sum, 1);
    return { x: wx / sum, y: wy / sum };
  }, png.toString("base64"));
}

/** Pixels the luminance centroid travelled between two samples. */
const shift = (a, b) => Math.hypot(b.x - a.x, b.y - a.y);
/** Below this the frame is unchanged; a real 800ms hold moves far more. */
const MOVED_PX = 0.5;
const fmt = (d) => `(${d.x.toFixed(2)}, ${d.y.toFixed(2)})`;

async function hold(page, code, ms = 800) {
  await page.keyboard.down(code);
  await page.waitForTimeout(ms);
  await page.keyboard.up(code);
  await page.waitForTimeout(400);
}

/** Mirrors apps/web/src/engine/canvasKeyboard.ts. */
const ROUTING = `
(() => {
  const synthetic = new WeakSet();
  const SCROLL = new Set(["ArrowUp","ArrowDown","ArrowLeft","ArrowRight","Space","PageUp","PageDown","Home","End"]);
  const TAGS = new Set(["INPUT","TEXTAREA","SELECT"]);
  const isTextEntry = (t) => t instanceof HTMLElement && (t.isContentEditable || TAGS.has(t.tagName));
  const forward = (event) => {
    if (synthetic.has(event) || isTextEntry(event.target)) return;
    const canvas = document.querySelector("canvas");
    if (!canvas || event.target === canvas || canvas.style.display === "none") return;
    const copy = new KeyboardEvent(event.type, {
      key: event.key, code: event.code, location: event.location,
      repeat: event.repeat, isComposing: event.isComposing,
      ctrlKey: event.ctrlKey, shiftKey: event.shiftKey,
      altKey: event.altKey, metaKey: event.metaKey,
      bubbles: false, cancelable: true,
    });
    synthetic.add(copy);
    canvas.dispatchEvent(copy);
    if (SCROLL.has(event.code)) event.preventDefault();
  };
  window.addEventListener("keydown", forward, { capture: true });
  window.addEventListener("keyup", forward, { capture: true });
})();
`;

await waitForServer();
const { browser } = await launchGpuBrowser(chromium);
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
await page.goto(BASE);
await page.waitForFunction(() => document.querySelectorAll("canvas").length > 0, { timeout: 60_000 });
await page.waitForTimeout(20_000);

// The sandbox autofocuses its canvas on boot, which is exactly the state
// the bug does *not* occur in. Move focus off it first, the way clicking
// any dock button or tool control does in the real app.
const blur = async () => page.evaluate(() => {
  document.getElementById("gizmos").focus();
  return document.activeElement?.tagName ?? "none";
});
console.log(`focused after blur: ${await blur()}`);

const results = [];

// 1. The bug: canvas not focused, keys go nowhere.
const before = await digest(page);
await hold(page, "KeyW");
const afterUnrouted = await digest(page);
const movedUnrouted = shift(before, afterUnrouted);
results.push([
  "1. unrouted keys are dropped",
  movedUnrouted < MOVED_PX,
  `${fmt(before)} -> ${fmt(afterUnrouted)}  moved ${movedUnrouted.toFixed(2)}px`,
]);

// 2. The fix: routing installed, focus still off the canvas, keys land.
await page.evaluate(ROUTING);
await blur();
const beforeRouted = await digest(page);
await hold(page, "KeyW");
const afterRouted = await digest(page);
const movedRouted = shift(beforeRouted, afterRouted);
results.push([
  "2. routed keys reach the engine",
  movedRouted > MOVED_PX,
  `${fmt(beforeRouted)} -> ${fmt(afterRouted)}  moved ${movedRouted.toFixed(2)}px`,
]);

// 2b. Release must actually stop it. A forwarded keydown with no matching
// keyup would leave the key stuck down in Bevy's `ButtonInput` and the
// token gliding forever, which no screenshot taken right after the release
// would catch.
const settle1 = await digest(page);
await page.waitForTimeout(1500);
const settle2 = await digest(page);
const drift = shift(settle1, settle2);
results.push([
  "2b. release stops movement",
  drift < MOVED_PX,
  `${fmt(settle1)} -> ${fmt(settle2)}  drifted ${drift.toFixed(2)}px over 1.5s`,
]);

// 3. Typing in a text field must not move anything. Uses the opposite
// direction so a clamp at the arena edge cannot pass this by accident.
// Appending the probe input shifts the page layout by a few pixels, so this
// phase's own before/after are both sampled with it already present.
await page.evaluate(() => {
  const input = document.createElement("input");
  input.id = "typing-probe";
  document.body.appendChild(input);
  input.focus();
});
const beforeTyping = await digest(page);
await hold(page, "KeyS");
const afterTyping = await digest(page);
const movedTyping = shift(beforeTyping, afterTyping);
results.push([
  "3. typing does not move tokens",
  movedTyping < MOVED_PX,
  `${fmt(beforeTyping)} -> ${fmt(afterTyping)}  moved ${movedTyping.toFixed(2)}px`,
]);

await browser.close();
shutdown();

let failed = 0;
for (const [name, ok, detail] of results) {
  console.log(`${ok ? "✓" : "✗"} ${name.padEnd(38)} centroid ${detail}`);
  if (!ok) failed += 1;
}
process.exit(failed > 0 ? 1 : 0);
