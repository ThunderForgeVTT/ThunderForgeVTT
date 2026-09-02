import { chromium, type FullConfig } from "@playwright/test";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { login } from "./helpers";

// Per-shard when sharding, so parallel stacks do not race on one file.
//
// `scripts/e2e-parallel.mjs` runs several Playwright processes at once, each
// against its own stack. They all run this global setup, and they would all
// write the same `storage-state.json` — a session belonging to whichever
// backend won the race, handed to every shard. The state is only valid
// against the stack that minted it, so the losers authenticate as nobody and
// fail in ways that have nothing to do with what they are testing.
export const DEMO_DIR =
  process.env.THUNDERFORGE_E2E_DEMO_DIR ?? path.join(__dirname, ".demo");
export const DEMO_STATE_PATH = path.join(DEMO_DIR, "storage-state.json");
export const DEMO_WORLD_PATH = path.join(DEMO_DIR, "world.json");

const SEED_SQL_PATH = path.join(
  __dirname,
  "../../../../src/server/seeds/e2e_demo.sql",
);

export const DEMO_USER = {
  identifier: "e2edemo",
  password: "Sup3r-Secret-Passphrase!",
};

export interface DemoWorld {
  worldId: string;
  worldName: string;
}

const DEMO_WORLD_ID = "00000000-0000-0000-0000-0000000000f0";
const DEMO_WORLD_NAME = "Genie Demo World";

/** Applies src/server/seeds/e2e_demo.sql against the dev Postgres
 * container. Idempotent — safe to run every test run, including against
 * a database that was just `docker compose down -v`'d and re-migrated. */
function applySeedSql(): void {
  const sql = fs.readFileSync(SEED_SQL_PATH, "utf-8");
  const containerName =
    process.env.THUNDERFORGE_POSTGRES_CONTAINER ?? "thunderforge-postgres";
  const dbName = process.env.THUNDERFORGE_DB_NAME ?? "thunderforge";
  const dbUser = process.env.THUNDERFORGE_DB_USER ?? "postgres";
  execFileSync(
    "docker",
    ["exec", "-i", containerName, "psql", "-U", dbUser, "-d", dbName],
    {
      input: sql,
      stdio: ["pipe", "inherit", "inherit"],
    },
  );
}

/**
 * Seeds one demo user + demo world (on the "genie" game system) via the
 * SQL seed file, then logs in once via the real UI to capture a reusable
 * storageState. Specs that just need to launch straight into the play
 * engine should use the `demoWorld` fixture (./demo-world) instead of
 * registering their own user.
 */
export default async function globalSetup(config: FullConfig): Promise<void> {
  fs.mkdirSync(DEMO_DIR, { recursive: true });

  applySeedSql();

  const baseURL =
    config.projects[0]?.use?.baseURL ??
    process.env.PLAYWRIGHT_BASE_URL ??
    "http://localhost:5173";

  const browser = await chromium.launch();
  const context = await browser.newContext({ baseURL });
  const page = await context.newPage();

  await login(page, DEMO_USER.identifier, DEMO_USER.password);
  await page.waitForURL(/\/welcome$/, { timeout: 15_000 });

  await context.storageState({ path: DEMO_STATE_PATH });
  fs.writeFileSync(
    DEMO_WORLD_PATH,
    JSON.stringify(
      {
        worldId: DEMO_WORLD_ID,
        worldName: DEMO_WORLD_NAME,
      } satisfies DemoWorld,
      null,
      2,
    ),
  );

  await browser.close();
}
