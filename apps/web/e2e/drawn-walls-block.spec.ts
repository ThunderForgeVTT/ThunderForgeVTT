import { test, expect, type Page } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  openGmTool,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * A wall a Game Master *draws* stops a player (owner decision 2026-09-15).
 *
 * `token-movement-walls.spec.ts` proves the server refuses a move through a
 * wall — through a wall it created itself over GraphQL with
 * `blocksMovement: true` spelled out. Every other wall spec did the same. So
 * the rule was proven for walls nobody draws, while the wall tool wrote
 * `blocksMovement: false` on every segment, chain and room it made, and a
 * Game Master's room was walkable. Nothing here passes a profile: the walls
 * come from the tool on the canvas, and whatever the tool writes is what is
 * judged.
 *
 * The move is still sent straight to the server from the player's session,
 * for the reason that spec gives: the engine stopping a move proves only the
 * engine, and the server is what a modified client meets.
 */

type Wall = {
  wallId: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksVision: boolean;
  blocksMovement: boolean;
  doorState: string;
};

async function sceneWalls(page: Page, sceneId: string): Promise<Wall[]> {
  const res = await graphql<{ data?: { walls?: Wall[] } }>(
    page,
    `
      query ($sceneId: UUID!) {
        walls(sceneId: $sceneId) {
          wallId
          x1
          y1
          x2
          y2
          blocksVision
          blocksMovement
          doorState
        }
      }
    `,
    { sceneId },
  );
  return res.data?.walls ?? [];
}

/** Arm what a wall-tool drag draws, through the app's own engine bridge. */
async function setWallPrimitive(page: Page, primitive: string): Promise<void> {
  const recognised = await page.evaluate(async (p) => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    return bevy.setWallPrimitive(p);
  }, primitive);
  expect(recognised, `the engine should know the "${primitive}" primitive`).toBe(
    true,
  );
}

/** A real press-move-release across the canvas, offsets from its centre. */
async function dragOnCanvas(
  page: Page,
  from: { dx: number; dy: number },
  to: { dx: number; dy: number },
): Promise<void> {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("Bevy canvas element not found");
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx + from.dx, cy + from.dy);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.move(cx + to.dx, cy + to.dy, { steps: 12 });
  await page.waitForTimeout(80);
  await page.mouse.up();
}

async function wallsReach(
  page: Page,
  sceneId: string,
  count: number,
): Promise<Wall[]> {
  await expect
    .poll(async () => (await sceneWalls(page, sceneId)).length, {
      message: `the drawn walls should reach the server (expected ${count})`,
      timeout: 30_000,
    })
    .toBe(count);
  return sceneWalls(page, sceneId);
}

test.describe("Drawn walls stop a player (owner decision 2026-09-15)", () => {
  test("a Game Master draws a room and a door; the server keeps a player in the room and behind the closed door", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Drawn Walls ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Drawn Room");
    await launchSceneByName(page, worldId, "Drawn Room");
    await waitForEngineReady(page);
    await openGmTool(page, "walls");

    // --- A room, drawn with the tool ------------------------------------
    await setWallPrimitive(page, "room");
    await dragOnCanvas(page, { dx: -150, dy: -150 }, { dx: 150, dy: 150 });
    const room = await wallsReach(page, sceneId, 4);
    for (const wall of room) {
      expect(wall.doorState).toBe("NONE");
      expect(wall.blocksVision, "a drawn wall blocks vision").toBe(true);
      expect(wall.blocksMovement, "a drawn wall blocks movement").toBe(true);
    }
    const xs = room.flatMap((w) => [w.x1, w.x2]);
    const ys = room.flatMap((w) => [w.y1, w.y2]);
    const minX = Math.min(...xs);
    const maxX = Math.max(...xs);
    const minY = Math.min(...ys);
    const maxY = Math.max(...ys);
    const midX = (minX + maxX) / 2;
    const midY = (minY + maxY) / 2;
    expect(maxX - minX, "the room has width").toBeGreaterThan(0);
    expect(maxY - minY, "the room has height").toBeGreaterThan(0);

    // --- A plain dragged segment is a wall too ---------------------------
    await setWallPrimitive(page, "segment");
    await dragOnCanvas(page, { dx: -150, dy: 260 }, { dx: 150, dy: 260 });
    const withSegment = await wallsReach(page, sceneId, 5);
    const segment = withSegment.find(
      (w) => !room.some((r) => r.wallId === w.wallId),
    )!;
    expect(segment.doorState).toBe("NONE");
    expect(segment.blocksMovement, "a dragged segment blocks movement").toBe(
      true,
    );

    // --- A door, drawn with the tool, east of the room -------------------
    await setWallPrimitive(page, "door");
    await dragOnCanvas(page, { dx: 300, dy: -100 }, { dx: 300, dy: 100 });
    const withDoor = await wallsReach(page, sceneId, 6);
    const door = withDoor.find((w) => w.doorState !== "NONE")!;
    expect(door, "the door tool drew a door").toBeTruthy();
    expect(door.doorState, "a drawn door starts closed").toBe("CLOSED");
    expect(door.blocksMovement).toBe(true);
    const doorX = (door.x1 + door.x2) / 2;
    const doorMidY = (door.y1 + door.y2) / 2;
    expect(doorX, "the door is east of the room").toBeGreaterThan(maxX);

    // --- The player, and their token in the middle of the room -----------
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
    const playerUserId = me.data!.me!.id;
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
      { input: { sceneId, x: midX, y: midY, tokenType: "character" } },
    );
    const tokenId = created.data!.createToken!.tokenId;
    const gmPlace = (x: number, y: number) =>
      graphql<{ errors?: { message: string }[] }>(
        page,
        `
          mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
            updateToken(tokenId: $tokenId, input: $input) {
              tokenId
            }
          }
        `,
        { tokenId, input: { x, y } },
      );
    await graphql(
      page,
      `
        mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
          updateToken(tokenId: $tokenId, input: $input) {
            tokenId
          }
        }
      `,
      { tokenId, input: { ownerUserId: playerUserId, isPrimary: true } },
    );

    const move = (x: number, y: number) =>
      graphql<{
        data?: { moveOwnToken?: { tokenId: string; x: number } };
        errors?: { message: string }[];
      }>(
        player,
        `
          mutation ($tokenId: UUID!, $x: Float!, $y: Float!) {
            moveOwnToken(tokenId: $tokenId, x: $x, y: $y) {
              tokenId
              x
            }
          }
        `,
        { tokenId, x, y },
      );

    // Out through the east wall of the room: refused.
    const outEast = await move((maxX + doorX) / 2, midY);
    expect(
      outEast.errors?.map((e) => e.message),
      "the server refuses walking out through a drawn wall",
    ).toEqual(["A wall is in the way"]);

    // And through the south wall.
    const outSouth = await move(midX, maxY + (maxY - minY) / 4);
    expect(outSouth.errors?.map((e) => e.message)).toEqual([
      "A wall is in the way",
    ]);

    // Inside the room the player still walks: the refusal is about the wall.
    const inside = await move(midX + (maxX - minX) / 8, midY);
    expect(inside.errors, "a move inside the room is allowed").toBeUndefined();

    // --- The door: closed refuses, open allows ---------------------------
    // The Game Master puts the token between the room and the door; walls
    // rule players' moves, never the Game Master's.
    const westOfDoor = (maxX + doorX) / 2;
    const eastOfDoor = doorX + (doorX - maxX) / 2;
    const placed = await gmPlace(westOfDoor, doorMidY);
    expect(placed.errors).toBeUndefined();

    const throughClosed = await move(eastOfDoor, doorMidY);
    expect(
      throughClosed.errors?.map((e) => e.message),
      "a closed drawn door refuses a player",
    ).toEqual(["A wall is in the way"]);

    const opened = await graphql<{
      data?: { updateWall?: { doorState: string } };
      errors?: { message: string }[];
    }>(
      page,
      `
        mutation ($wallId: UUID!, $input: GraphQLUpdateWallInput!) {
          updateWall(wallId: $wallId, input: $input) {
            doorState
          }
        }
      `,
      { wallId: door.wallId, input: { doorState: "OPEN" } },
    );
    expect(opened.errors).toBeUndefined();
    expect(opened.data?.updateWall?.doorState).toBe("OPEN");

    const throughOpen = await move(eastOfDoor, doorMidY);
    expect(
      throughOpen.errors,
      "an open drawn door lets a player through",
    ).toBeUndefined();
    expect(throughOpen.data?.moveOwnToken?.x).toBe(eastOfDoor);

    await player.context().close();
  });
});
