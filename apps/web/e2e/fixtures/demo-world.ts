import { test as base, expect } from "@playwright/test";
import fs from "node:fs";
import { DEMO_STATE_PATH, DEMO_WORLD_PATH, type DemoWorld } from "./global-setup";

function readDemoWorld(): DemoWorld {
  if (!fs.existsSync(DEMO_WORLD_PATH)) {
    throw new Error(
      `Demo world seed not found at ${DEMO_WORLD_PATH} — did global-setup.ts run? ` +
        `(playwright.config.ts's globalSetup must point at ./e2e/fixtures/global-setup.ts)`,
    );
  }
  return JSON.parse(fs.readFileSync(DEMO_WORLD_PATH, "utf-8")) as DemoWorld;
}

/**
 * A pre-authenticated, pre-seeded demo user + world, ready to launch
 * straight into the play engine — no per-test registration/world-creation
 * needed. Use this for specs that exercise the engine itself rather than
 * onboarding/world-creation flows.
 */
export const test = base.extend<{ demoWorld: DemoWorld }>({
  storageState: async ({}, use) => {
    await use(DEMO_STATE_PATH);
  },
  demoWorld: async ({}, use) => {
    await use(readDemoWorld());
  },
});

export { expect };
