import { expect, test } from "@playwright/test";
import { toWorld } from "@thunderforge/encounter";
import { BOOT_TIMEOUT, BUILT, engine } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { bootRoom, FIXED_SEEDS, playThrough, roomFrame, script } from "./support/room.ts";

/**
 * Spec 091: the chest's opening on the table. When the roll opens it, gold
 * glitters over the chest's square for a moment, over the board where the
 * engine draws the chest, and then it is gone; the open chest's art stands
 * alone. With motion allowed, since under reduced motion nothing glitters
 * (`room-motion.spec.ts` counts the room's animations then).
 */

const BOARD = { w: 960, h: 540 };

test.describe("the chest opens", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 240_000);
    await page.addInitScript(FIXED_SEEDS);
  });

  test("gold glitters over the chest's square for a moment, then the open chest stands alone", async ({ page }) => {
    const opened = await openLanding(page);
    // `openLanding` asks for reduced motion; this room wants its motion.
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await bootRoom(page);
    await expect(roomFrame(page).locator(".room-glitter")).toHaveCount(0);
    // The moment is short, so it is recorded as it happens rather than looked for after.
    await page.evaluate(() => {
      const w = window as unknown as { glitter: Array<{ at: number; x: number; y: number; size: number; sparks: number }>; glitterGone: number | null };
      w.glitter = [];
      w.glitterGone = null;
      const frame = document.querySelector("#room-table")!;
      new MutationObserver((records) => {
        for (const r of records) {
          for (const n of r.addedNodes) {
            if (n instanceof HTMLElement && n.classList.contains("room-glitter")) {
              w.glitter.push({
                at: performance.now(),
                x: parseFloat(n.style.left),
                y: parseFloat(n.style.top),
                size: parseFloat(n.style.width),
                sparks: n.querySelectorAll(".room-spark").length,
              });
            }
          }
          for (const n of r.removedNodes) {
            if (n instanceof HTMLElement && n.classList.contains("room-glitter")) w.glitterGone = performance.now();
          }
        }
      }).observe(frame, { childList: true, subtree: true });
    });

    await playThrough(page);

    // The moment: once, over the chest, in the board's pixels, with its sparks.
    const seen = () =>
      page.evaluate(() => {
        const w = window as unknown as { glitter: Array<{ at: number; x: number; y: number; size: number; sparks: number }>; glitterGone: number | null };
        return { glitter: w.glitter, gone: w.glitterGone };
      });
    await expect.poll(async () => (await seen()).glitter.length, { timeout: 5_000 }).toBe(1);
    const cam = await engine<{ x: number; y: number; scale: number }>(page, "camera_state");
    const chest = toWorld(script.chest.square, script.map.gridSize);
    const expected = {
      x: BOARD.w / 2 + (chest.x - cam.x) / cam.scale,
      y: BOARD.h / 2 - (chest.y - cam.y) / cam.scale,
    };
    const [at] = (await seen()).glitter;
    expect(at!.sparks).toBe(14);
    expect(Math.abs(at!.x - expected.x), JSON.stringify({ at, expected })).toBeLessThan(1);
    expect(Math.abs(at!.y - expected.y), JSON.stringify({ at, expected })).toBeLessThan(1);
    expect(Math.abs(at!.size - script.map.gridSize / cam.scale)).toBeLessThan(1);

    // Then gone, after its moment and within a few seconds, and the engine still has the chest.
    await expect.poll(async () => (await seen()).gone, { timeout: 5_000 }).not.toBeNull();
    const { gone } = await seen();
    expect(gone! - at!.at).toBeGreaterThan(1_000);
    expect(gone! - at!.at).toBeLessThan(4_000);
    await expect(roomFrame(page).locator(".room-glitter")).toHaveCount(0);
    const footprints = await engine<{ tokenId: string }[]>(page, "token_footprints");
    expect(footprints.some((t) => t.tokenId === script.chest.tokenId)).toBe(true);
    expectClean(opened);
  });
});
