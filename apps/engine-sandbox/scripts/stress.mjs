#!/usr/bin/env node
/**
 * Headless stress runs with budgets.
 *
 * `pnpm -F @thunderforge/engine-sandbox stress`
 *
 * Runs every load scenario against a real browser and a real GPU, samples
 * frame timing, and fails if any scenario breaches its budget. The budgets are
 * deliberately per-scenario rather than one global number: 500 tokens and 128
 * shadow-casting lights are not expected to cost the same, and a single
 * threshold would either be too loose to catch the cheap cases or too tight to
 * pass the expensive ones.
 *
 * The numbers are machine-dependent — this is a regression tracker, not a
 * benchmark to compare across hardware. What matters is that a change which
 * makes a scenario twice as slow fails here instead of being noticed at a
 * table months later.
 *
 * `--json` emits machine-readable results for tracking over time.
 */

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "@playwright/test";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "..");
const PORT = 5182;
const BASE = `http://localhost:${PORT}`;
const asJson = process.argv.includes("--json");
/// Ramping takes minutes, so it is opt-in: `--ramp`.
const doRamp = process.argv.includes("--ramp");

/**
 * Per-scenario budgets: p95 frame time in milliseconds.
 *
 * p95 rather than mean — a scene that averages 60fps but stalls 200ms whenever
 * a light moves feels broken while averaging fine.
 *
 * 33.3ms is 30fps, the floor for something that still feels interactive.
 * Scenarios expected to be genuinely heavy get more room, but not unlimited:
 * the point is to notice when one gets *worse*.
 */
const BUDGETS_MS = {
  "tokens-50": 20,
  "tokens-200": 25,
  "tokens-500": 33,
  "lights-8": 20,
  "lights-32": 25,
  "lights-128": 40,
  "walls-200": 20,
  "walls-800": 25,
  "pitched-battle": 40,
};

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
      if ((await fetch(BASE)).ok) return;
    } catch {
      /* not up yet */
    }
    await new Promise((r) => setTimeout(r, 300));
  }
  throw new Error("sandbox dev server did not start");
}

await waitForServer();

// Headed, so the GPU is real. Headless Chromium falls back to SwiftShader
// (CPU rendering), which produces numbers that say nothing about how the
// engine performs for an actual player.
const browser = await chromium.launch({ headless: false });
const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });

const failures = [];
const results = [];

try {
  await page.goto(BASE);
  await page.waitForFunction(() => "__stress" in window, { timeout: 90_000 });
  // The wasm engine is large; let it boot and present before loading it up.
  await page.waitForTimeout(20_000);

  // A real battle map, so texture and background costs are in the sample.
  await page.evaluate(() => window.__stress.loadMap("grassy-path-ambush"));
  await page.waitForTimeout(6_000);

  const scenarios = await page.evaluate(() => window.__stress.scenarios);

  for (const scenario of scenarios) {
    const stats = await page.evaluate(
      (name) => window.__stress.runScenario(name),
      scenario.name,
    );
    const budget = BUDGETS_MS[scenario.name] ?? 33;
    const over = stats.p95Ms > budget;

    results.push({ ...scenario, ...stats, budgetMs: budget, withinBudget: !over });
    if (over) failures.push(`${scenario.name}: p95 ${stats.p95Ms.toFixed(1)}ms > ${budget}ms`);

    if (!asJson) {
      console.log(
        `${over ? "✗" : "✓"} ${scenario.name.padEnd(16)} ${scenario.magnitude.padEnd(14)} ` +
          `${stats.fps.toFixed(0).padStart(3)}fps  ` +
          `mean ${stats.meanMs.toFixed(1).padStart(5)}ms  ` +
          `p95 ${stats.p95Ms.toFixed(1).padStart(5)}ms / ${budget}ms  ` +
          `worst ${stats.worstMs.toFixed(0).padStart(4)}ms  ` +
          `hitches ${stats.hitches}`,
      );
    }
  }
  // Capacity ramp. Fixed-load scenarios above cannot distinguish a lightly
  // loaded GPU from a nearly saturated one — on any machine that hits vsync
  // they all report the same 16.7ms. Ramping until the budget breaks turns
  // that into a number that actually moves when a cost per token, per light or
  // per wall changes.
  if (doRamp) {
    if (!asJson) console.log("\nCapacity (load at which p95 exceeds 33ms):");
    const axes = await page.evaluate(() => window.__stress.rampAxes);
    for (const axis of axes) {
      const result = await page.evaluate((a) => window.__stress.rampAxis(a, 33), axis);
      results.push({ name: `ramp-${axis}`, ...result });
      if (!asJson) {
        const limit = result.brokeAt === null
          ? `>= ${result.capacity} (never broke)`
          : `${result.capacity} (broke at ${result.brokeAt})`;
        console.log(
          `  ${axis.padEnd(8)} ${String(limit).padEnd(28)} ` +
            `p95 at capacity ${result.p95AtCapacity.toFixed(1)}ms`,
        );
      }
    }
  }
} finally {
  await browser.close();
  shutdown();
}

if (asJson) {
  console.log(JSON.stringify({ results, failures }, null, 2));
} else if (failures.length > 0) {
  console.error(`\n${failures.length} scenario(s) over budget:`);
  for (const failure of failures) console.error(`  ${failure}`);
} else {
  console.log(`\nAll ${results.length} scenarios within budget.`);
}

process.exit(failures.length > 0 ? 1 : 0);
