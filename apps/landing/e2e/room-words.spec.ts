import { expect, test, type Page } from "@playwright/test";
import { isEngineFile } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import {
  FIXED_SEEDS,
  goal,
  pad,
  recordSaid,
  room,
  saidLog,
  square,
  stepAndHear,
} from "./support/room.ts";

/**
 * Spec 091 T051, US2: the room without the canvas. Every goal through the
 * page's own controls, each outcome said once in the live region, and no
 * engine file fetched. Then each reason the frame holds the engine back
 * leaves the same working room, and the room's keys act only inside it.
 *
 * The route, on the Proving Ground: the hall is south of the start. Three
 * steps south reach the door's square; the fourth is refused by the shut
 * door (goal 1). Open it, two steps south and one east reach the chest.
 */

/** Plays all four goals with the page's controls alone. */
async function playThrough(page: Page) {
  await room(page).scrollIntoViewIfNeeded();
  await recordSaid(page);
  expect(await square(page)).toBe("-3,1.5");

  for (const y of ["0.5", "-0.5", "-1.5"]) {
    await stepAndHear(page, "South", /door to the hall/);
    expect(await square(page)).toBe(`-3,${y}`);
  }
  await expect(goal(page, "door-reached")).toContainText("(not yet)");
  await stepAndHear(page, "South", /^The door is shut\. Walls and closed doors refuse a move/);
  expect(await square(page)).toBe("-3,-1.5");
  await expect(goal(page, "door-reached")).toContainText("(done)");

  const door = room(page).getByRole("button", { name: "Open the door to the hall" });
  await expect(door).toBeEnabled();
  const before = (await saidLog(page)).length;
  await door.click();
  await expect.poll(async () => (await saidLog(page)).slice(before)).toHaveLength(1);
  expect((await saidLog(page)).at(-1)).toMatch(/^You open the door to the hall\./);
  await expect(goal(page, "door-opened")).toContainText("(done)");

  await stepAndHear(page, "South", /hall/);
  await stepAndHear(page, "South", /hall/);
  await stepAndHear(page, "East", /^You find the chest\. It is locked: roll 1d20\+2 and meet 12/);
  await expect(goal(page, "chest-found")).toContainText("(done)");

  const roll = room(page).getByRole("button", { name: "Roll 1d20+2" });
  for (let i = 0; i < 30; i += 1) {
    const n = (await saidLog(page)).length;
    await roll.click();
    await expect.poll(async () => (await saidLog(page)).length).toBe(n + 1);
    const line = (await saidLog(page)).at(-1)!;
    if (/The chest opens\.$/.test(line)) break;
    expect(line).toMatch(/^You rolled \d+\. Not enough: you need 12\. Roll again\.$/);
  }
  await expect(goal(page, "chest-opened")).toContainText("(done)");
  await expect(roll).toBeDisabled();
  await expect(room(page).getByRole("link", { name: "Open the demo" })).toBeVisible();
}

test.describe("the room in words", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(FIXED_SEEDS);
  });

  test("all four goals with no engine, each outcome said once (SC-003)", async ({ page }) => {
    const opened = await openLanding(page);
    await playThrough(page);

    expect(opened.requests.filter(isEngineFile)).toEqual([]);
    expect(opened.requests.filter((u) => new URL(u).pathname.startsWith("/room/"))).toEqual([]);

    // Start over: back to the start, every goal open again, no reload.
    await room(page).getByRole("button", { name: "Start over" }).click();
    expect(await square(page)).toBe("-3,1.5");
    for (const id of ["door-reached", "door-opened", "chest-found", "chest-opened"]) {
      await expect(goal(page, id)).toContainText("(not yet)");
    }
    expectClean(opened);
  });

  test("with no door in reach the door button says why", async ({ page }) => {
    const opened = await openLanding(page);
    const door = room(page).getByRole("button", { name: "Open the door" });
    await expect(door).toBeDisabled();
    await expect(door).toHaveAccessibleDescription(/No door is within reach/);
    expectClean(opened);
  });

  const HELD_BACK: Array<{ reason: string; words: RegExp; stub: () => void }> = [
    {
      reason: "no_webgl2",
      words: /no WebGL2/,
      stub: () => {
        const get = HTMLCanvasElement.prototype.getContext;
        HTMLCanvasElement.prototype.getContext = function (this: HTMLCanvasElement, kind: string, ...rest: unknown[]) {
          return kind === "webgl2" ? null : get.call(this, kind as "2d", ...(rest as []));
        } as typeof get;
      },
    },
    {
      reason: "small_texture",
      words: /its largest picture is 2048 pixels/,
      stub: () => {
        const get = WebGL2RenderingContext.prototype.getParameter;
        WebGL2RenderingContext.prototype.getParameter = function (this: WebGL2RenderingContext, p: number) {
          return p === this.MAX_TEXTURE_SIZE ? 2048 : get.call(this, p);
        };
      },
    },
    {
      reason: "save_data",
      words: /it asks to save data/,
      stub: () => {
        Object.defineProperty(navigator, "connection", { value: { saveData: true } });
      },
    },
    {
      reason: "low_memory",
      words: /under 4 GB of memory/,
      stub: () => {
        Object.defineProperty(navigator, "deviceMemory", { value: 2 });
      },
    },
  ];

  for (const { reason, words, stub } of HELD_BACK) {
    test(`held back for ${reason}, the room still plays`, async ({ page }) => {
      await page.addInitScript(stub);
      const opened = await openLanding(page);
      // The room's frame says why; the dice tray's frame may say it too.
      await expect(page.locator("#room-table")).toContainText(words);
      await expect(page.locator("#room-table").getByRole("button", { name: "Play the room" })).toHaveCount(0);
      await playThrough(page);
      expect(opened.requests.filter(isEngineFile)).toEqual([]);
      expectClean(opened);
    });
  }

  test("keys move the hero inside the room's controls only, and never scroll", async ({ page }) => {
    const opened = await openLanding(page);
    await room(page).scrollIntoViewIfNeeded();

    // Inside the controls: ArrowDown and S step south, the page stays put.
    await pad(page, "North").focus();
    const y0 = await page.evaluate(() => scrollY);
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => square(page)).toBe("-3,0.5");
    await page.keyboard.press("s");
    await expect.poll(() => square(page)).toBe("-3,-0.5");
    await page.keyboard.press("ArrowUp");
    await expect.poll(() => square(page)).toBe("-3,0.5");
    expect(await page.evaluate(() => scrollY)).toBe(y0);

    // In the dice formula: letters are typed, the hero does not move.
    const formula = page.locator("#dice input[type=text], #dice input:not([type])").first();
    await formula.fill("");
    await formula.pressSequentially("wasd");
    await expect(formula).toHaveValue("wasd");
    await formula.press("ArrowDown");
    expect(await square(page)).toBe("-3,0.5");

    // On the hero mat: the hero section's own controls, not the room's.
    const mat = page.locator("[data-section=hero] button").first();
    await mat.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("s");
    expect(await square(page)).toBe("-3,0.5");
    expectClean(opened);
  });
});
