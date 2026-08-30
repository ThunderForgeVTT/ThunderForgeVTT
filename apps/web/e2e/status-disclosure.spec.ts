import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, User Story 3 — a player cannot learn what the Game Master is
 * withholding.
 *
 * # Why this asserts the network payload and not the screen
 *
 * A screen assertion passes against a client that *received* the exact value
 * and chose not to draw it. That is not the property this feature promises:
 * the promise is that the figure never reaches the player's machine at all,
 * so that no bug, devtools session, or modified client can surface it.
 *
 * So this intercepts the real GraphQL response in the player's browser and
 * asserts the absence of the number. `status_display.rs` asserts the same
 * property at the resolver; this proves it survives the whole way out — which
 * is the difference between a rule that holds in a unit test and one that
 * holds in the product.
 *
 * # Why two browsers
 *
 * The claim is comparative. "The player sees a band" is only interesting
 * beside "the Game Master, at the same moment, on the same token, sees the
 * number" — otherwise a feature that showed nobody anything would pass.
 */

/** The exact figures the NPC is given. Chosen to be unmistakable in a payload. */
const NPC_CURRENT = 37;
const NPC_MAX = 250;

async function setResources(
  page: Page,
  actorId: string,
  data: Record<string, number>,
): Promise<void> {
  const res = await graphql<{
    data?: { updateActorSystemData?: { id: string } };
  }>(
    page,
    `
      mutation ($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) {
          id
        }
      }
    `,
    {
      input: {
        actorId,
        gameSystemId: "genie",
        dataType: "resource_data",
        data,
      },
    },
  );
  if (!res.data?.updateActorSystemData?.id) {
    throw new Error(`resource_data was not stored: ${JSON.stringify(res)}`);
  }
}

/** Capture the `tokenStatus` response body this page receives. */
function captureStatusPayload(page: Page): Promise<string> {
  return new Promise((resolve) => {
    page.on("response", (response) => {
      if (!response.url().includes("/api/graphql")) return;
      void response
        .text()
        .then((body) => {
          if (body.includes("tokenStatus")) resolve(body);
        })
        .catch(() => {
          // A body that cannot be read is not the assertion's business.
        });
    });
  });
}

test("a player is never sent an NPC's exact figures, while the GM is", async ({
  page,
  browser,
}) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Disclosure ${suffix}`);

  const active = await graphql<{
    data?: { world?: { activeSceneId: string | null } };
  }>(
    page,
    `
      query ($worldId: UUID!) {
        world(worldId: $worldId) {
          activeSceneId
        }
      }
    `,
    { worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.data?.world?.activeSceneId ?? firstScene;

  await graphql(
    page,
    `
      mutation ($input: UpdateWorldGameSystemInput!) {
        updateWorldGameSystem(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, gameSystemId: "genie" } },
  );

  // An NPC — which is what makes the derived default `chunked` for players,
  // with nobody configuring anything.
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
        label: `Ogre ${suffix}`,
        isNpc: true,
        gameSystemId: "genie",
      },
    },
  );
  const actorId = actor.data?.createActor?.id;
  expect(actorId, `the NPC must exist: ${JSON.stringify(actor)}`).toBeTruthy();

  await setResources(page, actorId!, {
    current_health: NPC_CURRENT,
    max_health: NPC_MAX,
    current_wish_points: 0,
    max_wish_points: 0,
  });

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
    { input: { sceneId, x: 0, y: 0, actorId, tokenType: "npc" } },
  );
  const tokenId = created.data?.createToken?.tokenId;
  expect(
    tokenId,
    `the token must exist: ${JSON.stringify(created)}`,
  ).toBeTruthy();

  // --- the Game Master's view -------------------------------------------

  const gmPayload = captureStatusPayload(page);
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);
  const gmBody = await gmPayload;

  expect(
    gmBody,
    "the Game Master must be sent the real number — they have to run the fight",
  ).toContain(String(NPC_CURRENT));
  expect(gmBody).toContain('"disclosure":"visible"');

  // --- the player's view, on the same token, at the same time -----------

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);
  const playerPayload = captureStatusPayload(playerPage);
  await playerPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(playerPage);
  const playerBody = await playerPayload;

  // The assertion this whole feature exists for.
  expect(
    playerBody,
    `the player's payload must not contain the exact current value: ${playerBody}`,
  ).not.toContain(String(NPC_CURRENT));
  expect(
    playerBody,
    "nor the maximum, which is what makes a percentage recoverable",
  ).not.toContain(String(NPC_MAX));
  expect(
    playerBody,
    "and it must be coarsened rather than simply omitted",
  ).toContain('"disclosure":"chunked"');

  // 37 of 250 is inside the first quarter.
  expect(playerBody).toContain('"quarter":0');

  // eslint-disable-next-line no-console -- the run's product is this line.
  console.log(
    `[disclosure] gm=visible(${NPC_CURRENT}/${NPC_MAX}) player=chunked(quarter) leaked=none`,
  );

  await playerPage.context().close();
});
