import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  reporter: [["html", { open: "never" }]],
  globalSetup: "./e2e/fixtures/global-setup.ts",
  use: {
    baseURL: process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:5173",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    launchOptions: {
      // Without these the browser gets no GPU and Bevy falls back to
      // SwiftShader — the engine then runs at roughly 4fps, and tests that
      // reload the ~190MB WASM bundle several times (or open a second
      // browser context) exhaust their timeout before they can fail or
      // pass on their own merits. Measured in the engine sandbox: 240ms a
      // frame software, 16.6ms with these flags.
      args: [
        "--enable-gpu",
        "--enable-gpu-rasterization",
        // ANGLE over Vulkan is the combination that works headless on Linux.
        "--use-gl=angle",
        "--use-angle=vulkan",
        // VMs and older drivers are blocklisted by default; without this
        // the flags above are accepted and then quietly ignored.
        "--ignore-gpu-blocklist",
      ],
    },
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
  webServer: {
    command: "pnpm run dev",
    url: "http://localhost:5173",
    reuseExistingServer: true,
    timeout: 120_000,
  },
});
