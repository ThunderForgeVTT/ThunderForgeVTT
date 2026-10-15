import { expect, test, type Page } from "@playwright/test";
import { HERO_ID } from "@thunderforge/encounter";
import { HALL_POINT } from "../src/room/script.ts";
import { BOOT_TIMEOUT, BUILT, engine, landed, type Landed } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import {
  bootRoom,
  engineHero,
  FIXED_SEEDS,
  goal,
  pressAndHear,
  recordSaid,
  room,
  roomFrame,
  saidLog,
  script,
  seen,
  square,
  stepAndHear,
} from "./support/room.ts";

/**
 * Spec 091 T040, US1: the room through the engine. The pad forwards each
 * step to the canvas as a key, so the engine's own movement rule answers
 * and the page's referee checks it; a disagreement is a `console.error`,
 * which fails the test (FR-012). Every claim about the table is read back
 * from the engine: where the hero stands, what the viewer sees, the dice.
 */

const onSquare = async (page: Page) => {
  const s = await square(page);
  const at = await engineHero(page);
  return { dom: s, engine: at && `${at.x},${at.y}` };
};

/**
 * The DOM's square and the engine's agree, and are `want`. A click below the
 * frame can scroll it off screen, and a frame out of view is paused: the
 * engine catches up once it is seen again, so look at it first.
 */
async function standsAt(page: Page, want: string) {
  await roomFrame(page).evaluate((el) => el.scrollIntoView({ block: "nearest" }));
  await expect.poll(() => onSquare(page)).toEqual({ dom: want, engine: want });
}

test.describe("the room on the engine", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 180_000);
    await page.emulateMedia({ reducedMotion: "reduce" });
    await page.addInitScript(FIXED_SEEDS);
  });

  test("from the click to the open chest (US1 scenarios 2 to 7 and 9)", async ({ page }) => {
    const opened = await openLanding(page);

    // Scenario 9: the simulation sentence and the credit beside the frame.
    await expect(room(page).getByText("A simulation.")).toBeVisible();
    await expect(room(page).locator(".room-caption").getByRole("link", { name: "CC BY-SA 4.0" })).toBeVisible();

    await bootRoom(page);
    const started = Date.now();
    await recordSaid(page);

    // Scenario 2: the hero is the controlled token, at the start, seeing
    // from there; the hall behind the shut door is dark to them.
    await expect
      .poll(() => engine<{ named?: string; found?: boolean }>(page, "movement_state"))
      .toMatchObject({ named: HERO_ID, found: true });
    await standsAt(page, "-3,1.5");
    await expect.poll(() => seen(page, HALL_POINT)).toBe(false);
    for (const id of ["door-reached", "door-opened", "chest-found", "chest-opened"]) {
      await expect(goal(page, id)).toContainText("(not yet)");
    }

    // The engine moves the hero; the page agrees, square by square.
    for (const y of ["0.5", "-0.5", "-1.5"]) {
      await stepAndHear(page, "South", /door to the hall/);
      await standsAt(page, `-3,${y}`);
    }

    // Scenario 3: the shut door refuses the step, said once.
    await stepAndHear(page, "South", /^The door is shut\. Walls and closed doors refuse a move/);
    await standsAt(page, "-3,-1.5");
    await expect(goal(page, "door-reached")).toContainText("(done)");

    // Scenario 4: the door opens on the table, and the hall comes into sight.
    await pressAndHear(
      page,
      room(page).getByRole("button", { name: "Open the door to the hall" }),
      /^You open the door to the hall\./,
    );
    await expect.poll(() => seen(page, HALL_POINT)).toBe(true);
    await expect(goal(page, "door-opened")).toContainText("(done)");

    // Scenario 6: into the hall and beside the chest; found once.
    await stepAndHear(page, "South", /hall/);
    await standsAt(page, "-3,-2.5");
    await stepAndHear(page, "South", /hall/);
    await stepAndHear(page, "East", /^You find the chest\./);
    await standsAt(page, "-2,-3.5");
    await expect(goal(page, "chest-found")).toContainText("(done)");
    await page.waitForTimeout(500);
    expect((await saidLog(page)).filter((l) => l.startsWith("You find the chest"))).toHaveLength(1);

    // Scenario 7: the DC and formula first; each roll thrown on the table,
    // its kept faces adding up to the total the page states (SC-006).
    await expect(room(page).locator(".room-roll")).toContainText("Roll 1d20+2 and meet DC 12");
    const roll = room(page).getByRole("button", { name: "Roll 1d20+2" });
    let throws = 0;
    for (let i = 0; i < 30; i += 1) {
      await pressAndHear(page, roll, /^You rolled \d+\./);
      throws += 1;
      // The throw waits, paused, if the click took the frame off screen.
      await roomFrame(page).evaluate((el) => el.scrollIntoView({ block: "nearest" }));
      const total = Number(/You rolled (\d+)/.exec((await saidLog(page)).at(-1)!)![1]);
      let last: Landed | undefined;
      await expect
        .poll(async () => (last = (await landed(page)).at(-1)) && (await landed(page)).length, {
          timeout: 30_000,
        })
        .toBe(throws);
      const kept = last!.dice.filter((d) => d.kept).reduce((n, d) => n + d.face, 0);
      expect(kept + 2, `throw ${throws}`).toBe(total);
      if (total >= script.roll.dc) break;
      await expect(roll).toBeEnabled();
    }
    await expect(goal(page, "chest-opened")).toContainText("(done)");
    await expect(room(page).getByRole("link", { name: "Open the demo" })).toBeVisible();

    // SC-007: under 60 seconds of page time from the running table.
    const took = Date.now() - started;
    test.info().annotations.push({ type: "SC-007", description: `${took} ms, ${throws} throw(s)` });
    expect(took).toBeLessThan(60_000);
    expectClean(opened);
  });

  test("on screen, the room takes the table by itself, with no button", async ({ page }) => {
    const opened = await openLanding(page);
    // The landing's tests hold still; this one is about a page that moves.
    await page.emulateMedia({ reducedMotion: "no-preference" });
    await page.reload();
    await roomFrame(page).evaluate((el) => el.scrollIntoView({ block: "center" }));
    await expect(roomFrame(page)).toHaveClass(/engine-frame--running/, { timeout: BOOT_TIMEOUT });
    await expect(roomFrame(page).getByRole("button")).toHaveCount(0);
    await expect
      .poll(() => engine<{ named?: string; found?: boolean }>(page, "movement_state"))
      .toMatchObject({ named: HERO_ID, found: true });
    expectClean(opened);
  });

  test("a door out of reach, and Start over (US1 scenarios 5 and 8)", async ({ page }) => {
    const opened = await openLanding(page);
    await bootRoom(page);
    await recordSaid(page);
    await standsAt(page, "-3,1.5");

    // Scenario 5: no door in reach: the button says why, the hall stays dark.
    const door = room(page).getByRole("button", { name: "Open the door" });
    await expect(door).toBeDisabled();
    await expect(door).toHaveAccessibleDescription(/No door is within reach/);

    // Walk up, open the door, step through.
    for (let i = 0; i < 3; i += 1) await stepAndHear(page, "South", /door/);
    await room(page).getByRole("button", { name: "Open the door to the hall" }).click();
    await expect.poll(() => seen(page, HALL_POINT)).toBe(true);
    await stepAndHear(page, "South", /hall/);
    await standsAt(page, "-3,-2.5");

    // Scenario 8: back to the start on the table and in the page, the door
    // shut again, no goal done, and no reload.
    await page.evaluate(() => ((window as unknown as { kept: number }).kept = 1));
    await room(page).getByRole("button", { name: "Start over" }).click();
    await standsAt(page, "-3,1.5");
    await expect.poll(() => seen(page, HALL_POINT)).toBe(false);
    for (const id of ["door-reached", "door-opened", "chest-found", "chest-opened"]) {
      await expect(goal(page, id)).toContainText("(not yet)");
    }
    expect(await page.evaluate(() => (window as unknown as { kept?: number }).kept)).toBe(1);
    await expect(roomFrame(page)).toHaveClass(/engine-frame--running/);
    expectClean(opened);
  });
});
