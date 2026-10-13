// Spec 091 FR-018 (T039): the room frame's poster, drawn by the engine.
//
// Serves the built landing with the demo's engine, as `make-poster.mjs`
// does, presses "Play the room" with Sir Pip, waits until the hero's sight
// is drawn and the camera has settled, and saves the frame as a 1200x675
// webp in `public/room-table.webp`. The webp is committed.
//
// Rerun by hand when the engine's look or the room's art changes, after
// both builds:
//   pnpm -F @thunderforge/demo run build
//   pnpm -F @thunderforge/landing run build
//   node apps/landing/scripts/make-room-poster.mjs
import { spawn } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";

const HERE = new URL(".", import.meta.url).pathname;
const LANDING = resolve(HERE, "..");
const ROOT = resolve(LANDING, "../..");
const OUT = join(LANDING, "public/room-table.webp");
const [W, H] = [1200, 675];
/** `fit_camera_to` eases whatever reduced motion says; let it arrive. */
const SETTLE_MS = 2500;

for (const need of [
  join(LANDING, "dist/index.html"),
  join(LANDING, "dist/maps/demo.json"),
  join(ROOT, "data/demo/engine.json"),
]) {
  if (!existsSync(need)) throw new Error(`make-room-poster: ${need} is missing; build first`);
}

const port = await new Promise((done) => {
  const s = createServer().listen(0, "127.0.0.1", () => {
    const { port } = s.address();
    s.close(() => done(port));
  });
});
const server = spawn("pnpm", ["exec", "vite", "preview"], {
  cwd: LANDING,
  env: { ...process.env, LANDING_PREVIEW_PORT: String(port) },
  stdio: "ignore",
});
const browser = await chromium.launch();
try {
  const base = `http://127.0.0.1:${port}`;
  for (let i = 0; i < 100; i++) {
    if (await fetch(base).then((r) => r.ok, () => false)) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  const page = await browser.newPage({
    viewport: { width: W + 80, height: H + 80 },
    reducedMotion: "reduce",
  });
  await page.goto(base);
  const frame = page.locator("#room-table");
  await frame.scrollIntoViewIfNeeded();
  await page.addStyleTag({
    content: `#room-table { position: fixed; inset: 0 auto auto 0; z-index: 9; width: ${W}px; border: 0; border-radius: 0; }`,
  });
  await frame.getByRole("button", { name: "Play the room" }).click();
  await frame.and(page.locator(".engine-frame--running")).waitFor({ timeout: 300_000 });

  await page.evaluate(async () => {
    const m = await (await fetch("/demo/engine.json")).json();
    const engine = await import(`/demo/${m.js}`);
    for (let i = 0; i < 300; i++) {
      if (JSON.parse(engine.sight_probe(0, 0)).looking) return;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("the hero's sight was never drawn");
  });
  await page.waitForTimeout(SETTLE_MS);

  const png = await page.screenshot({ clip: { x: 0, y: 0, width: W, height: H } });
  const webp = await page.evaluate(async (b64) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = document.createElement("canvas");
    [c.width, c.height] = [img.width, img.height];
    c.getContext("2d").drawImage(img, 0, 0);
    return c.toDataURL("image/webp", 0.82).split(",")[1];
  }, png.toString("base64"));
  writeFileSync(OUT, Buffer.from(webp, "base64"));
  console.log(`make-room-poster: wrote ${OUT}`);
} finally {
  await browser.close();
  server.kill();
}
