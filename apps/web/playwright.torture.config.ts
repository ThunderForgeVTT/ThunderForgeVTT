import { defineConfig, devices } from "@playwright/test";

/**
 * Torture / load runs. Deliberately a separate config, not a tag in the main
 * one.
 *
 * The default `playwright.config.ts` has `testDir: "./e2e"`, which would pick
 * these up on every ordinary run — and a 100-session storm is not something
 * anyone should trigger by typing `playwright test`. Separating the config
 * makes running them an explicit act rather than an accident, and lets them
 * have their own output directory that can be deleted wholesale.
 *
 * Driven by `scripts/torture.mjs`. Running this config directly points it at
 * whatever stack is already on :5173, which is usually your dev database —
 * use the script.
 */
export default defineConfig({
  testDir: "./e2e/torture",
  testMatch: /.*\.torture\.spec\.ts/,

  // One at a time, always. These tests exist to saturate a shared server;
  // running two concurrently measures them competing with each other rather
  // than the server's actual ceiling.
  fullyParallel: false,
  workers: 1,

  // No retries. A retry on a load test hides exactly the intermittent
  // failure the run was looking for.
  retries: 0,

  reporter: [
    ["list"],
    ["json", { outputFile: "torture-results/results.json" }],
  ],
  outputDir: "torture-results/artifacts",

  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:5173",
    trace: "off",
    screenshot: "off",
    video: "off",
    launchOptions: {
      // Same GPU flags as the main config: without them Bevy falls back to
      // SwiftShader at ~240ms/frame, and the GM page driving the mutation
      // storm becomes the bottleneck instead of the server.
      args: [
        "--enable-gpu",
        "--enable-gpu-rasterization",
        "--use-gl=angle",
        "--use-angle=vulkan",
        "--ignore-gpu-blocklist",
      ],
    },
  },

  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],

  webServer: {
    command: "pnpm run dev",
    url: "http://localhost:5173",
    // This starts **vite only** — `apps/web`'s own `dev` script is one word.
    // The backend is started by `scripts/torture.mjs`, against the ephemeral
    // database, before Playwright is invoked at all.
    //
    // That division is not cosmetic. This flag governs vite and nothing else,
    // so on its own it never prevented a storm from reaching the dev backend
    // through vite's `/api` proxy — which is what happened on every run until
    // the script began starting a backend of its own.
    reuseExistingServer: false,
    timeout: 180_000,
    // Forward server output. Playwright hides webServer logs unless startup
    // fails, which is the opposite of what a load run needs: when subscribers
    // report missing events, the only way to tell "the server never sent it"
    // from "the client never got it" is the server's own delivery log.
    stdout: "pipe",
    stderr: "pipe",
  },
});
