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
    // Never reuse: the script points the stack at an ephemeral database via
    // env, and reusing a server already bound to the dev database would
    // silently run the storm against real data.
    reuseExistingServer: false,
    timeout: 180_000,
  },
});
