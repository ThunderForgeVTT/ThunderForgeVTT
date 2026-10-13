import { expect, test } from "@playwright/test";
import {
  BOOT_TIMEOUT,
  BUILT,
  bootDiceFrame,
  engine,
  frames,
  landedOf,
  seeded,
} from "./support/engine.ts";
import { expectClean, openLanding } from "./support/page.ts";

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
