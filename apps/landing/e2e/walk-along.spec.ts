import { expect, test, type Page } from "@playwright/test";
import { BOOT_TIMEOUT, BUILT, engine } from "./support/engine.ts";
import { openLanding } from "./support/page.ts";

/**
 * Spec 098: on a wide page the hero walks the page with the visitor. Left
 * alone, it leaves the tavern by the south door, never through a wall, and
 * keeps near the middle of the screen as the page scrolls.
 */

interface Pt {
  x: number;
  y: number;
}

/** Records where the page sends the walker, in page pixels, every frame:
 * the engine draws it snapped to its square. */
async function watchWalker(page: Page) {
  await page.evaluate(() => {
    const w = window as unknown as { walk: Pt[]; walker: () => Pt | null };
    w.walk = [];
    w.walker = () => {
      const at = document.getElementById("page-table")?.dataset.walker?.split(",").map(Number);
      return at ? { x: at[0]!, y: at[1]! } : null;
    };
    const tick = () => {
      const at = w.walker();
      const last = w.walk[w.walk.length - 1];
      if (at && (!last || last.x !== at.x || last.y !== at.y)) w.walk.push(at);
      requestAnimationFrame(tick);
    };
    tick();
  });
}

const walker = (page: Page) => page.evaluate(() => (window as unknown as { walker: () => Pt | null }).walker());
const walk = (page: Page) => page.evaluate(() => (window as unknown as { walk: Pt[] }).walk);

/** Scrolls to `y` and waits for the hero to stand still. */
async function scrollAndSettle(page: Page, y: number): Promise<Pt> {
  await page.evaluate((y) => window.scrollTo(0, y), y);
  let last: Pt | null = null;
  let still = 0;
  for (let i = 0; i < 100 && still < 4; i++) {
    await page.waitForTimeout(50);
    const at = await walker(page);
    still = at && last && Math.hypot(at.x - last.x, at.y - last.y) < 0.5 ? still + 1 : 0;
    last = at;
  }
  return last!;
}

test.describe("the hero walks a wide page", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the walk is the wide page's");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
  });

  test("left alone, it leaves the tavern by the south door and keeps to the screen's middle", async ({ page }) => {
    await openLanding(page, { settle: false });
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.reload();
    await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });
    await watchWalker(page);

    const inn = await page.evaluate(() => {
      const cell = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
      const r = document.querySelector("[data-testid=hero-tavern]")!.getBoundingClientRect();
      return { cell, x: r.left + scrollX, y: r.top + scrollY };
    });
    const seat = await scrollAndSettle(page, 0);
    expect(seat.y).toBeLessThan(inn.y + 7 * inn.cell);

    // Down past the tavern in small steps, as a wheel would.
    const vh = page.viewportSize()!.height;
    const out = inn.y + 9 * inn.cell;
    for (let y = 0; y <= out; y += 60) await scrollAndSettle(page, y);
    // The engine has the walker too, on the square under it.
    const drawn = await engine<Array<{ tokenId: string }>>(page, "token_footprints");
    expect(drawn.some((t) => t.tokenId === "walker")).toBe(true);
    const below = await scrollAndSettle(page, out);
    expect(below.y).toBeGreaterThan(inn.y + 7 * inn.cell);

    // SC-003: from there down to the dungeon, it stays in the middle 40%.
    const room = await page.evaluate(() => document.querySelector("#room")!.getBoundingClientRect().top + scrollY);
    for (let y = out; y < room; y += Math.round(vh / 3)) {
      const at = await scrollAndSettle(page, y);
      const onScreen = at.y - (await page.evaluate(() => scrollY));
      expect.soft(onScreen / vh, `at scroll ${y}`).toBeGreaterThan(0.3);
      expect.soft(onScreen / vh, `at scroll ${y}`).toBeLessThan(0.7);
    }

    // The engine plans round the tavern's walls: from the seat to a point
    // under the tavern's west end, its way goes out by the door.
    const ask = (p: Pt) => {
      const doc = { w: 1440 - scrollbar, h: docH };
      return { x: p.x - doc.w / 2, y: doc.h / 2 - p.y };
    };
    const scrollbar = await page.evaluate(() => 1440 - document.documentElement.clientWidth);
    const docH = await page.evaluate(() => document.documentElement.scrollHeight);
    const way = await engine<{ path: Pt[] | null }>(
      page,
      "plan_path",
      JSON.stringify({ from: ask(seat), to: ask({ x: inn.x + 1.5 * inn.cell, y: inn.y + 9.5 * inn.cell }) }),
    );
    expect(way.path).not.toBeNull();
    const wx = ask({ x: inn.x + 5 * inn.cell, y: 0 }).x;
    const wy = ask({ x: 0, y: inn.y + 7 * inn.cell }).y;
    const through = way.path!.slice(1).flatMap((b, i) => {
      const a = way.path![i]!;
      if ((a.y - wy) * (b.y - wy) > 0 || a.y === b.y) return [];
      return [a.x + ((wy - a.y) / (b.y - a.y)) * (b.x - a.x) - wx];
    });
    expect(through).toHaveLength(1);
    expect(through[0]! / inn.cell).toBeGreaterThan(0);
    expect(through[0]! / inn.cell).toBeLessThan(1);

    // SC-002: past the goblins, all three are down.
    const goblins = await page.evaluate(() =>
      [...document.querySelectorAll<HTMLImageElement>("[data-testid=hero-tavern] img[alt^=Goblin]")].map(
        (img) => img.getBoundingClientRect().bottom + scrollY,
      ),
    );
    expect(goblins).toHaveLength(3);
    const down = () => engine<string[]>(page, "token_defeated").then((ids) => ids.filter((id) => id.includes("goblin")).sort());
    await expect.poll(down).toEqual(["mat-goblin-0", "mat-goblin-1", "mat-goblin-2"]);
    // Scrolled back until the hero is between the first two, only the first is.
    const [first, second] = goblins.map((b) => b - inn.cell / 2);
    let at = await scrollAndSettle(page, second! - vh / 2 + 200);
    for (let y = second! - vh / 2 + 200; y > 0 && at!.y >= second!; y -= 16) {
      at = await scrollAndSettle(page, y);
    }
    expect(at!.y).toBeGreaterThanOrEqual(first!);
    await expect.poll(down).toEqual(["mat-goblin-0"]);

    // Back to the top: back to the seat, by the same door.
    const back = await scrollAndSettle(page, 0);
    expect(Math.hypot(back.x - seat.x, back.y - seat.y)).toBeLessThan(1);
    await expect.poll(down).toEqual([]);

    // SC-001: out and back, it crossed the south wall in the doorway (cells
    // 5 to 6), and never crossed the walls' line anywhere else.
    const wall = inn.y + 7 * inn.cell;
    const steps = await walk(page);
    const crossings = steps.slice(1).flatMap((b, i) => {
      const a = steps[i]!;
      if ((a.y - wall) * (b.y - wall) > 0 || a.y === b.y) return [];
      return [a.x + ((wall - a.y) / (b.y - a.y)) * (b.x - a.x)];
    });
    expect(crossings.length).toBeGreaterThan(0);
    for (const x of crossings) {
      expect(x).toBeGreaterThan(inn.x + 5 * inn.cell);
      expect(x).toBeLessThan(inn.x + 6 * inn.cell);
    }
  });

  test("it goes to stand under the map picked in the atlas", async ({ page }) => {
    await openLanding(page, { settle: false });
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.reload();
    await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });
    await watchWalker(page);

    const tiles = page.locator(".atlas-tile");
    await expect(tiles.first()).toBeVisible({ timeout: 30_000 });
    const under = (i: number) =>
      tiles.nth(i).evaluate((el) => {
        const r = el.getBoundingClientRect();
        return { x: r.left + r.width / 2, bottom: r.bottom + scrollY };
      });
    const cell = await page.evaluate(() =>
      parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell")),
    );
    const vh = page.viewportSize()!.height;

    // SC-004: with the first map picked, the hero waits under it.
    const first = await under(0);
    for (let y = first.bottom - vh; y <= first.bottom - vh / 2; y += 120) await scrollAndSettle(page, y);
    let at = await scrollAndSettle(page, first.bottom - vh / 2);
    expect(Math.abs(at.x - first.x)).toBeLessThan(cell);
    expect(at.y).toBeGreaterThan(first.bottom);
    expect(at.y).toBeLessThan(first.bottom + 2 * cell);

    // Another map picked: the hero walks over to it, scroll untouched.
    await tiles.nth(5).click();
    const fifth = await under(5);
    await expect
      .poll(async () => Math.abs((await walker(page))!.x - fifth.x), { timeout: 15_000 })
      .toBeLessThan(cell);
    at = (await walker(page))!;
    expect(at.y).toBeGreaterThan(fifth.bottom);
    expect(at.y).toBeLessThan(fifth.bottom + 2 * cell);
  });

  test("it walks into the dungeon by its top, and round it when the room is not played", async ({ page }) => {
    await openLanding(page, { settle: false });
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.reload();
    await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });
    await watchWalker(page);
    const vh = page.viewportSize()!.height;
    const frame = page.locator("#room-table");
    await expect(frame).toHaveClass(/room-board/, { timeout: 30_000 });
    const board = await frame.evaluate((el) => {
      const cell = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
      const r = el.getBoundingClientRect();
      return { cell, x: r.left, y: r.top + scrollY, w: r.width, h: r.height };
    });
    const inside = (p: Pt) => p.x > board.x && p.x < board.x + board.w && p.y > board.y && p.y < board.y + board.h;

    // SC-005: down to the room, the hero goes in by its top and the room sits.
    const seatAt = board.y + board.cell / 2 - vh / 2;
    for (let y = seatAt - vh; y <= seatAt; y += 96) await scrollAndSettle(page, y);
    await expect(frame).toHaveClass(/engine-frame--running/, { timeout: 15_000 });
    const into = (await walk(page)).filter(inside);
    expect(into.length).toBeGreaterThan(0);
    for (const p of into) expect(p.y, "in by the top row only").toBeLessThan(board.y + board.cell);

    // Left alone and scrolled on, it stands up, out by the top and round.
    const past = board.y + board.h + vh;
    for (let y = seatAt; y <= past; y += 96) await scrollAndSettle(page, y);
    await expect(frame).not.toHaveClass(/engine-frame--running/);
    const below = (await walker(page))!;
    expect(below.y).toBeGreaterThan(board.y + board.h);
    for (const p of (await walk(page)).filter(inside)) expect(p.y).toBeLessThan(board.y + board.cell);

    // Scrolled back up, the walk runs backwards and the room sits again.
    for (let y = past; y >= seatAt; y -= 96) await scrollAndSettle(page, y);
    await expect(frame).toHaveClass(/engine-frame--running/, { timeout: 15_000 });
    for (const p of (await walk(page)).filter(inside)) expect(p.y).toBeLessThan(board.y + board.cell);
  });

  test("at the camp, the party sits on the logs and the hero parks on the empty seat", async ({ page }) => {
    await openLanding(page, { settle: false });
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.reload();
    await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });
    await watchWalker(page);
    const camp = await page.locator("[data-testid=camp]").evaluate((el) => {
      const cell = parseFloat(getComputedStyle(document.documentElement).getPropertyValue("--cell"));
      const r = el.getBoundingClientRect();
      return { cell, x: r.left + scrollX, y: r.top + scrollY };
    });
    const square = (p: Pt) => [Math.floor((p.x - camp.x) / camp.cell), Math.floor((p.y - camp.y) / camp.cell)];

    // To the page's end: the walk ends on the top log, over the fire.
    const end = await page.evaluate(() => document.documentElement.scrollHeight - innerHeight);
    const vh = page.viewportSize()!.height;
    for (let y = end - 2 * vh; y <= end; y += vh / 3) await scrollAndSettle(page, y);
    const parked = await scrollAndSettle(page, end);
    expect(square(parked)).toEqual([3, 1]);

    // Everyone the visitor did not pick is on a log seat (FR-010).
    const doc = await page.evaluate(() => ({
      w: document.documentElement.clientWidth,
      h: document.documentElement.scrollHeight,
    }));
    await expect
      .poll(async () => {
        const all = await engine<Array<{ tokenId: string; x: number; y: number }>>(page, "token_footprints");
        return all
          .filter((t) => t.tokenId.startsWith("camp-"))
          .map((t) => square({ x: t.x + doc.w / 2, y: doc.h / 2 - t.y }))
          .sort();
      })
      .toEqual([
        [1, 1],
        [6, 3],
        [6, 5],
      ]);
  });
});

test.describe("the dusk on a big screen", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the big screens are desktops");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
  });

  for (const [width, height] of [
    [1920, 1080],
    [2560, 1440],
  ] as const) {
    test(`at ${width}x${height}, no 100 px of scroll darkens the page by more than 0.08 (SC-006)`, async ({ page }) => {
      await page.setViewportSize({ width, height });
      await openLanding(page, { settle: false });
      await page.emulateMedia({ reducedMotion: "no-preference" });
      await page.reload();
      await expect(page.locator("body.in-engine")).toBeAttached({ timeout: BOOT_TIMEOUT });
      // The page has laid its day and night out once its walker is on the table.
      await expect(page.locator("#page-table[data-walker]")).toBeAttached();
      const end = await page.evaluate(() => document.documentElement.scrollHeight - innerHeight);
      const dark: number[] = [];
      for (let y = 0; y <= end; y += 100) {
        // The dark the page is heading for, once the scroll at `y` is heard.
        const goal = await page.evaluate(
          (y) =>
            new Promise<number>((done) => {
              scrollTo({ top: y, behavior: "instant" });
              requestAnimationFrame(() =>
                requestAnimationFrame(() => done(Number(document.getElementById("page-table")!.dataset.dusk))),
              );
            }),
          y,
        );
        dark.push(goal);
      }
      const steps = dark.slice(1).map((d, i) => Math.abs(d - dark[i]!));
      expect(Math.max(...dark), "night falls at all").toBeGreaterThan(0.9);
      expect(Math.max(...steps)).toBeLessThanOrEqual(0.08);
    });
  }
});
