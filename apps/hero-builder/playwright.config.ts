import { defineConfig, devices } from "@playwright/test";

/**
 * The hero builder's own suite, against a production build of this app.
 *
 * Not part of `scripts/e2e-parallel.mjs`: that runner's suites are rooted at
 * `apps/web` and every lane stands up a database, a bucket and a backend,
 * none of which this app uses. Here there is no global setup and no stack —
 * `vite preview` serves the built page and that is the whole world.
 */
const port = Number(process.env.HERO_BUILDER_PREVIEW_PORT ?? 5191);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./e2e",
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL,
    actionTimeout: 15_000,
    trace: "retain-on-failure",
    permissions: ["clipboard-read", "clipboard-write"],
    acceptDownloads: true,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "pnpm run preview",
    url: baseURL,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
