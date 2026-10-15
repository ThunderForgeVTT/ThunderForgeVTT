import { expect, test, type Page } from "@playwright/test";
import {
  BOOT_TIMEOUT,
  BUILT,
  bootDiceFrame,
  diceFrame,
  engine,
  frames,
  landed,
  landedOf,
  seeded,
  throwButton,
} from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";
import { bootRoom, engineHero, goal, pad, recordSaid, roomFrame, saidLog, square } from "./support/room.ts";

/**
 * Spec 091 T001: the probe. Can the one engine the dice tray booted move its
 * canvas into another element and keep drawing, throwing and taking keys?
 * The answer decides T002: one engine shared by two frames, or the reload.
 */

interface Movement {
  presses: number;
}

test.describe("one engine, moved", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
  });

  test("the canvas keeps drawing, throwing and taking keys in a new element", async ({ page }) => {
    const opened = await openLanding(page);
    await page.addInitScript(() => {
      (window as unknown as { lost: number }).lost = 0;
    });
    await bootDiceFrame(page);
    await page.evaluate(() => {
      const w = window as unknown as { lost: number };
      w.lost = 0;
      document
        .querySelector("#dice-table canvas")!
        .addEventListener("webglcontextlost", () => w.lost++);
    });

    const first = seeded("1d20+5", 3, "probe-before");
    await engine(page, "apply_world_command", first.json);
    await landedOf(page, "probe-before");

    // A board the movement system will act on: a grid, a token, control.
    const send = (command: Record<string, unknown>) =>
      engine(page, "apply_world_command", JSON.stringify(command));
    await send({ type: "set_scene_grid", gridType: "square", size: 117, visible: true, mapWidth: 1170, mapHeight: 702 });
    await send({ type: "upsert_token", token: { id: "probe-hero", x: 175, y: 175, z: 1, label: "Probe" } });
    await send({ type: "set_controlled_token", tokenId: "probe-hero" });

    // The new home: fixed, 960x540, and the tray left in view so its
    // observer does not pause the engine.
    await page.evaluate(() => {
      const host = document.createElement("div");
      host.id = "probe-host";
      Object.assign(host.style, {
        position: "fixed",
        right: "0",
        bottom: "0",
        width: "960px",
        height: "540px",
        zIndex: "50",
      });
      document.body.append(host);
      host.append(document.querySelector("#dice-table canvas")!);
    });
    await page.locator("#dice-table").scrollIntoViewIfNeeded();

    const a = await frames(page);
    await expect.poll(() => frames(page), { timeout: 10_000 }).toBeGreaterThan(a + 10);

    const after = seeded("4d6kh3", 5, "probe-after");
    await engine(page, "apply_world_command", after.json);
    const thrown = await landedOf(page, "probe-after");
    expect(thrown.dice.map((d) => [d.face, d.kept])).toEqual(
      after.detail.dice.map((d) => [d.final_value, d.kept]),
    );

    const shot = await page.locator("#probe-host canvas").screenshot();
    const colours = await page.evaluate(async (b64) => {
      const img = new Image();
      img.src = `data:image/png;base64,${b64}`;
      await img.decode();
      const c = document.createElement("canvas");
      [c.width, c.height] = [img.width, img.height];
      const ctx = c.getContext("2d")!;
      ctx.drawImage(img, 0, 0);
      const px = ctx.getImageData(0, 0, c.width, c.height).data;
      const seen = new Set<number>();
      for (let i = 0; i < px.length; i += 4 * 97) seen.add((px[i] << 16) | (px[i + 1] << 8) | px[i + 2]);
      return { size: [c.width, c.height], distinct: seen.size };
    }, shot.toString("base64"));
    console.log(`probe: canvas ${colours.size.join("x")}, ${colours.distinct} sampled colours`);
    expect(colours.distinct, "the moved canvas is not blank").toBeGreaterThan(8);

    const before = (await engine<Movement>(page, "movement_state")).presses;
    await page.locator("#probe-host canvas").focus();
    await page.keyboard.down("ArrowRight");
    await page.waitForTimeout(150);
    await page.keyboard.up("ArrowRight");
    await expect
      .poll(async () => (await engine<Movement>(page, "movement_state")).presses, { timeout: 5_000 })
      .toBeGreaterThan(before);

    expect(await page.evaluate(() => (window as unknown as { lost: number }).lost)).toBe(0);
    console.log(`probe: movement ${JSON.stringify(await engine(page, "movement_state"))}`);
    expectClean(opened);
  });
});

/**
 * Spec 091 T025, US3: the tray and the room share the one table. Whichever
 * frame asked last holds the canvas; the other shows its poster and a way
 * to bring the table back. Nothing is downloaded twice.
 */

/**
 * The engine's code and assets. The manifest is left out: the test's own
 * `engine()` probe reads it on every call. So are Bevy's `.meta` probes.
 */
const engineFiles = (requests: string[]) =>
  requests.filter((u) => new URL(u).pathname.startsWith("/demo/assets/") && !u.endsWith(".meta"));

/** Marks the canvas, so a later look can tell it is the same element. */
const markCanvas = (page: Page) =>
  page.evaluate(() => (document.getElementById("thunderforge-table")!.dataset.mark = "first"));
const canvasIn = (page: Page, frameId: string) =>
  page.evaluate((id) => document.querySelector(`#${id} #thunderforge-table`)?.getAttribute("data-mark") ?? null, frameId);

const chip = (page: Page, label: string) =>
  page.locator("#dice").getByRole("group", { name: "Example rolls" }).getByRole("button", { name: label });

test.describe("the tray and the room share one table", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(async ({ page }, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 180_000);
    await page.emulateMedia({ reducedMotion: "reduce" });
  });

  test("tray, then room, then back (US3 scenarios 1, 2 and 4)", async ({ page }) => {
    const opened = await openLanding(page);
    await bootDiceFrame(page);
    await chip(page, "Attack").click();
    await expect.poll(async () => (await landed(page)).length, { timeout: 30_000 }).toBe(1);
    await markCanvas(page);
    const fetched = engineFiles(opened.requests);
    expect(new Set(fetched).size, "each engine file once").toBe(fetched.length);

    // Scenario 1: the same canvas moves to the room, which sends its scene.
    await bootRoom(page);
    expect(await canvasIn(page, "room-table")).toBe("first");
    await expect.poll(() => engineHero(page)).toEqual({ x: 2, y: 3.5 });
    await expect(diceFrame(page)).toHaveClass(/engine-frame--away/);
    await expect(diceFrame(page)).toContainText("The table is in the room below.");
    await expect(diceFrame(page).getByRole("button", { name: "Bring it back" })).toBeVisible();
    expect(engineFiles(opened.requests), "no engine file fetched again").toEqual(fetched);

    // Scenario 2: the tray still rolls in words; nothing lands on the table.
    await chip(page, "Ability score").click();
    await expect(page.locator("#dice .total code")).toHaveText("4d6kh3");
    await page.waitForTimeout(1_500);
    expect(await landed(page)).toHaveLength(1);

    // Scenario 4: the room's frame, out of view, stops drawing.
    await page.evaluate(() => window.scrollTo(0, 0));
    await expect(roomFrame(page)).toHaveClass(/engine-frame--paused/);
    await page.waitForTimeout(300);
    const stopped = await frames(page);
    await page.waitForTimeout(1_000);
    expect(await frames(page), "frames drawn while out of view").toBe(stopped);

    // Back to the tray: the same canvas, the room's hero gone, a throw lands.
    await diceFrame(page).scrollIntoViewIfNeeded();
    await diceFrame(page).getByRole("button", { name: "Bring it back" }).click();
    await expect(diceFrame(page)).toHaveClass(/engine-frame--running/);
    expect(await canvasIn(page, "dice-table")).toBe("first");
    await expect(roomFrame(page)).toHaveClass(/engine-frame--away/);
    await expect.poll(() => engineHero(page)).toBeUndefined();
    await chip(page, "Fireball").click();
    await expect.poll(async () => (await landed(page)).length, { timeout: 30_000 }).toBe(2);
    expect(engineFiles(opened.requests)).toEqual(fetched);
    expectClean(opened);
  });

  test("room first, then the tray takes the table (US3 scenario 3)", async ({ page }) => {
    const opened = await openLanding(page);
    await bootRoom(page);
    await recordSaid(page);
    await markCanvas(page);

    // Goal 1 on the table: three steps to the door, the fourth refused.
    await pad(page, "North").focus();
    for (let i = 0; i < 4; i += 1) {
      const n = (await saidLog(page)).length;
      await page.keyboard.press("ArrowDown");
      await expect.poll(async () => (await saidLog(page)).length).toBe(n + 1);
    }
    await expect(goal(page, "door-reached")).toContainText("(done)");
    const fetched = engineFiles(opened.requests);

    await diceFrame(page).scrollIntoViewIfNeeded();
    await throwButton(page).click();
    await expect(diceFrame(page)).toHaveClass(/engine-frame--running/);
    expect(await canvasIn(page, "dice-table")).toBe("first");
    await expect(roomFrame(page)).toHaveClass(/engine-frame--away/);
    await expect.poll(() => engineHero(page)).toBeUndefined();

    // The room's words keep their state.
    expect(await square(page)).toBe("2,0.5");
    await expect(goal(page, "door-reached")).toContainText("(done)");

    await chip(page, "Attack").click();
    await expect.poll(async () => (await landed(page)).length, { timeout: 30_000 }).toBe(1);
    expect(engineFiles(opened.requests)).toEqual(fetched);
    expectClean(opened);
  });
});
