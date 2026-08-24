import { chromium, type FullConfig } from "@playwright/test";
import fs from "node:fs";
import path from "node:path";
import { registerAndCreateWorld, uniqueSuffix } from "./helpers";

export const DEMO_DIR = path.join(__dirname, ".demo");
export const DEMO_STATE_PATH = path.join(DEMO_DIR, "storage-state.json");
export const DEMO_WORLD_PATH = path.join(DEMO_DIR, "world.json");

export interface DemoWorld {
  worldId: string;
  worldName: string;
}

/**
 * Seeds one demo user + demo world (defaulting to the "genie" game system,
 * per the server-side default in prepare_world_input) via real UI
 * registration, once per test run. Specs that just need to launch straight
 * into the play engine should use the `demoWorld` fixture (./demo-world)
 * instead of registering their own user.
 */
export default async function globalSetup(config: FullConfig): Promise<void> {
  fs.mkdirSync(DEMO_DIR, { recursive: true });

  const baseURL =
    config.projects[0]?.use?.baseURL ??
    process.env.PLAYWRIGHT_BASE_URL ??
    "http://localhost:5173";

  const browser = await chromium.launch();
  const context = await browser.newContext({ baseURL });
  const page = await context.newPage();

  const worldName = `Demo World ${uniqueSuffix()}`;
  const worldId = await registerAndCreateWorld(page, worldName, "e2edemo");

  await context.storageState({ path: DEMO_STATE_PATH });
  fs.writeFileSync(
    DEMO_WORLD_PATH,
    JSON.stringify({ worldId, worldName } satisfies DemoWorld, null, 2),
  );

  await browser.close();
}
