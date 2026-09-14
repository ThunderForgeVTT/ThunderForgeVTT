import { test, expect, type Page } from "./fixtures/test";
import { waitForEngineReady } from "./fixtures/helpers";
import { dimTokens, hiddenTokens, storeCounts } from "./fixtures/lightingProbe";
import {
  feet,
  joinPlayer,
  openDarkDnd5eScene,
  placeCreature,
  setTraits,
} from "./fixtures/visionTable";

/**
 * Spec 045 SC-007, on a board (tasks.md T066).
 *
 * The range rule was proven only inside `thunderforge-canvas-core`
 * (`darkvision_reveals_the_dark_only_dimly_and_only_in_range`), and the sixty
 * feet were proven only to *arrive* at the engine (`combat-5e.playtest.ts`).
 * Nothing had watched the two meet on a player's canvas. This does: a sheet
 * says sixty feet, the server turns that into world units on this scene's
 * grid, and Aria's board shows the goblin fifty feet off — dimly — and not the
 * orc at seventy. Brom, whose sheet says nothing about darkvision, is shown
 * neither.
 *
 * Asked of each player's own engine, through the probe `scene-lighting.spec.ts`
 * reads: which tokens this canvas hides, and which it draws dimly.
 */

/** What this client's engine believes one token sees in the dark. */
async function darkvisionOn(
  page: Page,
  tokenId: string,
): Promise<number | null> {
  return page.evaluate(
    (id) =>
      (
        window as unknown as {
          __engineProbe?: { tokenVision?: (id: string) => number | null };
        }
      ).__engineProbe?.tokenVision?.(id) ?? null,
    tokenId,
  );
}

test.describe("Darkvision's reach on a board (spec 045 SC-007)", () => {
  test("sixty feet of darkvision shows a goblin at fifty feet dimly and not an orc at seventy; without it, neither", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const table = await openDarkDnd5eScene(page, "Dark Hall");
    const aria = await joinPlayer(browser, table);
    const brom = await joinPlayer(browser, table);

    // On cell centres of a 50-unit grid, in a row, so every distance is the
    // one the step names and snapping has nothing to move.
    const at = { x: 25, y: 25 };
    const ariaCast = await placeCreature(table, {
      label: "Aria",
      ...at,
      ownerUserId: aria.userId,
    });
    const bromCast = await placeCreature(table, {
      label: "Brom",
      x: at.x,
      y: at.y - feet(10),
      ownerUserId: brom.userId,
    });
    const goblin = await placeCreature(table, {
      label: "Goblin",
      x: at.x + feet(50),
      y: at.y,
    });
    const orc = await placeCreature(table, {
      label: "Orc",
      x: at.x + feet(70),
      y: at.y,
    });
    await setTraits(table, ariaCast.actorId, {
      class: "fighter",
      level: 3,
      darkvision: 60,
    });
    await setTraits(table, bromCast.actorId, { class: "wizard", level: 3 });

    for (const player of [aria, brom]) {
      await player.page.goto(`/world/${table.worldId}/play`);
      await waitForEngineReady(player.page);
      await expect
        .poll(() => storeCounts(player.page), { timeout: 20_000 })
        .toMatchObject({ tokens: 4 });
    }

    // The chain first, so a miss below is about the range and not about the
    // sixty feet never arriving.
    await expect
      .poll(() => darkvisionOn(aria.page, ariaCast.tokenId), {
        timeout: 20_000,
        message: `Aria's sixty feet reach her engine as ${feet(60)} world units`,
      })
      .toBeCloseTo(feet(60), 0);
    expect(
      await darkvisionOn(brom.page, bromCast.tokenId),
      "Brom's sheet names no darkvision, so he has none",
    ).toBe(0);

    // Aria: the goblin inside her sixty feet, dimly; the orc beyond them, not.
    await expect
      .poll(() => dimTokens(aria.page), {
        timeout: 20_000,
        message: "Aria's board shows the goblin at fifty feet, dimly",
      })
      .toContain(goblin.tokenId);
    expect(await hiddenTokens(aria.page), "and does not hide it").not.toContain(
      goblin.tokenId,
    );
    await expect
      .poll(() => hiddenTokens(aria.page), {
        timeout: 20_000,
        message: "Aria's board hides the orc at seventy feet",
      })
      .toContain(orc.tokenId);

    // Brom: no darkvision, no light — the dark hides both.
    await expect
      .poll(() => hiddenTokens(brom.page), {
        timeout: 20_000,
        message: "Brom's board, with no darkvision, shows neither",
      })
      .toEqual(expect.arrayContaining([goblin.tokenId, orc.tokenId]));
    expect(await dimTokens(brom.page), "not even dimly").not.toContain(
      goblin.tokenId,
    );

    await aria.page.context().close();
    await brom.page.context().close();
  });
});
