/**
 * The demo the e2e stacks serve at `/demo` (spec 074 FR-015, SC-006).
 *
 * The backend mounts `/demo` only when it is started with `DEMO_DIR` naming a
 * directory with a built demo in it, so a stack that is never told where one
 * is answers "not found" whatever the `feature.demo` setting says — and the
 * half of SC-006 where a stranger opens the demo could not be proved at all.
 *
 * Built once per run into `data/demo`, where `pnpm -F @thunderforge/demo run
 * build` puts it, and only when nothing is there: the demo imports the web
 * app's own pages, so a build is a minute of Vite, which every slice would
 * otherwise pay. A warm checkout pays nothing. Delete `data/demo` to refresh
 * it. The maps it ships are made by the binary `ensureBaseMaps` has already
 * built, when `apps/demo/public/maps` is missing.
 */
import { existsSync } from "node:fs";
import { join } from "node:path";

import { ROOT_DIR, log, runCommand } from "../shared.mjs";

export const DEMO_DIR = join(ROOT_DIR, "data", "demo");
const DEMO_MAPS = join(ROOT_DIR, "apps", "demo", "public", "maps", "maps.json");

/** The environment every e2e backend gets for the demo. */
export function demoEnv() {
  return { DEMO_DIR };
}

export async function ensureDemoBuild() {
  if (existsSync(join(DEMO_DIR, "index.html"))) {
    log("e2e", "The demo at data/demo is reused; delete it to rebuild.");
    return;
  }
  if (!existsSync(DEMO_MAPS)) {
    await runCommand(
      "cargo run -q -p thunderforge --bin thunderforge-demo-maps --features demo-maps -- examples/maps apps/demo/public/maps",
      { name: "demo maps", prefix: "e2e" },
    );
  }
  await runCommand("pnpm -F @thunderforge/demo run build", {
    name: "build demo",
    prefix: "e2e",
  });
}
