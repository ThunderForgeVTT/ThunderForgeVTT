import { defineConfig, devices } from "@playwright/test";

// The landing is proved as it ships: the built page behind `vite preview`.
// `pnpm run e2e` builds it first. A port of its own, beside the demo's 5193.
const port = Number(process.env.THUNDERFORGE_LANDING_E2E_PORT ?? 5211);

export default defineConfig({
  testDir: "./e2e",
  outputDir: "./test-results",
  // Spec 090 plan: baselines are committed under `e2e/__screenshots__/`,
  // made on Linux only, so the name carries no platform.
  snapshotPathTemplate: "{testDir}/__screenshots__/{testFilePath}/{arg}-{projectName}{ext}",
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
      // The page runs the engine. Without a GPU Bevy falls back to
      // SwiftShader at a few frames a second and every engine test times
      // out. The blocklist flag stops the GPU being accepted and then quietly
      // ignored. ANGLE over Vulkan (as in `apps/web`) also reaches the GPU but
      // breaks `page.screenshot`, which the looks tests need.
      args: ["--enable-gpu", "--ignore-gpu-blocklist"],
      env: {
        ...(process.env as Record<string, string>),
        // See `scripts/e2e-parallel.mjs`: a headless browser has no business
        // on the system bus, and aborts when it cannot get on it.
        DBUS_SYSTEM_BUS_ADDRESS:
          "unix:path=/nonexistent/thunderforge-e2e-no-dbus",
      },
    },
  },
  // Spec 090: the page is proved at a phone, a tablet and a desktop width.
  // Telemetry is about events, not layout, so it runs at one of them.
  projects: [
    {
      name: "phone",
      testIgnore: /telemetry\.spec\.ts/,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 390, height: 844 },
        hasTouch: true,
      },
    },
    {
      name: "tablet",
      testIgnore: /telemetry\.spec\.ts/,
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 768, height: 1024 },
      },
    },
    {
      name: "desktop",
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
