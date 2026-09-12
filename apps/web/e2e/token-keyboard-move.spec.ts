import { test, expect, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { serverTokenPosition, tokenPosition } from "./fixtures/offline";
import { createScene } from "./fixtures/world-cache";

/**
 * Spec 045 US1: a player walks their own token with the keyboard.
 *
 * The playtest of 2026-09-11 found a key press moving nothing, and the fix had
 * two halves that failed independently: the engine was never told which token
 * the player may move (it drove only its own startup placeholder), and the move
 * it did make was announced as an event no web module handles. Either half
 * regressing puts the table back where it started, so this watches the whole
 * round trip — the press, the server's stored position, and a second client's
 * board — rather than the engine's own belief about where the token is.
 */

/** What the engine believes about control, reported when a press goes nowhere. */
async function movementState(page: Page): Promise<string> {
  return JSON.stringify(
    await page.evaluate(
      () =>
        (
          window as unknown as {
            __engineProbe?: { movementState?: () => unknown };
          }
        ).__engineProbe?.movementState?.() ?? null,
    ),
  );
}

test.describe("Keyboard movement (spec 045 US1)", () => {
  test("a player walks their own token east, and it reaches the server and the Game Master", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);

    await registerAndCreateWorld(page, `E2E Keyboard Move ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    const sceneId = await createScene(page, worldId, "Walking Scene");
    await graphql(
      page,
      `
        mutation ($sceneId: UUID!) {
          updateSceneHidden(sceneId: $sceneId, hidden: false) {
            sceneId
          }
        }
      `,
      { sceneId },
    );
    await launchSceneByName(page, worldId, "Walking Scene");

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

    // A character for the player, and a token for it that is theirs to move:
    // their primary owned token is both their eyes and their piece.
    const actor = await graphql<{ data?: { createActor?: { id: string } } }>(
      page,
      `
        mutation ($input: CreateActorInput!) {
          createActor(input: $input) {
            id
          }
        }
      `,
      {
        input: {
          worldId,
          label: "Aria",
          isNpc: false,
          gameSystemId: "genie",
        },
      },
    );
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
      {
        input: {
          sceneId,
          actorId: actor.data!.createActor!.id,
          x: 0,
          y: 0,
          tokenType: "character",
        },
      },
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

    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);

    // Both boards have the token before anyone touches the keyboard, so a
    // failure below is about the move and not about loading the scene.
    await expect
      .poll(() => tokenPosition(player, tokenId), { timeout: 20_000 })
      .not.toBeNull();
    await expect
      .poll(() => tokenPosition(page, tokenId), { timeout: 20_000 })
      .not.toBeNull();
    const before = (await tokenPosition(player, tokenId))!;

    await player.keyboard.press("d");

    // The server is the record: the player's own client agreeing with itself
    // would prove only that the engine moved something locally.
    await expect
      .poll(
        async () =>
          (await serverTokenPosition(page, sceneId, tokenId))?.x ?? null,
        {
          timeout: 20_000,
          message: `the press should persist one cell east (engine: ${await movementState(player)})`,
        },
      )
      .toBeGreaterThan(before.x);

    // And it reaches the Game Master's board, with no reload.
    await expect
      .poll(async () => (await tokenPosition(page, tokenId))?.x ?? null, {
        timeout: 20_000,
        message: "the Game Master's board follows the player's walk",
      })
      .toBeGreaterThan(before.x);

    await player.context().close();
  });
});
