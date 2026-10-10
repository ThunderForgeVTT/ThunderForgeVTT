import path from "node:path";
import { test, expect, type Page } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 088 US6 (SC-008): an imported map is walled at its edges.
 *
 * The ambush map is an outdoor one: the file has no walls at all, so before
 * US6 a token dragged off its side kept going into the dark. Imported with
 * the defaults it arrives with four walls, one on each edge, and a re-import
 * replaces them rather than adding four more.
 *
 * The move off the map is sent from the player's session straight to the
 * server, as `drawn-walls-block.spec.ts` sends its moves: the engine stopping
 * a drag proves only the engine, and the server is what a modified client
 * meets.
 */

const AMBUSH = path.resolve(
  __dirname,
  "../../../examples/maps/grassy-path-ambush.dd2vtt",
);

type Wall = {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  metadata: { perimeter?: boolean } | null;
};

type Board = { width: number; height: number; gridSize: number };

type ImportAnswer = { wallsCreated: number; perimeterWallsCreated: number };

async function board(page: Page, sceneId: string): Promise<Board> {
  const res = await graphql<{
    data?: { scene?: Board };
  }>(
    page,
    `
      query ($sceneId: UUID!) {
        scene(sceneId: $sceneId) {
          width
          height
          gridSize
        }
      }
    `,
    { sceneId },
  );
  expect(res.data?.scene, JSON.stringify(res)).toBeTruthy();
  return res.data!.scene!;
}

async function walls(page: Page, sceneId: string): Promise<Wall[]> {
  const res = await graphql<{ data?: { walls?: Wall[] } }>(
    page,
    `
      query ($sceneId: UUID!) {
        walls(sceneId: $sceneId) {
          x1
          y1
          x2
          y2
          metadata
        }
      }
    `,
    { sceneId },
  );
  return res.data?.walls ?? [];
}

/** Whether a wall runs along one edge of the board, centred and y up. */
function onAnEdge(wall: Wall, { width, height }: Board): boolean {
  const near = (a: number, b: number) => Math.abs(Math.abs(a) - b) <= 0.5;
  return (
    (wall.x1 === wall.x2 && near(wall.x1, width / 2)) ||
    (wall.y1 === wall.y2 && near(wall.y1, height / 2))
  );
}

/**
 * Import a map from the Settings dock, with **Wall the map's edges** as
 * asked. Waits on the upload's own answer: after the first import the
 * success panel is already showing, so it says nothing about the second.
 */
async function importMap(
  page: Page,
  wallEdges: boolean,
): Promise<ImportAnswer> {
  const tool = page.locator('[data-testid="map-import-tool"]:visible').first();
  if (!(await tool.isVisible().catch(() => false))) {
    await page.getByTestId("world-dock-tab-settings").click();
    await expect(tool).toBeVisible({ timeout: 20_000 });
  }
  const box = tool.getByTestId("map-import-wall-edges");
  await expect(box, "the box is ticked by default").toBeChecked();
  if (!wallEdges) {
    await box.click();
    await expect(box).not.toBeChecked();
  }
  const answered = page.waitForResponse(
    (r) => r.url().includes("/import/uvtt") && r.request().method() === "POST",
    { timeout: 90_000 },
  );
  await tool
    .locator('input[type="file"]')
    .setInputFiles(AMBUSH, { timeout: 30_000 });
  const response = await answered;
  expect(response.status()).toBe(200);
  return (await response.json()) as ImportAnswer;
}

test.describe("An imported map is walled at its edges", () => {
  test("the ambush map arrives with four edge walls that keep a player's token on it", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Edge Walls ${uniqueSuffix()}`,
    );
    const sceneId = await createScene(page, worldId, "Ambush");
    await launchSceneByName(page, worldId, "Ambush");
    await waitForEngineReady(page);

    const first = await importMap(page, true);
    expect(first).toMatchObject({ wallsCreated: 4, perimeterWallsCreated: 4 });
    await expect(page.getByTestId("map-import-success")).toContainText(
      "4 of the walls at the map's edges",
    );

    const size = await board(page, sceneId);
    const edges = await walls(page, sceneId);
    expect(edges, "four walls, the map has none of its own").toHaveLength(4);
    for (const wall of edges) {
      expect(onAnEdge(wall, size), JSON.stringify(wall)).toBe(true);
      expect(wall.metadata?.perimeter).toBe(true);
    }

    // --- A player's token, ten cells past each edge ----------------------
    const player = await inviteAndJoinAsPlayer(browser, page, worldId);
    const me = await graphql<{ data?: { me?: { id: string } } }>(
      player,
      `
        query {
          me {
            id
          }
        }
      `,
      {},
    );
    const created = await graphql<{
      data?: { createToken?: { tokenId: string } };
    }>(
      page,
      `
        mutation ($input: GraphQLCreateTokenInput!) {
          createToken(input: $input) {
            tokenId
          }
        }
      `,
      { input: { sceneId, x: 0, y: 0, tokenType: "character" } },
    );
    const tokenId = created.data!.createToken!.tokenId;
    await graphql(
      page,
      `
        mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
          updateToken(tokenId: $tokenId, input: $input) {
            tokenId
          }
        }
      `,
      { tokenId, input: { ownerUserId: me.data!.me!.id, isPrimary: true } },
    );
    const move = (x: number, y: number) =>
      graphql<{ errors?: { message: string }[] }>(
        player,
        `
          mutation ($tokenId: UUID!, $x: Float!, $y: Float!) {
            moveOwnToken(tokenId: $tokenId, x: $x, y: $y) {
              tokenId
            }
          }
        `,
        { tokenId, x, y },
      );
    const past = 10 * size.gridSize;
    for (const [x, y] of [
      [size.width / 2 + past, 0],
      [-size.width / 2 - past, 0],
      [0, size.height / 2 + past],
      [0, -size.height / 2 - past],
    ]) {
      const off = await move(x, y);
      expect(
        off.errors?.map((e) => e.message),
        `a move to (${x}, ${y}) leaves the map`,
      ).toEqual(["A wall is in the way"]);
    }
    const inside = await move(size.gridSize, 0);
    expect(inside.errors, "a move on the map is allowed").toBeUndefined();

    // --- A re-import replaces the edge walls -----------------------------
    const again = await importMap(page, true);
    expect(again.perimeterWallsCreated).toBe(4);
    expect(await walls(page, sceneId), "four, not eight").toHaveLength(4);

    await player.context().close();
  });

  test("with the box unticked the map arrives with no edge walls", async ({
    page,
  }) => {
    test.setTimeout(240_000);

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Open Edges ${uniqueSuffix()}`,
    );
    const sceneId = await createScene(page, worldId, "Open Field");
    await launchSceneByName(page, worldId, "Open Field");
    await waitForEngineReady(page);

    const answer = await importMap(page, false);
    expect(answer).toMatchObject({ wallsCreated: 0, perimeterWallsCreated: 0 });
    expect(await walls(page, sceneId)).toHaveLength(0);
  });
});
