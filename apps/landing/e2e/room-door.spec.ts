import { expect, test, type Page } from "@playwright/test";
import { HALL_POINT } from "../src/room/script.ts";
import { BOOT_TIMEOUT, BUILT, engine } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { bootRoom, pad, recordSaid, room, roomFrame, saidLog, seen, square } from "./support/room.ts";

/**
 * Spec 091 T037, FR-021: the door icon on the table takes a click through
 * the stage's CSS `scale`, at a phone, a tablet and a desktop width, and the
 * wheel over the table still scrolls the page.
 *
 * The icon's place comes from the engine (`door_icons`, world units) and the
 * camera (`camera_state`); the board is 960x540 logical pixels, +y up, and
 * the page shows it at the stage's on-screen box.
 */

const BOARD = { w: 960, h: 540 };
const WIDTHS = [1440, 768, 390];

interface Icon {
  wallId: string;
  icon: string;
  x: number;
  y: number;
}

/** Where on the page the engine draws `wallId`'s icon. */
async function iconOnPage(page: Page, wallId: string) {
  let icon: Icon | undefined;
  await expect
    .poll(async () => (icon = (await engine<Icon[]>(page, "door_icons")).find((i) => i.wallId === wallId))?.icon)
    .toBe("open");
  const cam = await engine<{ x: number; y: number; scale: number }>(page, "camera_state");
  const box = (await roomFrame(page).locator(".engine-stage").boundingBox())!;
  const bx = BOARD.w / 2 + (icon!.x - cam.x) / cam.scale;
  const by = BOARD.h / 2 - (icon!.y - cam.y) / cam.scale;
  return { x: box.x + (bx / BOARD.w) * box.width, y: box.y + (by / BOARD.h) * box.height };
}

/** Three steps south, to the door's square, by keys inside the controls. */
async function walkToDoor(page: Page) {
  await pad(page, "North").focus();
  for (let i = 0; i < 3; i += 1) {
    const n = (await saidLog(page)).length;
    await page.keyboard.press("ArrowDown");
    await expect.poll(async () => (await saidLog(page)).length).toBe(n + 1);
  }
  expect(await square(page)).toBe("2,0.5");
}

test.describe("the door on the table", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 300_000);
    await page.emulateMedia({ reducedMotion: "reduce" });
  });

  test("a click on the icon opens the door at three widths; the wheel scrolls the page", async ({ page }) => {
    const opened = await openLanding(page);
    await bootRoom(page);
    await recordSaid(page);

    for (const width of WIDTHS) {
      await page.setViewportSize({ width, height: 900 });
      await room(page).getByRole("button", { name: "Start over" }).click();
      await walkToDoor(page);
      // The engine draws only while its frame is on screen; sight comes from drawing.
      await roomFrame(page).scrollIntoViewIfNeeded();
      // The landing shows the whole map: the shut door hides nothing.
      await expect.poll(() => seen(page, HALL_POINT), { message: `${width}px` }).toBe(true);
      const at = await iconOnPage(page, "door-hall");
      const n = (await saidLog(page)).length;
      await page.mouse.move(at.x, at.y);
      await page.waitForTimeout(400);
      await page.mouse.down();
      await page.waitForTimeout(200);
      await page.mouse.up();
      await expect.poll(async () => (await saidLog(page)).slice(n), { message: `${width}px` }).toHaveLength(1);
      expect((await saidLog(page)).at(-1), `${width}px`).toMatch(/^You open the door to the hall\./);
      await expect.poll(() => seen(page, HALL_POINT), { message: `${width}px` }).toBe(true);

      // The wheel over the table moves the page, not the camera.
      const cam = await engine<{ scale: number }>(page, "camera_state");
      const y0 = await page.evaluate(() => scrollY);
      await page.mouse.wheel(0, 200);
      await expect.poll(() => page.evaluate(() => scrollY), { message: `${width}px` }).toBeGreaterThan(y0);
      expect((await engine<{ scale: number }>(page, "camera_state")).scale, `${width}px`).toBe(cam.scale);
    }
    expectClean(opened);
  });
});
