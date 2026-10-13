import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT, engine, landed } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { bootRoom, FIXED_SEEDS, playThrough, room } from "./support/room.ts";

/**
 * Spec 091 T053, US2 scenario 6: with reduced motion, the room moves
 * nothing it need not. The camera lands on the room in one frame, the dice
 * take the reduced time, and the room's own markup starts no CSS transition
 * or animation. The order of the setup (`set_reduced_motion` first) is the
 * encounter package's unit test; here the engine's answers are read back.
 */

interface Camera {
  x: number;
  y: number;
  scale: number;
}

/** Counts every CSS transition and animation started inside the room. */
async function countMotion(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { roomMotion: string[] };
    w.roomMotion = [];
    const el = document.querySelector("#room")!;
    for (const type of ["transitionrun", "animationstart"]) {
      el.addEventListener(type, (e) => {
        const t = e.target as Element;
        w.roomMotion.push(`${type} ${t.tagName.toLowerCase()}.${t.className}`);
      });
    }
  });
}
const motion = (page: Page) =>
  page.evaluate(() => (window as unknown as { roomMotion: string[] }).roomMotion);

/**
 * Clicks Start over from inside the page and reads the camera on every
 * animation frame for `ms`, so no frame of a glide goes unseen.
 */
function cameraAfterStartOver(page: Page, ms: number) {
  return page.evaluate(async (ms) => {
    const m = await (await fetch("/demo/engine.json")).json();
    const e = await import(`/demo/${m.js}`);
    const read = () => JSON.parse(e.camera_state()) as { x: number; y: number; scale: number };
    const seen = [read()];
    const button = [...document.querySelectorAll<HTMLButtonElement>("#room button")].find(
      (b) => b.textContent?.trim() === "Start over",
    )!;
    button.click();
    const end = performance.now() + ms;
    while (performance.now() < end) {
      await new Promise(requestAnimationFrame);
      const c = read();
      const last = seen.at(-1)!;
      if (c.x !== last.x || c.y !== last.y || c.scale !== last.scale) seen.push(c);
    }
    return seen;
  }, ms);
}

test.describe("the room under reduced motion", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 180_000);
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.addInitScript(FIXED_SEEDS);
  });

  test("the camera lands at once, the dice are reduced, the room's CSS is still", async ({ page }) => {
    const opened = await openLanding(page);
    await countMotion(page);
    await bootRoom(page);

    // Put the camera somewhere else, then let it settle there.
    const fitted = await engine<Camera>(page, "camera_state");
    await engine(page, "apply_world_command", JSON.stringify({ type: "set_camera", x: 4000, y: 4000, zoom: 0.5 }));
    let away: Camera | undefined;
    await expect
      .poll(async () => {
        const before = away;
        away = await engine<Camera>(page, "camera_state");
        return before !== undefined && away.x !== fitted.x && JSON.stringify(before) === JSON.stringify(away);
      }, { timeout: 10_000 })
      .toBe(true);

    // Start over sends the setup again: the next camera the engine shows is
    // the room, with no glide between.
    const seen = await cameraAfterStartOver(page, 2000);
    expect(seen.length, JSON.stringify(seen)).toBe(2);
    expect(seen[1]).toEqual(fitted);

    // The whole room, through the engine; every throw at the reduced time.
    await playThrough(page);
    await expect.poll(async () => (await landed(page)).length, { timeout: 30_000 }).toBeGreaterThan(0);
    const throws = await landed(page);
    for (const t of throws) expect(t.reducedMotion).toBe(true);

    await expect(room(page).getByRole("link", { name: "Open the demo" })).toBeVisible();
    expect(await motion(page)).toEqual([]);
    expectClean(opened);
  });
});
