import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import {
  doorCommand,
  HERO_ID,
  roomSetup,
  toWorld,
  type MapFile,
  type Point,
} from "@thunderforge/encounter";
import { HALL_POINT, roomScript } from "../src/room/script.ts";
import { BOOT_TIMEOUT, BUILT, bootDiceFrame, engine } from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";

/**
 * Spec 091 T031: the room script's facts, confirmed on the engine. The dice
 * frame boots the engine and the test sends the room's scene itself, so
 * this holds before the room's UI exists and keeps holding after.
 */

const file = JSON.parse(
  readFileSync(new URL("../public/maps/dungeon-sample.json", import.meta.url), "utf8"),
) as MapFile;
const script = roomScript(file);
const grid = script.map.gridSize;
const hero = { name: "Sir Pip", art: "/room/sir-pip.webp" };

interface Footprint {
  tokenId: string;
  x: number;
  y: number;
}
interface Carried {
  tokenId: string;
  x: number;
  y: number;
  bright: number;
  dim: number;
}

const send = (page: Page, command: object) =>
  engine(page, "apply_world_command", JSON.stringify(command));

async function heroAt(page: Page): Promise<Point | undefined> {
  const all = await engine<Footprint[]>(page, "token_footprints");
  const h = all.find((t) => t.tokenId === HERO_ID);
  return h && { x: h.x / grid, y: h.y / grid };
}

async function seen(page: Page, square: Point) {
  const at = toWorld(square, grid);
  return page.evaluate(
    async ([x, y]) => {
      const m = await (await fetch("/demo/engine.json")).json();
      const e = await import(`/demo/${m.js}`);
      return JSON.parse(e.sight_probe(x, y)) as { looking: boolean; seen: boolean };
    },
    [at.x, at.y] as const,
  );
}

test.describe("the room script on the engine", () => {
  test.skip(!BUILT, "no demo build: run pnpm -F @thunderforge/demo run build");
  test.beforeEach(({}, info) => {
    test.skip(info.project.name !== "desktop", "the engine boots at the desktop width only");
    test.setTimeout(BOOT_TIMEOUT + 120_000);
  });

  test("north, sight through the door, the chest in the torch's ring and the torch's units", async ({ page }) => {
    const opened = await openLanding(page);
    await bootDiceFrame(page);

    for (const command of roomSetup(script, hero, { reducedMotion: true })) {
      await send(page, command);
    }
    await engine(page, "set_viewer_token", HERO_ID);

    // The hero stands on the start square's centre.
    await expect.poll(() => heroAt(page)).toEqual(script.start);

    // The torch is carried, in world units.
    await expect
      .poll(async () => (await engine<Carried[]>(page, "carried_lights")).find((l) => l.tokenId === HERO_ID))
      .toMatchObject({
        bright: script.scene.torch.bright * grid,
        dim: script.scene.torch.dim * grid,
        ...toWorld(script.start, grid),
      });

    // Behind the shut door, the hall point is unseen.
    await expect.poll(() => seen(page, HALL_POINT)).toEqual({ looking: true, seen: false });

    // ArrowUp is north, and north is the script's sign of y.
    await page.locator("#thunderforge-table").focus();
    await page.keyboard.down("ArrowUp");
    await page.waitForTimeout(150);
    await page.keyboard.up("ArrowUp");
    await expect
      .poll(() => heroAt(page))
      .toEqual({ x: script.start.x, y: script.start.y + script.north });

    // Open the door: the hall point comes into sight.
    const door = script.map.walls.find((w) => w.id === script.goalDoor)!;
    await send(page, doorCommand(door, "open"));
    await expect.poll(() => seen(page, HALL_POINT)).toEqual({ looking: true, seen: true });

    // In the hall, the torch brings the chest into sight.
    await send(page, {
      type: "upsert_token",
      token: { id: HERO_ID, ...toWorld(HALL_POINT, grid), z: 2, label: hero.name, tokenType: "character", photoUrl: hero.art },
    });
    await expect.poll(() => seen(page, script.chest.square)).toEqual({ looking: true, seen: true });

    await page.locator("#thunderforge-table").screenshot({ path: test.info().outputPath("hall.png") });
    expectClean(opened);
  });
});
