#!/usr/bin/env node
/**
 * Turns `examples/maps/*.dd2vtt` into plain files the sandbox can serve.
 *
 * A `.dd2vtt` is JSON with one huge base64 image plus geometry. The app
 * normally routes that image through upload → RustFS → an authenticated
 * `/api/canvas-assets/{id}` proxy. The whole point of the sandbox is to cut
 * that path out, so the image is written next to the page as an ordinary
 * static file and the geometry as a small JSON sidecar.
 *
 * The image is NOT always a PNG — `demo.dd2vtt` carries WebP — so the
 * extension is chosen from magic bytes rather than assumed (the same
 * mistake `map_import.rs::detect_image_extension` had to fix once already).
 *
 * Output lands in `public/assets/maps/`, which means Bevy's `AssetServer`
 * reaches it as the plain relative path `maps/<name>.<ext>` against its
 * default "assets" root — deliberately the *opposite* of the app's rooted
 * `/api/...` paths, so the sandbox can tell a Bevy asset problem apart from
 * an app-plumbing problem.
 */

import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SOURCE_DIR = path.resolve(__dirname, "../../../examples/maps");
const OUT_DIR = path.resolve(__dirname, "../public/assets/maps");

/** Picks a file extension from the image's magic bytes. */
function detectExtension(bytes) {
  if (bytes.length >= 8 && bytes.subarray(0, 8).equals(Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))) {
    return "png";
  }
  if (
    bytes.length >= 12 &&
    bytes.subarray(0, 4).toString("ascii") === "RIFF" &&
    bytes.subarray(8, 12).toString("ascii") === "WEBP"
  ) {
    return "webp";
  }
  if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) {
    return "jpg";
  }
  throw new Error("unrecognised image format in dd2vtt payload");
}

mkdirSync(OUT_DIR, { recursive: true });

const manifest = [];

for (const file of readdirSync(SOURCE_DIR).filter((f) => f.endsWith(".dd2vtt")).sort()) {
  const name = path.basename(file, ".dd2vtt");
  const parsed = JSON.parse(readFileSync(path.join(SOURCE_DIR, file), "utf8"));

  const bytes = Buffer.from(parsed.image ?? "", "base64");
  if (bytes.length === 0) {
    console.warn(`[maps] ${name}: no image payload, skipped`);
    continue;
  }

  const ext = detectExtension(bytes);
  writeFileSync(path.join(OUT_DIR, `${name}.${ext}`), bytes);

  const pixelsPerGrid = parsed.resolution?.pixels_per_grid ?? 128;
  const gridsX = parsed.resolution?.map_size?.x ?? 0;
  const gridsY = parsed.resolution?.map_size?.y ?? 0;

  manifest.push({
    name,
    // Relative, Bevy-asset-root form — see this file's header.
    image: `maps/${name}.${ext}`,
    // The dimensions the app derives server-side on import, computed the
    // same way here so the sandbox exercises identical numbers.
    widthPx: gridsX * pixelsPerGrid,
    heightPx: gridsY * pixelsPerGrid,
    pixelsPerGrid,
    walls: (parsed.line_of_sight ?? []).length,
    portals: (parsed.portals ?? []).length,
    lights: (parsed.lights ?? []).length,
    bytes: bytes.length,
  });

  console.log(`[maps] ${name}.${ext} — ${gridsX * pixelsPerGrid}x${gridsY * pixelsPerGrid}px, ${(bytes.length / 1024 / 1024).toFixed(1)}MB`);
}

writeFileSync(path.join(OUT_DIR, "manifest.json"), JSON.stringify(manifest, null, 2));
console.log(`[maps] wrote ${manifest.length} maps + manifest.json to public/assets/maps/`);
