// Spec 090 FR-011 (T035): the engine frame's poster, drawn by the engine.
//
// Serves the built landing with the demo's engine (`vite preview`, which
// mounts `data/demo` at `/demo/`), boots the frame in Chromium with reduced
// motion, throws `1d20+5` from a seed the dice crate rolls to 17, waits for
// the engine to report the dice landed, and saves the frame as a 1200x675
// webp in `public/dice-table.webp`. The webp is committed, so a landing build
// needs no GPU.
//
// Rerun by hand when the engine's look changes (the dice, the clear colour,
// the camera), after both builds:
//   pnpm -F @thunderforge/demo run build
//   pnpm -F @thunderforge/landing run build
//   node apps/landing/scripts/make-poster.mjs
import { spawn } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { createServer } from "node:net";
import { join, resolve } from "node:path";
import { chromium } from "@playwright/test";
import { initSync, roll } from "@thunderforge/dice";
import { diceCommand } from "../../../packages/dice-throw/src/index.ts";

const HERE = new URL(".", import.meta.url).pathname;
const LANDING = resolve(HERE, "..");
const ROOT = resolve(LANDING, "../..");
const OUT = join(LANDING, "public/dice-table.webp");
const FORMULA = "1d20+5";
const WANT = 17;
const [W, H] = [1200, 675];

for (const need of [join(LANDING, "dist/index.html"), join(ROOT, "data/demo/engine.json")]) {
  if (!existsSync(need)) throw new Error(`make-poster: ${need} is missing; build first`);
}

// The crate itself picks the seed, so the poster shows a roll it really makes.
const require = createRequire(import.meta.url);
initSync({ module: readFileSync(require.resolve("@thunderforge/dice/dice_bg.wasm")) });
let detail = null;
for (let s = 1; s < 10_000 && !detail; s++) {
  // xoshiro's first draw reads only the second word, through a multiply
  // whose top bits a small word barely moves: that word is the spread one.
  const seed = new Uint32Array([s, Math.imul(s, 0x9e3779b1), Math.imul(s, 0x85ebca6b), 1]);
  const r = JSON.parse(roll(FORMULA, "{}", seed));
  if (r.kind.Total === WANT) detail = r;
}
if (!detail) throw new Error(`make-poster: no seed under 10000 rolls ${FORMULA} to ${WANT}`);
const command = diceCommand(detail, { rollerName: "You", id: "poster" });

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
  // The frame alone, at the poster's own size, so the shot needs no resample
  // and is quick enough to land inside the throw's hold.
  await page.addStyleTag({
    content: `#dice-table { position: fixed; inset: 0 auto auto 0; z-index: 9; width: ${W}px; border: 0; border-radius: 0; }`,
  });
  const frame = page.locator("#dice-table");
  await frame.scrollIntoViewIfNeeded();
  await frame.getByRole("button", { name: "Throw it on the table" }).click();
  await frame.and(page.locator(".engine-frame--running")).waitFor({ timeout: 300_000 });

  // The page imported the engine from this URL, so this is the running one.
  await page.evaluate(async (json) => {
    const m = await (await fetch("/demo/engine.json")).json();
    const engine = await import(`/demo/${m.js}`);
    engine.apply_world_command(json);
    for (let i = 0; i < 300; i++) {
      if (JSON.parse(engine.dice_landed()).some((t) => t.rollId === "poster")) return;
      await new Promise((r) => setTimeout(r, 100));
    }
    throw new Error("the dice never landed");
  }, JSON.stringify(command));
  // At once: a landed throw holds 2.5 s, and a headless screenshot of a
  // live canvas takes most of that.
  const t0 = Date.now();
  const png = await page.screenshot({ clip: { x: 0, y: 0, width: W, height: H } });
  console.log(`make-poster: shot in ${Date.now() - t0} ms`);
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
  console.log(`make-poster: ${FORMULA} = ${WANT}, wrote ${OUT}`);
} finally {
  await browser.close();
  server.kill();
}
