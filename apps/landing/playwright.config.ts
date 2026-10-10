import { defineConfig, devices } from "@playwright/test";

// The landing is proved as it ships: the built page behind `vite preview`.
// `pnpm run e2e` builds it first. A port of its own, beside the demo's 5193.
const port = Number(process.env.THUNDERFORGE_LANDING_E2E_PORT ?? 5211);

export default defineConfig({
  testDir: "./e2e",
  outputDir: "./test-results",
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  timeout: 60_000,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    actionTimeout: 15_000,
    screenshot: "only-on-failure",
    launchOptions: {
      env: {
        ...(process.env as Record<string, string>),
        // See `scripts/e2e-parallel.mjs`: a headless browser has no business
        // on the system bus, and aborts when it cannot get on it.
        DBUS_SYSTEM_BUS_ADDRESS:
          "unix:path=/nonexistent/thunderforge-e2e-no-dbus",
      },
    },
  },
  projects: [
    {
      name: "built",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 900 },
      },
    },
  ],
  webServer: {
    command: `pnpm exec vite preview --strictPort --port ${port}`,
    url: `http://127.0.0.1:${port}/`,
    reuseExistingServer: false,
    timeout: 60_000,
    env: { LANDING_PREVIEW_PORT: String(port) },
  },
});
