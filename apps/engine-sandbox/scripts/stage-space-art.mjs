#!/usr/bin/env node
/**
 * Copies `examples/space/` into `public/assets/space/` so the sandbox can
 * serve it as plain static files.
 *
 * Separate from `extract-maps.mjs` because nothing needs extracting: these
 * are already ordinary PNGs, not base64 payloads inside a `.dd2vtt`. They
 * land under the same "assets" root, so Bevy reaches them as
 * `space/backgrounds/nebula17.png` and friends.
 *
 * The art is CC-BY-SA 3.0 from the FreeOrion project — see
 * `examples/space/README.md` for the attribution that has to travel with
 * it.
 */

import { cpSync, mkdirSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SOURCE_DIR = path.resolve(__dirname, "../../../examples/space");
const OUT_DIR = path.resolve(__dirname, "../public/assets/space");

mkdirSync(OUT_DIR, { recursive: true });

for (const entry of readdirSync(SOURCE_DIR, { withFileTypes: true })) {
  // The README stays in the repo; it is documentation, not an asset.
  if (entry.isFile() && entry.name.endsWith(".md")) continue;
  cpSync(path.join(SOURCE_DIR, entry.name), path.join(OUT_DIR, entry.name), {
    recursive: true,
  });
}

const count = (dir) =>
  readdirSync(dir, { withFileTypes: true, recursive: true }).filter((e) =>
    e.isFile(),
  ).length;

console.log(`staged ${count(OUT_DIR)} space art files → public/assets/space/`);
