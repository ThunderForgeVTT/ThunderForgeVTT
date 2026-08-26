#!/usr/bin/env node
/**
 * Checks the stacked-token gestures, engine-side.
 *
 * Three tokens on one square — the case that used to be unreachable,
 * because the hit test took the first token it found and stopped. Asserts:
 *
 *  1. A single click selects the whole stack, not just the top token.
 *  2. Dragging that selection moves every member.
 *  3. Repeated clicks keep reporting the same stack, in the same order —
 *     what the frontend's double-click picker renders.
 *
 * Double-click itself is NOT checked here, because it is not the engine's
 * job: two clicks that fast routinely land in one Bevy frame, where
 * `just_pressed` sees a single press and the second is lost. Detection
 * lives in the DOM (`WorldPage`'s `dblclick` handler) over the stack the
 * preceding click already selected.
 *
 * Assertions read the engine's emitted events, not pixels: selection has no
 * visual signature a screenshot can distinguish from three tokens sitting
 * still.
 *
 * Usage: pnpm -F @thunderforge/engine-sandbox stacking
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { assertGpuRendering, launchGpuBrowser } from "./browser.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const PORT = 5189;
const BASE = `http://localhost:${PORT}`;

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
await page.goto(BASE);
await page.waitForFunction(() => document.querySelectorAll("canvas").length > 0, { timeout: 60_000 });
await page.waitForTimeout(20_000);
await assertGpuRendering(page, "stacking-check");

const canvas = page.locator("canvas");
const box = await canvas.boundingBox();
// World origin sits at the canvas centre at the default camera.
const cx = box.x + box.width / 2;
const cy = box.y + box.height / 2;

await page.evaluate(() => {
  window.__stress.stackTokens(3, 0, 0);
});
await page.waitForTimeout(1500);

const failures = [];
const check = (name, ok, detail) => {
  console.log(`${ok ? "✓" : "✗"} ${name}${detail ? ` — ${detail}` : ""}`);
  if (!ok) failures.push(name);
};

const events = () => page.evaluate(() => window.__stress.engineEvents());
const clearEvents = () => page.evaluate(() => window.__stress.clearEngineEvents());

// Control: does a *single* token drag emit anything in this harness at
// all? The sandbox leaves every authoring tool live, so a click can also
// create a light or a shape, and a stacking failure has to be told apart
// from that interference.
await page.evaluate(() => {
  window.__stress.clearEngineEvents();
  window.__stress.stackTokens(1, -300, 200, "control");
});
await page.waitForTimeout(800);
await clearEvents();
await page.mouse.move(cx - 300, cy - 200);
await page.mouse.down();
await page.mouse.move(cx - 260, cy - 160, { steps: 8 });
await page.mouse.up();
await page.waitForTimeout(800);
const controlEvents = await events();
check(
  "control: a lone token still drags",
  controlEvents.some((e) => e.type === "upsert_token"),
  "single-token drag reported a new position",
);

// 1. One click takes the whole stack.
await clearEvents();
await page.mouse.move(cx, cy);
await page.mouse.down();
await page.waitForTimeout(120);
await page.mouse.up();
await page.waitForTimeout(600);

const afterClick = await events();
const selectTokens = afterClick.filter((e) => e.type === "select_tokens").pop();
const stacked = selectTokens?.tokenIds ?? [];
check(
  "a single click selects the whole stack",
  stacked.length === 3,
  `select_tokens carried ${stacked.length} id(s)`,
);

// 2. Dragging moves every member.
await clearEvents();
await page.mouse.move(cx, cy);
await page.mouse.down();
await page.waitForTimeout(200);
await page.mouse.move(cx + 180, cy - 120, { steps: 12 });
await page.waitForTimeout(200);
await page.mouse.up();
await page.waitForTimeout(800);

const dragEvents = await events();
const moved = new Set(
  dragEvents
    .filter((e) => e.type === "upsert_token")
    .map((e) => e.token?.id),
);
check(
  "dragging the stack moves every member",
  moved.size === 3,
  `${moved.size} token(s) reported a new position`,
);

// 3. The stack a picker would render is the same one, in the same order,
// on every click — an order that reshuffled between the click and the reach
// would make the picker unusable.
// Where the tokens actually came to rest, not where the pointer was let
// go: they snap to the grid on drop, which can move them most of a cell.
const landed = dragEvents.filter((e) => e.type === "upsert_token").pop()?.token;
const dragged = landed
  ? { x: cx + landed.x, y: cy - landed.y }
  : { x: cx + 180, y: cy - 120 };
await clearEvents();
const stackFrom = async () => {
  await page.mouse.move(dragged.x, dragged.y);
  await page.mouse.down();
  await page.waitForTimeout(120);
  await page.mouse.up();
  await page.waitForTimeout(500);
  const seen = await events();
  return seen.filter((e) => e.type === "select_tokens").pop()?.tokenIds ?? [];
};

const first = await stackFrom();
await clearEvents();
const second = await stackFrom();

check(
  "the stack is reported identically on repeat clicks",
  first.length === 3 && JSON.stringify(first) === JSON.stringify(second),
  `${JSON.stringify(first)} then ${JSON.stringify(second)}`,
);

await browser.close();
shutdown();

if (failures.length > 0) {
  console.error(`\n${failures.length} failure(s)`);
  process.exit(1);
}
console.log("\n✓ stacked-token gestures behave");
