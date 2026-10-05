import { defineConfig, devices } from "@playwright/test";

// The demo is proved as it ships: the built bundle, behind a static file
// server and nothing else. `pnpm run e2e` builds it first.
const port = Number(process.env.THUNDERFORGE_DEMO_E2E_PORT ?? 5193);

export default defineConfig({
  testDir: "./e2e",
  outputDir: "./test-results",
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  reporter: [["list"]],
  timeout: 180_000,
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    actionTimeout: 15_000,
    screenshot: "only-on-failure",
    launchOptions: {
      // The same flags the web suite launches with, for the same reason:
      // without a GPU the engine runs at a few frames a second.
      args: [
        "--enable-gpu",
        "--enable-gpu-rasterization",
        "--use-gl=angle",
        "--use-angle=vulkan",
        "--ignore-gpu-blocklist",
      ],
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
    command: "pnpm exec vite preview --strictPort",
    url: `http://127.0.0.1:${port}/demo/`,
    reuseExistingServer: false,
    timeout: 60_000,
    env: { THUNDERFORGE_DEMO_PORT: String(port) },
  },
});
