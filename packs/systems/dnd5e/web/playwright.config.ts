import { defineConfig, devices } from "@playwright/test";

/**
 * The 5e sheet reader's own suite (spec 048 T043): the slice's standalone
 * half. No database, bucket or backend: `vite preview` serves the harness
 * page, and the native answers come from `cargo test`, which the
 * `test:sheet-reader` script runs first.
 */
const port = Number(process.env.SHEET_READER_PREVIEW_PORT ?? 5196);
const baseURL = `http://127.0.0.1:${port}`;

export default defineConfig({
  testDir: "./e2e",
  workers: 1,
  reporter: [["list"]],
  use: { baseURL, actionTimeout: 15_000, trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "vite preview --config vite.sheet-reader.config.ts",
    url: baseURL,
    reuseExistingServer: false,
    timeout: 60_000,
  },
});
