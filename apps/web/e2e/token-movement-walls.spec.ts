import { test, expect } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 045 US2 (ADR-095): the server refuses a player's move across a wall.
 *
 * Deliberately **not** through the canvas. The engine stops such a move before
 * it is sent, so a test that drags a token proves only that the engine is
 * working — and the engine is a program on somebody else's computer. This
 * sends the crossing move straight to the server from the player's own
 * authenticated session, which is exactly what a modified client would do, and
 * exactly what the server exists to refuse.
 *
 * The Game Master is checked too, in the other direction: walls rule players'
 * moves only (FR-017, owner's decision 1), so a Game Master putting a token
 * down on the far side of a wall must still work.
 */

const WALL_X = 100;

test.describe("Walls stop a move (spec 045 US2)", () => {
  test("a player's crossing move is refused by the server, and a Game Master's is not", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Movement Walls ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Walled Scene");

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

    // A wall down the middle, blocking movement.
    const wall = await graphql<{
      data?: { createWall?: { wallId: string } };
    }>(
      page,
      `
        mutation ($input: GraphQLCreateWallInput!) {
          createWall(input: $input) {
            wallId
          }
        }
      `,
      {
        input: {
          sceneId,
          x1: WALL_X,
          y1: -400,
          x2: WALL_X,
          y2: 400,
          blocksVision: true,
          blocksMovement: true,
        },
      },
    );
    expect(wall.data?.createWall?.wallId).toBeTruthy();

    // The player's own token, west of it.
    const token = await graphql<{
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
    const tokenId = token.data!.createToken!.tokenId;
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

    const move = (
      x: number,
      y: number,
      path?: { x: number; y: number }[],
    ) =>
      graphql<{
        data?: { moveOwnToken?: { tokenId: string; x: number } };
        errors?: { message: string }[];
      }>(
        player,
        `
          mutation (
            $tokenId: UUID!
            $x: Float!
            $y: Float!
            $path: [GraphQLPathPoint!]
          ) {
            moveOwnToken(tokenId: $tokenId, x: $x, y: $y, path: $path) {
              tokenId
              x
            }
          }
        `,
        { tokenId, x, y, path },
      );

    // Straight through it, with no client involved at all.
    const refused = await move(WALL_X + 100, 0);
    expect(
      refused.errors?.map((e) => e.message),
      "the server refuses a crossing move whoever sends it",
    ).toEqual(["A wall is in the way"]);
    expect(refused.data?.moveOwnToken).toBeFalsy();

    // And the refusal did not half-apply: the token is where it was.
    const after = await graphql<{
      data?: { tokens?: { tokenId: string; x: number }[] };
    }>(
      page,
      `
        query ($sceneId: UUID!) {
          tokens(sceneId: $sceneId) {
            tokenId
            x
          }
        }
      `,
      { sceneId },
    );
    expect(
      after.data?.tokens?.find((t) => t.tokenId === tokenId)?.x,
      "a refused move leaves the token at its last accepted position",
    ).toBe(0);

    // A move on its own side of the wall still works, so the refusal is about
    // the wall and not about the player.
    const allowed = await move(-50, 0);
    expect(allowed.errors).toBeUndefined();
    expect(allowed.data?.moveOwnToken?.x).toBe(-50);

    // Walking round the end of the wall, declared as a route, is allowed —
    // the endpoints alone would read as a crossing.
    const around = await move(WALL_X + 100, 0, [
      { x: -50, y: 500 },
      { x: WALL_X + 100, y: 500 },
    ]);
    expect(
      around.errors,
      "a route around the wall is not a route through it",
    ).toBeUndefined();
    expect(around.data?.moveOwnToken?.x).toBe(WALL_X + 100);

    // FR-017: the Game Master is never judged. Back across, their way.
    const gmMove = await graphql<{
      data?: { updateToken?: { x: number } };
      errors?: { message: string }[];
    }>(
      page,
      `
        mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
          updateToken(tokenId: $tokenId, input: $input) {
            x
          }
        }
      `,
      { tokenId, input: { x: -200, y: 0 } },
    );
    expect(
      gmMove.errors,
      "a Game Master moves any token anywhere — walls rule players' moves",
    ).toBeUndefined();
    expect(gmMove.data?.updateToken?.x).toBe(-200);

    await player.context().close();
  });
});
