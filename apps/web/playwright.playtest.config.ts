import { defineConfig, devices } from "@playwright/test";

/**
 * Playtests: a Game Master and players, driven through a whole session in the
 * engine, recorded so a person can watch it back.
 *
 * A separate config for the same reason `playwright.torture.config.ts` is one.
 * The default config's `testDir` is `./e2e`, and `scripts/e2e-parallel.mjs`
 * walks only `apps/web/e2e`, so nothing under `./playtest` runs unless someone
 * asks for it. These are slow, they record video of three browsers at once,
 * and a scenario that finds something is doing its job — none of which belongs
 * in the suite that gates a change.
 *
 * Run through `pnpm playtest` (`scripts/e2e-parallel.mjs --suite=playtest`),
 * which gives the run a stack of its own. Running this config directly points
 * it at whatever answers on :5173, which is usually your dev database.
 */
export default defineConfig({
  testDir: "./playtest",
  testMatch: /.*\.playtest\.ts$/,

  // One scenario at a time. Each already runs three browsers against one
  // engine-heavy scene, and the recording is only worth watching if nothing
  // else is competing for the GPU while it is made.
  fullyParallel: false,
  workers: 1,
  // A playtest that passes on its second attempt has hidden what it found.
  retries: 0,

  reporter: [
    ["list"],
    ["html", { outputFolder: "playtest-report", open: "never" }],
  ],
  outputDir: "playtest-results",

  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:5173",
    // Everything on: the point of the run is to be watched afterwards. Video
    // covers the page fixture; the table fixture records the contexts it
    // opens for players itself, since `video` only reaches Playwright's own.
    video: "on",
    screenshot: "on",
    // Off, and measured: `on` recorded every action of three browser contexts
    // and produced a 307MB archive per scenario — a 614MB report for a
    // two-scenario run, which is not something anyone opens. The recordings
    // and the per-step screenshots are what a playtest is watched through;
    // turn this back on for one run when a specific step needs picking apart.
    trace: "off",
    viewport: { width: 1280, height: 800 },
    actionTimeout: 30_000,
    // The same GPU flags as the main config, for the same reason: without
    // them Bevy falls back to SwiftShader at a few frames a second, and a
    // three-client session spends its whole budget waiting on frames.
    launchOptions: {
      args: [
        "--enable-gpu",
        "--enable-gpu-rasterization",
        "--use-gl=angle",
        "--use-angle=vulkan",
        "--ignore-gpu-blocklist",
      ],
    },
  },

  // Named `chromium` because `scripts/e2e-parallel.mjs` selects the project by
  // that name for every lane it runs.
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
