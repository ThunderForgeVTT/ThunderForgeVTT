// Spec 091 T030: the room's token art, drawn once and committed.
//
// Renders each hero of the front page's PARTY with `createHero(spec).token()`
// from @thunderforge/heroes, and `src/room/chest.svg` and `chest-open.svg`,
// to 256x256 webp files
// in `public/room/<id>.webp`. Chromium draws each SVG into a canvas on a blank
// page; no server, no engine. Outside the token the image is transparent.
//
// Rerun by hand when packages/heroes changes the token's look, when PARTY in
// src/sections/Hero.tsx changes, or when a chest SVG is redrawn:
//   node apps/landing/scripts/make-room-art.mjs
// The landing slice does not run it, so packages/heroes stays out of it.

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";
import { createHero, PRESET_HEROES } from "@thunderforge/heroes";

const HERE = new URL(".", import.meta.url).pathname;
const LANDING = resolve(HERE, "..");
const OUT = join(LANDING, "public/room");
const SIZE = 256;
const QUALITY = 0.85;
// The same four as PARTY in src/sections/Hero.tsx.
const PARTY = ["sir-pip", "mira-starweave", "fenna-swiftbow", "brother-oak"];

const art = PARTY.map((slug) => {
  const preset = PRESET_HEROES.find((p) => p.slug === slug);
  if (!preset) throw new Error(`make-room-art: no preset hero "${slug}"`);
  return { id: slug, svg: createHero(preset.spec).token() };
});
for (const id of ["chest", "chest-open"]) {
  art.push({ id, svg: readFileSync(join(LANDING, `src/room/${id}.svg`), "utf8") });
}

mkdirSync(OUT, { recursive: true });
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.setContent("<!doctype html><title>room art</title>");
  for (const { id, svg } of art) {
    const webp = await page.evaluate(
      async ({ svg, size, quality }) => {
        const img = new Image(size, size);
        img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
        await img.decode();
        const c = document.createElement("canvas");
        c.width = c.height = size;
        c.getContext("2d").drawImage(img, 0, 0, size, size);
        return c.toDataURL("image/webp", quality).split(",")[1];
      },
      { svg, size: SIZE, quality: QUALITY },
    );
    const file = join(OUT, `${id}.webp`);
    const bytes = Buffer.from(webp, "base64");
    writeFileSync(file, bytes);
    console.log(`make-room-art: ${id}.webp ${bytes.length} bytes`);
  }
} finally {
  await browser.close();
}
