import { test, expect, type Page } from "./fixtures/test";
import { graphql, waitForEngineReady } from "./fixtures/helpers";
import { clickBoard } from "./fixtures/boardPointer";
import {
  luminanceAt,
  placedLightOf,
  storeCounts,
  zoomOutTo,
} from "./fixtures/lightingProbe";
import {
  feet,
  joinPlayer,
  openDarkDnd5eScene,
  type VisionTable,
} from "./fixtures/visionTable";

/**
 * Spec 045 FR-061: a Game Master sets a placed light's bright reach apart from
 * its dim reach, and FR-062: a light saved with one radius keeps its look.
 *
 * Before this a stored light had one radius, drawn bright to half of it and
 * dim to the whole. Now the Lights panel names both, in the game system's own
 * units — twenty feet bright and forty dim, a torch as the rulebook gives it —
 * and every seat's engine lights the board by the two numbers the server
 * stored. Watched on the Game Master's board and two players', by what each
 * engine lights by and, on a player's board, by the pixels.
 */

const LIGHT_AT = { x: 25, y: 25 };

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
        visibleToPlayers: true,
      },
    },
  );
  expect(result.errors).toBeUndefined();
}

async function storedLight(
  gm: Page,
  sceneId: string,
): Promise<{ lightId: string; radius: number; brightRadius: number }> {
  const result = await graphql<{
    data?: {
      lightSources: { lightId: string; radius: number; brightRadius: number }[];
    };
  }>(
    gm,
    `
      query ($sceneId: UUID!) {
        lightSources(sceneId: $sceneId) {
          lightId
          radius
          brightRadius
        }
      }
    `,
    { sceneId },
  );
  return result.data!.lightSources[0];
}

test.describe("A placed light's two reaches (spec 045 FR-061)", () => {
  test("the Game Master sets 20 ft bright and 40 ft dim in the Lights panel, and every seat lights by it", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const table = await openDarkDnd5eScene(page, "Lamplit Hall");
    const aria = await joinPlayer(browser, table);
    const brom = await joinPlayer(browser, table);
    await addFloor(table);

    // A light saved the old way: one radius, no bright reach named.
    const created = await graphql<{
      data?: { createLightSource: { lightId: string; brightRadius: number } };
      errors?: unknown;
    }>(
      table.gm,
      `
        mutation ($input: GraphQLCreateLightSourceInput!) {
          createLightSource(input: $input) {
            lightId
            brightRadius
          }
        }
      `,
      {
        input: {
          sceneId: table.sceneId,
          ...LIGHT_AT,
          radius: feet(10),
          intensity: 1,
          castsShadows: true,
        },
      },
    );
    expect(created.errors).toBeUndefined();
    const lightId = created.data!.createLightSource.lightId;
    expect(
      created.data!.createLightSource.brightRadius,
      "a light given one radius is bright to half of it (FR-062)",
    ).toBe(feet(5));

    const seats = [
      ["the Game Master", table.gm],
      ["Aria", aria.page],
      ["Brom", brom.page],
    ] as const;
    for (const [, seat] of seats) {
      await seat.goto(`/world/${table.worldId}/play`);
      await waitForEngineReady(seat);
      await expect
        .poll(() => storeCounts(seat), { timeout: 20_000 })
        .toMatchObject({ lights: 1, shapes: 1 });
    }
    for (const [who, seat] of seats) {
      await expect
        .poll(() => placedLightOf(seat, lightId), {
          timeout: 20_000,
          message: `${who}'s engine lights the old light bright to half its radius`,
        })
        .toMatchObject({ bright: feet(5), dim: feet(10) });
    }

    await test.step("the Game Master selects the light and sets its reaches in feet", async () => {
      await table.gm.getByTestId("gm-tool-lights").click();
      const bright = table.gm.getByTestId("light-bright-reach");
      await expect(async () => {
        await clickBoard(table.gm, LIGHT_AT);
        await expect(bright).toBeVisible({ timeout: 2_000 });
      }).toPass({ timeout: 20_000 });

      const dim = table.gm.getByTestId("light-dim-reach");
      await expect(table.gm.getByText("Bright reach (ft)")).toBeVisible();
      await expect(bright).toHaveValue("5");
      await expect(dim).toHaveValue("10");

      await dim.fill("40");
      await dim.press("Enter");
      await bright.fill("20");
      await bright.press("Enter");
    });

    await test.step("the server stores both, in world units", async () => {
      await expect
        .poll(() => storedLight(table.gm, table.sceneId), { timeout: 15_000 })
        .toMatchObject({ lightId, radius: feet(40), brightRadius: feet(20) });
    });

    await test.step("every seat's engine lights by 20 ft bright and 40 ft dim", async () => {
      for (const [who, seat] of seats) {
        await expect
          .poll(() => placedLightOf(seat, lightId), {
            timeout: 15_000,
            message: `${who}'s board is bright to ${feet(20)} and dim to ${feet(40)}`,
          })
          .toMatchObject({ bright: feet(20), dim: feet(40) });
      }
    });

    await test.step("on a player's board, bright, dim and dark are three different floors", async () => {
      await zoomOutTo(aria.page, 2.5);
      // West, east and further east of the light, on open floor: 15 ft is
      // inside the bright reach, 30 ft inside the dim ring, 55 ft beyond it.
      const inBright = { x: LIGHT_AT.x - feet(15), y: LIGHT_AT.y };
      const inDim = { x: LIGHT_AT.x + feet(30), y: LIGHT_AT.y };
      const inDark = { x: LIGHT_AT.x + feet(55), y: LIGHT_AT.y };
      await expect(async () => {
        const [b, d, k] = await luminanceAt(aria.page, [
          inBright,
          inDim,
          inDark,
        ]);
        const reading = `bright=${b.toFixed(1)} dim=${d.toFixed(1)} dark=${k.toFixed(1)}`;
        expect(
          b,
          `15 ft out is brighter than 30 ft out (${reading})`,
        ).toBeGreaterThan(d + 10);
        expect(
          d,
          `30 ft out is lit, unlike 55 ft out (${reading})`,
        ).toBeGreaterThan(k + 10);
      }).toPass({ timeout: 20_000 });
    });

    await test.step("the panel says what was stored after a reload", async () => {
      await table.gm.reload();
      await waitForEngineReady(table.gm);
      await table.gm.getByTestId("gm-tool-lights").click();
      const bright = table.gm.getByTestId("light-bright-reach");
      await expect(async () => {
        await clickBoard(table.gm, LIGHT_AT);
        await expect(bright).toBeVisible({ timeout: 2_000 });
      }).toPass({ timeout: 20_000 });
      await expect(bright).toHaveValue("20");
      await expect(table.gm.getByTestId("light-dim-reach")).toHaveValue("40");

      // A bright reach past the dim one is refused in the panel, not stored.
      await bright.fill("50");
      await bright.press("Enter");
      await expect(table.gm.getByTestId("light-reach-problem")).toBeVisible();
      await table.gm.waitForTimeout(1_000);
      expect(await storedLight(table.gm, table.sceneId)).toMatchObject({
        radius: feet(40),
        brightRadius: feet(20),
      });
    });

    await aria.page.context().close();
    await brom.page.context().close();
  });
});
