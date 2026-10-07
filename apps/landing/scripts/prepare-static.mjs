// Copies the example maps the homepage's atlas shows, with their thumbnails
// and geometry, out of the demo's generated maps, writes the atlas's index,
// and makes sure the dice crate's WebAssembly build is current.
//
// The maps are generated, not committed (`pnpm -F @thunderforge/demo run
// maps`); without them the page still builds and the map section says the
// maps live in the demo. Credit travels with the maps: MBRound18, CC BY-SA 4.0.
import { copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { ensureDiceBuild } from "../../../scripts/shared.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const source = path.resolve(here, "../../demo/public/maps");
const out = path.resolve(here, "../public/maps");

// The atlas's order, and each map's name as the demo world's scene list has
// it (apps/demo/src/seed/world.ts). The first opens on the page.
const ATLAS = [
  ["demo", "The Proving Ground"],
  ["road-side-in", "Roadside Inn"],
  ["little-fish-academy", "Little Fish Academy"],
  ["dwarven-forge", "Dwarven Forge"],
  ["grassy-path-ambush", "Grassy Path Ambush"],
  ["chamber-of-echoing-grief", "Chamber of Echoing Grief"],
  ["azheim-meeting", "Azheim Meeting"],
];

await mkdir(out, { recursive: true });
const catalog = path.join(source, "maps.json");
if (existsSync(catalog)) {
  const maps = JSON.parse(await readFile(catalog, "utf8"));
  const index = [];
  for (const [file, title] of ATLAS) {
    const map = maps.find((m) => m.name === file);
    if (!map) continue;
    await copyFile(path.join(source, `${file}.webp`), path.join(out, `${file}.webp`));
    await copyFile(path.join(source, `${file}.thumb.webp`), path.join(out, `${file}.thumb.webp`));
    const { name, width, height, gridSize, walls, lights } = map;
    await writeFile(
      path.join(out, `${file}.json`),
      JSON.stringify({ name, width, height, gridSize, walls, lights }),
    );
    const doors = walls.filter((w) => w.doorState !== "none").length;
    index.push({
      name,
      title,
      width,
      height,
      walls: walls.length - doors,
      doors,
      lights: lights.length,
    });
  }
  await writeFile(path.join(out, "index.json"), JSON.stringify(index));
} else {
  console.warn(`landing: no generated maps in ${source}; the map section has nothing to show`);
}

await ensureDiceBuild();
