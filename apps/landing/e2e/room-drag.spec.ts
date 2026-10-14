import { expect, test, type Page } from "@playwright/test";
import { HERO_ID, toWorld } from "@thunderforge/encounter";
import { BOOT_TIMEOUT, BUILT, engine } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { bootRoom, engineHero, recordSaid, roomFrame, saidLog, script, square } from "./support/room.ts";

/**
 * Spec 091 FR-011 by the mouse: a token dragged on the table obeys the walls
 * as a keyboard step does. The engine stops a player's token at a wall while
 * it is carried (`token.rs`, the drag's follow), and judges the drop's
 * straight line as the server would; a drag that ends across a wall or a
 * shut door is refused, the room says so, and the hero stays put. A drag let
 * go off the table cannot leave the token across a wall either, since it
 * never crossed one.
 */

const BOARD = { w: 960, h: 540 };
const grid = script.map.gridSize;

/** Where on the page the engine draws a world point. */
async function onPage(page: Page, world: { x: number; y: number }) {
  const cam = await engine<{ x: number; y: number; scale: number }>(page, "camera_state");
  const box = (await roomFrame(page).locator(".engine-stage").boundingBox())!;
  const bx = BOARD.w / 2 + (world.x - cam.x) / cam.scale;
  const by = BOARD.h / 2 - (world.y - cam.y) / cam.scale;
  return { x: box.x + (bx / BOARD.w) * box.width, y: box.y + (by / BOARD.h) * box.height };
}

/** A square read off the engine, to the half-square a snapped token sits on. */
const near = (p: { x: number; y: number } | undefined) =>
  p && { x: Math.round(p.x * 2) / 2, y: Math.round(p.y * 2) / 2 };

/** Drags the hero from one square towards another, in eight mouse moves, and lets go unless told not to. */
async function drag(
  page: Page,
  a: { x: number; y: number },
  b: { x: number; y: number },
  { release = true } = {},
) {
  const from = await onPage(page, toWorld(a, grid));
  const to = await onPage(page, toWorld(b, grid));
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  for (let i = 1; i <= 8; i += 1) {
    await page.mouse.move(from.x + ((to.x - from.x) * i) / 8, from.y + ((to.y - from.y) * i) / 8);
    await page.waitForTimeout(40);
  }
  if (release) await page.mouse.up();
}

test.describe("a drag on the table", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
    await page.emulateMedia({ reducedMotion: "reduce" });
  });

  test("walls and the shut door refuse it, carried or dropped", async ({ page }) => {
    const opened = await openLanding(page);
    await bootRoom(page);
    await recordSaid(page);
    await roomFrame(page).scrollIntoViewIfNeeded();
    await expect.poll(() => engineHero(page)).toEqual(script.start);

    // Towards the shut door to the hall, five squares south: the token stops
    // at the door while it is carried, and the drop lands it there, in front.
    const n = (await saidLog(page)).length;
    await drag(page, script.start, { x: script.start.x, y: script.start.y - 5 });
    await expect.poll(async () => (await saidLog(page)).length).toBeGreaterThan(n);
    expect((await saidLog(page)).at(-1)).toMatch(/door to the hall is within reach to the south, shut/);
    const atDoor = { x: script.start.x, y: -1.5 };
    await expect.poll(async () => near(await engineHero(page))).toEqual(atDoor);
    await expect.poll(() => square(page)).toBe(`${atDoor.x},${atDoor.y}`);
    expect((await engine<{ tokenId: string }[]>(page, "token_footprints")).some((t) => t.tokenId === HERO_ID)).toBe(true);

    // A legal drag back, one square north, and the room reads the square
    // clean, not the engine's float.
    const at = { x: atDoor.x, y: atDoor.y + 1 };
    await drag(page, atDoor, at);
    await expect.poll(async () => near(await engineHero(page))).toEqual(at);
    await expect.poll(() => square(page)).toBe(`${at.x},${at.y}`);

    // Carried west through the room's plain wall at x = -9.5 and held there:
    // the token stops at the wall while the pointer goes on.
    await drag(page, at, { x: -12, y: at.y }, { release: false });
    const held = (await engineHero(page))!;
    expect(held.x, `held at ${JSON.stringify(held)}`).toBeGreaterThan(-9.5);
    expect(held.x).toBeLessThan(at.x);

    // Let go off the table entirely: the token is put back, not left across.
    const box = (await roomFrame(page).boundingBox())!;
    await page.mouse.move(box.x - 40, box.y + box.height / 2);
    await page.mouse.up();
    await page.waitForTimeout(800);
    const after = near(await engineHero(page))!;
    expect(after.x).toBeGreaterThan(-9.5);
    expect(after.y).toBe(at.y);
    expect(await square(page)).toBe(`${after.x},${after.y}`);
    expectClean(opened);
  });

  test("the chest is furniture: dragged, it goes back to its square", async ({ page }) => {
    const opened = await openLanding(page);
    await bootRoom(page);
    await recordSaid(page);
    await roomFrame(page).scrollIntoViewIfNeeded();
    await expect.poll(() => engineHero(page)).toEqual(script.start);

    // The engine lets a press pick any token up, as the client does; there
    // the server refuses a move of a token that is not yours. The room is
    // the server here: the chest is sent back, the hero has not moved, and
    // the room says nothing of it, not even of the wall the drag crossed.
    const n = (await saidLog(page)).length;
    const home = script.chest.square;
    await drag(page, home, { x: home.x + 2, y: home.y + 1 });
    await expect.poll(() => engineChest(page), { timeout: 5_000 }).toEqual(home);
    await page.waitForTimeout(500);
    expect(await engineChest(page)).toEqual(home);
    expect(await engineHero(page)).toEqual(script.start);
    expect(await square(page)).toBe(`${script.start.x},${script.start.y}`);
    expect((await saidLog(page)).length).toBe(n);
    expectClean(opened);
  });
});

/** Where the engine has the chest, in squares, rounded to the half-square. */
async function engineChest(page: Page) {
  const all = await engine<{ tokenId: string; x: number; y: number }[]>(page, "token_footprints");
  const c = all.find((t) => t.tokenId === script.chest.tokenId);
  return near(c && { x: c.x / grid, y: c.y / grid });
}
