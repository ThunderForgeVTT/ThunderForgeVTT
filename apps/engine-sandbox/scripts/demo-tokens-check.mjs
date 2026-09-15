/**
 * The sandbox still has its demo tokens, and its buttons still reach them.
 *
 * Owner decision 2026-09-15: the engine's red "player" and blue "npc" squares
 * are the sandbox's alone. The engine used to spawn them at startup in every
 * session, a real world's included; now it spawns them only when sent
 * `spawn_demo_tokens`, which this page sends and `apps/web` never does
 * (`apps/web/e2e/world-session-tokens.spec.ts` proves that half).
 *
 * Asked of the engine, not read from pixels:
 *  1. both demo tokens exist, the red left of the origin and the blue right;
 *  2. a size button resizes both (`set_token_grid`);
 *  3. the darkvision checkbox reaches the player token (`set_token_vision`).
 */
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";
import { launchGpuBrowser } from "./browser.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
// This checkout's sandbox, wherever it is — not a fixed path.
const ROOT = path.resolve(__dirname, "..");
const PORT = 5186;
const BASE = `http://localhost:${PORT}`;

const server = spawn("pnpm", ["exec", "vite", "--port", String(PORT), "--strictPort"], {
  cwd: ROOT,
  stdio: "ignore",
  detached: true,
});
const shutdown = () => {
  try {
    process.kill(-server.pid, "SIGTERM");
  } catch {}
};
process.on("exit", shutdown);

async function waitForServer(timeoutMs = 60_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      if ((await fetch(BASE)).ok) return;
    } catch {}
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error("sandbox dev server did not start");
}

async function poll(read, ok, timeoutMs = 20_000) {
  const deadline = Date.now() + timeoutMs;
  let last;
  while (Date.now() < deadline) {
    last = await read();
    if (ok(last)) return { ok: true, last };
    await new Promise((r) => setTimeout(r, 250));
  }
  return { ok: false, last };
}

await waitForServer();
const { browser } = await launchGpuBrowser(chromium);
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
await page.goto(BASE);
await page.waitForFunction(() => window.__sandbox?.tokenFootprints, null, {
  timeout: 120_000,
});

// The probes exist before the wasm has finished initialising, and throw until
// it has; an engine not up yet reads as nothing drawn.
const footprints = () =>
  page.evaluate(() => {
    try {
      return JSON.parse(window.__sandbox.tokenFootprints());
    } catch {
      return [];
    }
  });
const demo = (list) =>
  Object.fromEntries(
    list
      .filter((t) => t.tokenId === "player" || t.tokenId === "npc")
      .map((t) => [t.tokenId, t]),
  );

const results = [];

// Spawned at (-180, 0) and (180, 0), then snapped to the sandbox's grid, so
// asked only which side of the origin each stands.
const spawned = await poll(footprints, (list) => {
  const d = demo(list);
  return d.player?.x < 0 && d.npc?.x > 0;
}, 60_000);
results.push([
  "1. both demo tokens are on the board",
  spawned.ok,
  JSON.stringify(demo(spawned.last ?? [])),
]);

await page.getByRole("button", { name: "Large (2)" }).click();
const resized = await poll(footprints, (list) => {
  const d = demo(list);
  return d.player?.footprint === 2 && d.npc?.footprint === 2;
});
results.push([
  "2. a size button resizes both",
  resized.ok,
  JSON.stringify(demo(resized.last ?? [])),
]);

// Vision is only resolved, and so only reported, when there is darkness to
// resolve it against.
await page.getByRole("button", { name: "ambient: dark" }).click();
await page.locator("#darkvision").check();
const seeing = await poll(
  () =>
    page.evaluate(() => {
      try {
        return JSON.parse(window.__sandbox.tokenVision());
      } catch {
        return {};
      }
    }),
  (vision) => vision.player === 600,
);
results.push([
  "3. darkvision reaches the player token",
  seeing.ok,
  JSON.stringify(seeing.last),
]);

await browser.close();
shutdown();

let failed = 0;
for (const [name, ok, detail] of results) {
  console.log(`${ok ? "✓" : "✗"} ${name.padEnd(40)} ${detail}`);
  if (!ok) failed += 1;
}
process.exit(failed > 0 ? 1 : 0);
