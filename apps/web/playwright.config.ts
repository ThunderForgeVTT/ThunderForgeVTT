import { defineConfig, devices } from "@playwright/test";

// One place, so the server Playwright probes is always the server it talks to.
//
// These were two literals naming port 5173 independently. Under
// `scripts/e2e-parallel.mjs` each shard runs against its own stack on its own
// port, and a hardcoded probe URL means every shard checks 5173 — reusing a
// stack it is not testing, or starting one that collides with the others.
const baseURL = process.env.PLAYWRIGHT_BASE_URL ?? "http://localhost:5173";

// `e2e-parallel.mjs` builds, migrates, seeds and starts every stack itself,
// then waits for each to answer before a shard runs. Playwright must not also
// try: `reuseExistingServer` would have it adopt whatever is on that port, and
// on a free port it would start a `pnpm run dev` bound to the config's own
// defaults rather than to this shard's database and bucket.
const stackIsExternal = process.env.THUNDERFORGE_E2E_EXTERNAL_STACK === "1";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  reporter: [["html", { open: "never" }]],
  globalSetup: "./e2e/fixtures/global-setup.ts",
  use: {
    baseURL,
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
  webServer: stackIsExternal ? undefined : {
    command: "pnpm run dev",
    url: baseURL,
    reuseExistingServer: true,
    timeout: 120_000,
    env: {
      // Every auth route is rate limited per IP per path: 15/min for
      // login and register, 40/min for the rest — including
      // `/authentication/setup/status`, which `App.tsx` calls on *every*
      // page load and which gates the whole router. A suite that
      // registers a throwaway user per test blows through both budgets,
      // and the symptom is not a 429 anywhere in the test output: it is a
      // form that never renders, because the app replaced itself with
      // "ThunderForge could not load the current instance state".
      //
      // The server already provides exactly this escape hatch for exactly
      // this reason (`auth_middleware.rs` — debug builds only, compiled
      // out of a release binary, announced loudly in the log), and
      // `scripts/torture.mjs` already sets it. The e2e harness is the same
      // kind of harness and needs it for the same reason.
      //
      // Note this only reaches a dev stack Playwright itself starts:
      // `reuseExistingServer` means an already-running stack keeps
      // whatever environment it was launched with.
      THUNDERFORGE_DISABLE_AUTH_RATE_LIMIT: "1",
    },
  },
});
