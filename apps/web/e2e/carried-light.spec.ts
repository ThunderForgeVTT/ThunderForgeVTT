import { test, expect, type Page } from "./fixtures/test";
import { graphql, waitForEngineReady } from "./fixtures/helpers";
import { serverTokenPosition, tokenPosition } from "./fixtures/offline";
import {
  carriedLightOf,
  hiddenTokens,
  luminanceAt,
  storeCounts,
  zoomOutTo,
} from "./fixtures/lightingProbe";
import {
  GRID,
  addWall,
  feet,
  joinPlayer,
  openDarkDnd5eScene,
  placeCreature,
  setTraits,
  type VisionTable,
} from "./fixtures/visionTable";

/**
 * Spec 045 FR-061 and FR-064: a game system sets the light a character
 * carries (tasks.md T065).
 *
 * The owner's decision of 2026-09-14: a carried light is a light attached to
 * its token, not part of the token's eyes. So a torch on Brom's sheet has to
 * do what a torch at a table does — light the room for everyone, Aria
 * included, be stopped by a wall like any other light, and go where Brom
 * goes. Before T065 the server resolved the torch's reach from the sheet and
 * the web dropped it on the floor, so none of this happened anywhere.
 *
 * Watched from Aria's board, which has no token of its own and so shows the
 * board as it is lit (FR-035): by pixels, as `scene-lighting.spec.ts` reads
 * per-light shadows, and by which tokens her engine hides.
 */

const TORCH = { light_bright: 20, light_dim: 40 };

/** The bright surface under the darkness, so lit and unlit differ by pixels. */
async function addFloor(table: VisionTable): Promise<void> {
  const result = await graphql<{ errors?: unknown }>(
    table.gm,
    `
      mutation ($input: GraphQLCreateShapeInput!) {
        createShape(input: $input) {
          shapeId
        }
      }
    `,
    {
      input: {
        sceneId: table.sceneId,
        kind: "RECT",
        geometry: { x: -1200, y: -800, w: 2400, h: 1600 },
        // A shape is the Game Master's until shown; Aria has to see the floor
        // for her pixels to say anything about the light on it.
        visibleToPlayers: true,
      },
    },
  );
  expect(result.errors).toBeUndefined();
}

test.describe("A carried light (spec 045 T065)", () => {
  test("Brom's torch lights the room on Aria's board, a wall shadows it, and it walks with him", async ({
    page,
    browser,
  }) => {
    test.setTimeout(360_000);

    const table = await openDarkDnd5eScene(page, "Torchlit Crypt");
    const aria = await joinPlayer(browser, table);
    const brom = await joinPlayer(browser, table);
    await addFloor(table);

    // Cell centres on a 50-unit grid. Brom's torch is bright to 200 and dim
    // to 400. A wall stands 100 east of him; the rat is 200 north of him with
    // nothing between, and the goblin is behind the wall, well inside reach.
    const start = { x: 25, y: 25 };
    const bromCast = await placeCreature(table, {
      label: "Brom",
      ...start,
      ownerUserId: brom.userId,
    });
    await setTraits(table, bromCast.actorId, { class: "fighter", level: 3 });
    const rat = await placeCreature(table, {
      label: "Rat",
      x: start.x,
      y: start.y + feet(20),
    });
    const goblin = await placeCreature(table, {
      label: "Goblin",
      x: start.x + feet(20),
      y: start.y + feet(5),
    });
    await addWall(
      table,
      { x: start.x + 2 * GRID, y: start.y - 4 * GRID },
      { x: start.x + 2 * GRID, y: start.y + 4 * GRID },
    );

    for (const who of [aria, brom]) {
      await who.page.goto(`/world/${table.worldId}/play`);
      await waitForEngineReady(who.page);
      await expect
        .poll(() => storeCounts(who.page), { timeout: 20_000 })
        .toMatchObject({ tokens: 3, walls: 1, shapes: 1 });
    }

    // Before the torch: the dark hides the rat from Aria, and there is no
    // carried light anywhere to find.
    await expect
      .poll(() => hiddenTokens(aria.page), {
        timeout: 20_000,
        message: "in the dark, with nothing lit, Aria's board hides the rat",
      })
      .toContain(rat.tokenId);
    expect(await carriedLightOf(aria.page, bromCast.tokenId)).toBeNull();

    // Brom takes up a torch — on his sheet, as a Game Master would write it.
    await setTraits(table, bromCast.actorId, {
      class: "fighter",
      level: 3,
      ...TORCH,
    });
    await expect
      .poll(() => carriedLightOf(aria.page, bromCast.tokenId), {
        timeout: 20_000,
        message:
          "Brom's torch reaches Aria's engine as a light on Brom's token, " +
          `bright to ${feet(20)} and dim to ${feet(40)}`,
      })
      .toMatchObject({ bright: feet(20), dim: feet(40) });
    const lit = (await carriedLightOf(aria.page, bromCast.tokenId))!;
    const bromOnAria = (await tokenPosition(aria.page, bromCast.tokenId))!;
    expect(
      Math.hypot(lit.x - bromOnAria.x, lit.y - bromOnAria.y),
      "and it burns where Brom stands on her board",
    ).toBeLessThan(1);

    await expect
      .poll(() => hiddenTokens(aria.page), {
        timeout: 20_000,
        message: "Brom's torch lights the rat for Aria, not only for Brom",
      })
      .not.toContain(rat.tokenId);
    expect(
      await hiddenTokens(aria.page),
      "the wall stops Brom's torch short of the goblin, so it stays dark",
    ).toContain(goblin.tokenId);

    // By pixels: 150 west and 150 south of Brom, open floor, lit; 150 east,
    // behind the wall, as dark as the floor far from any light.
    await zoomOutTo(aria.page, 2.5);
    const west = { x: lit.x - 150, y: lit.y };
    const south = { x: lit.x, y: lit.y - 150 };
    const east = { x: lit.x + 150, y: lit.y };
    const far = { x: 625, y: -325 };
    await expect(async () => {
      const [w, s, e, r] = await luminanceAt(aria.page, [
        west,
        south,
        east,
        far,
      ]);
      const reading = `W=${w.toFixed(1)} S=${s.toFixed(1)} E=${e.toFixed(1)} far=${r.toFixed(1)}`;
      expect(w, `west of Brom is lit (${reading})`).toBeGreaterThan(r + 60);
      expect(s, `south of Brom is lit (${reading})`).toBeGreaterThan(r + 60);
      expect(
        e,
        `east of Brom, behind the wall, is shadowed (${reading})`,
      ).toBeLessThan(w - 60);
    }).toPass({ timeout: 20_000 });

    // Brom walks west with the keyboard, eight cells. Each step waits for the
    // server, so the presses are eight moves and not one burst the engine
    // could fold together.
    for (let step = 1; step <= 8; step += 1) {
      const before = (await serverTokenPosition(
        table.gm,
        table.sceneId,
        bromCast.tokenId,
      ))!;
      await brom.page.keyboard.press("a");
      await expect
        .poll(
          async () =>
            (
              await serverTokenPosition(
                table.gm,
                table.sceneId,
                bromCast.tokenId,
              )
            )?.x ?? null,
          {
            timeout: 20_000,
            message:
              `Brom's step ${step} west persists (Brom's board has him at ` +
              `${JSON.stringify(await tokenPosition(brom.page, bromCast.tokenId))}; ` +
              `engine: ${JSON.stringify(
                await brom.page.evaluate(
                  () =>
                    (
                      window as unknown as {
                        __engineProbe?: { movementState?: () => unknown };
                      }
                    ).__engineProbe?.movementState?.() ?? null,
                ),
              )})`,
          },
        )
        .toBeLessThan(before.x);
    }
    const walked = (await serverTokenPosition(
      table.gm,
      table.sceneId,
      bromCast.tokenId,
    ))!;
    expect(walked.x).toBeLessThanOrEqual(start.x - feet(35));

    // On Aria's board that walk is a remote move, and the torch goes with it.
    await expect
      .poll(
        async () => (await carriedLightOf(aria.page, bromCast.tokenId))?.x,
        {
          timeout: 20_000,
          message: "the torch follows Brom across Aria's board",
        },
      )
      .toBeCloseTo(walked.x, 0);
    await expect
      .poll(() => hiddenTokens(aria.page), {
        timeout: 20_000,
        message: "with Brom gone west, the rat is in the dark again",
      })
      .toContain(rat.tokenId);

    const moved = (await carriedLightOf(aria.page, bromCast.tokenId))!;
    const newWest = { x: moved.x - 150, y: moved.y };
    await expect(async () => {
      const [w, s, r] = await luminanceAt(aria.page, [newWest, south, far]);
      const reading = `W'=${w.toFixed(1)} old S=${s.toFixed(1)} far=${r.toFixed(1)}`;
      expect(
        w,
        `the floor beside Brom now is lit (${reading})`,
      ).toBeGreaterThan(r + 60);
      expect(
        s,
        `where he stood, the floor has gone dark (${reading})`,
      ).toBeLessThan(w - 60);
    }).toPass({ timeout: 20_000 });

    // Put away, it goes out on every board — not left burning until a reload.
    await setTraits(table, bromCast.actorId, { class: "fighter", level: 3 });
    await expect
      .poll(() => carriedLightOf(aria.page, bromCast.tokenId), {
        timeout: 20_000,
        message: "Brom puts his torch away, and Aria's board loses it",
      })
      .toBeNull();

    // And a torch goes with the token that carried it.
    await setTraits(table, bromCast.actorId, {
      class: "fighter",
      level: 3,
      ...TORCH,
    });
    await expect
      .poll(() => carriedLightOf(aria.page, bromCast.tokenId), {
        timeout: 20_000,
      })
      .not.toBeNull();
    await deleteToken(table.gm, bromCast.tokenId);
    await expect
      .poll(() => carriedLightOf(aria.page, bromCast.tokenId), {
        timeout: 20_000,
        message: "Brom's token is removed, and his torch with it",
      })
      .toBeNull();

    await aria.page.context().close();
    await brom.page.context().close();
  });
});

async function deleteToken(gm: Page, tokenId: string): Promise<void> {
  const result = await graphql<{ errors?: unknown }>(
    gm,
    `
      mutation ($tokenId: UUID!) {
        deleteToken(tokenId: $tokenId)
      }
    `,
    { tokenId },
  );
  expect(result.errors).toBeUndefined();
}
