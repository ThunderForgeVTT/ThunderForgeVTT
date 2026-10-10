/**
 * The base maps the e2e stacks offer (spec 088 US2).
 *
 * Built once per run by the same binary `make base-maps` runs, into the same
 * `target/base-maps`, and only when something it is made from is newer than
 * the `maps.json` already there: the example maps themselves, the importer
 * that turns them into scenes, and the binary that writes them down. A warm
 * checkout pays nothing; a change to the importer (spec 088 T079's edge
 * walls, say) rebuilds them before any stack starts.
 *
 * Every stack is also told to make no map the default
 * (`THUNDERFORGE_BASE_MAPS_DEFAULT=none`, see `base_maps::NO_DEFAULT`). The
 * specs make hundreds of worlds and were written against a blank Starting
 * Scene; a map on each would put walls and lights under every one of them.
 * The maps are still listed and still chosen by the specs that test them.
 */
import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

import { ROOT_DIR, runCommand } from "../shared.mjs";

export const BASE_MAPS_DIR = join(ROOT_DIR, "target", "base-maps");

const SOURCES = [
  join(ROOT_DIR, "examples", "maps"),
  join(ROOT_DIR, "crates", "thunderforge-server", "src", "map_import"),
  join(ROOT_DIR, "apps", "thunderforge", "src", "bin", "demo_maps.rs"),
];

/** The environment every e2e backend gets for base maps. */
export function baseMapsEnv() {
  return {
    THUNDERFORGE_BASE_MAPS_DIR: BASE_MAPS_DIR,
    THUNDERFORGE_BASE_MAPS_DEFAULT: "none",
  };
}

export async function ensureBaseMaps() {
  const listing = join(BASE_MAPS_DIR, "maps.json");
  const notice = join(BASE_MAPS_DIR, "NOTICE.txt");
  if (
    existsSync(listing) &&
    existsSync(notice) &&
    statSync(listing).mtimeMs >= Math.max(...SOURCES.map(newest))
  ) {
    return;
  }
  await runCommand(
    "cargo run -q -p thunderforge --bin thunderforge-demo-maps --features demo-maps -- examples/maps target/base-maps",
    { name: "base maps", prefix: "e2e" },
  );
}

/** The newest modification time under `path`, or 0 when it is not there. */
function newest(path) {
  if (!existsSync(path)) return 0;
  const stat = statSync(path);
  if (!stat.isDirectory()) return stat.mtimeMs;
  let latest = stat.mtimeMs;
  for (const entry of readdirSync(path)) {
    latest = Math.max(latest, newest(join(path, entry)));
  }
  return latest;
}
