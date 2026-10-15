import { expect, test } from "@playwright/test";
import { isEngineFile } from "./support/engine.ts";
import { PARTY } from "../src/room/party.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { FIXED_SEEDS, goal, pad, playThrough, recordSaid, room, said, saidLog, square, stepAndHear } from "./support/room.ts";

/**
 * Spec 091 T051, US2: the room without the canvas. Every goal through the
 * page's own controls, each outcome said once in the live region, and no
 * engine file fetched. Then each reason the frame holds the engine back
 * leaves the same working room, and the room's keys act only inside it.
 *
 * The route, in the dungeon: the hall is south of the start. Three steps
 * south reach the door's square; the fourth is refused by the shut door
 * (goal 1). Open it, two steps south and one east reach the chest.
 */

test.describe("the room in words", () => {
  test.beforeEach(async ({ page }) => {
    await page.addInitScript(FIXED_SEEDS);
  });

  test("all four goals with no engine, each outcome said once (SC-003)", async ({ page }) => {
    const opened = await openLanding(page);
    await playThrough(page);

    expect(opened.requests.filter(isEngineFile)).toEqual([]);
    // The party's portraits are the hero builder's pictures, not the room's.
    const portraits = new Set(PARTY.map((h) => h.art));
    const roomFiles = opened.requests.map((u) => new URL(u).pathname);
    expect(roomFiles.filter((p) => p.startsWith("/room/") && !portraits.has(p))).toEqual([]);

    // Start over: back to the start, every goal open again, no reload.
    await room(page).getByRole("button", { name: "Start over" }).click();
    expect(await square(page)).toBe("2,3.5");
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

  // A disabled button drops focus to <body>: Enter after a throw would do
  // nothing, and a screen reader would lose its place. The roll is refused
  // while the dice are out, but the button keeps focus to the end.
  test("Enter on the roll keeps focus on the roll, throw after throw", async ({ page }) => {
    const opened = await openLanding(page);
    await room(page).scrollIntoViewIfNeeded();
    await expect(said(page)).toBeAttached();
    await recordSaid(page);
    for (let i = 0; i < 3; i += 1) await stepAndHear(page, "South", /door to the hall/);
    await room(page).getByRole("button", { name: "Open the door to the hall" }).click();
    await expect(goal(page, "door-opened")).toContainText("(done)");
    await stepAndHear(page, "South", /hall/);
    await stepAndHear(page, "South", /hall/);
    await stepAndHear(page, "East", /^You find the chest/);

    const roll = room(page).getByRole("button", { name: "Roll 1d20+2" });
    await roll.focus();
    for (let i = 0; i < 30; i += 1) {
      const n = (await saidLog(page)).length;
      await page.keyboard.press("Enter");
      await expect.poll(async () => (await saidLog(page)).length).toBeGreaterThan(n);
      await expect(roll).toBeFocused();
      if (/The chest opens\.$/.test((await saidLog(page)).at(-1)!)) break;
    }
    await expect(goal(page, "chest-opened")).toContainText("(done)");
    await expect(roll).toBeFocused();
    await expect(roll).toHaveAttribute("aria-disabled", "true");
    expectClean(opened);
  });

  test("keys move the hero inside the room's controls only, and never scroll", async ({ page }) => {
    const opened = await openLanding(page);
    await room(page).scrollIntoViewIfNeeded();

    // Inside the controls: ArrowDown and S step south, the page stays put.
    await pad(page, "North").focus();
    const y0 = await page.evaluate(() => scrollY);
    await page.keyboard.press("ArrowDown");
    await expect.poll(() => square(page)).toBe("2,2.5");
    await page.keyboard.press("s");
    await expect.poll(() => square(page)).toBe("2,1.5");
    await page.keyboard.press("ArrowUp");
    await expect.poll(() => square(page)).toBe("2,2.5");
    expect(await page.evaluate(() => scrollY)).toBe(y0);

    // In the dice formula: letters are typed, the hero does not move.
    const formula = page.locator("#dice input[type=text], #dice input:not([type])").first();
    await formula.fill("");
    await formula.pressSequentially("wasd");
    await expect(formula).toHaveValue("wasd");
    await formula.press("ArrowDown");
    expect(await square(page)).toBe("2,2.5");

    // In the hero section: its own controls, not the room's. A wide page's
    // tavern is on the engine, so its first control is a link.
    const mat = page.locator("[data-section=hero] button, [data-section=hero] a").first();
    await mat.focus();
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("s");
    expect(await square(page)).toBe("2,2.5");
    expectClean(opened);
  });
});
