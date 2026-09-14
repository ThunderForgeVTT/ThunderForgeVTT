import path from "node:path";
import { defineConfig, devices } from "@playwright/test";

/**
 * Journeys (spec 051 T066): real operator and player flows, through the UI,
 * on an instance of their own. A separate config for the reason
 * `playwright.torture.config.ts` gives, and `playwright.config.ts` ignores
 * `e2e/journeys` so an ordinary run never collects one.
 *
 * Driven by `scripts/journeys.mjs` (`pnpm journeys`), which starts a throwaway
 * Postgres, RustFS and Mailpit, a backend and Vite against them, and only then
 * invokes this. Unlike the torture and playtest configs, this one refuses to
 * run without that script rather than falling back to :5173: a journey signs an
 * operator in and pauses worlds, and doing that to whatever answers on the dev
 * port is doing it to your dev database.
 */
if (process.env.THUNDERFORGE_JOURNEYS_STACK !== "1") {
  throw new Error(
    "Journeys run on a stack of their own. Use `pnpm journeys` " +
      "(scripts/journeys.mjs); running this config directly would point it " +
      "at whatever is on :5173, which is usually your dev database.",
  );
}

/** Per run, from the script, so two overlapping runs keep separate evidence. */
const resultsDir =
  process.env.THUNDERFORGE_JOURNEYS_RESULTS ?? "journeys-results/local";

export default defineConfig({
  testDir: "./e2e/journeys",
  testMatch: /.*\.journey\.spec\.ts/,

  // One at a time. A journey is a whole table — a Game Master, a player, an
  // operator — against one instance, and some of what it touches is
  // instance-wide; two at once would read each other's pauses.
  fullyParallel: false,
  workers: 1,

  // No retries. A journey that passes on its second attempt has hidden the
  // thing a person would have hit on their first.
  retries: 0,

  // The suite's own seeding and second-factor enrolment, unchanged. The script
  // points it at this run's Postgres container and demo directory.
  globalSetup: "./e2e/fixtures/global-setup.ts",

  // `list` for the person watching; `json` for `scripts/journeys.mjs`, which
  // judges the run from the report as well as the exit code. The path comes
  // from `PLAYWRIGHT_JSON_OUTPUT_NAME`.
  reporter: [["list"], ["json"]],
  outputDir: path.join(resultsDir, "artifacts"),

  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL,
    // The main config's reasoning: an action that waits forever reports the
    // wrong thing, slowly.
    actionTimeout: 30_000,
    // On failure only, and always kept then. With `retries: 0` the main
    // config's `on-first-retry` would never record anything.
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "off",
    launchOptions: {
      // Same GPU flags as the main config: without them Bevy falls back to
      // SwiftShader and a table with two playfields spends its budget waiting
      // on frames.
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

  // None. The stack is started, and waited for, by `scripts/journeys.mjs`.
});
