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
 * Spec 030, User Story 4 — a secret the Game Master chooses to reveal.
 *
 * # What "secret" means here, and what it deliberately does not
 *
 * Per the spec's own decision, secrets are a table concern. The geometry
 * reaches every client; it is the *drawing* that differs. Somebody who opens
 * their devtools and announces a secret door has created a problem at their
 * table, not found a hole in this product.
 *
 * That decision is not laziness, and the alternative is worse. A secret door
 * that did not reach a client would also stop blocking vision and movement
 * there — so the player's line of sight would run straight through a wall the
 * Game Master can see, which is a far louder tell than a hidden sprite.
 *
 * So this asserts what a table can observe: the player is not *shown* a door,
 * the GM is, revealing makes it an ordinary door for both, and the revelation
 * survives a reload — because a secret that un-reveals itself when somebody
 * refreshes is worse than one that was never revealed.
 */

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const res = await graphql<{ data?: T; errors?: { message: string }[] }>(
    page,
    query,
    variables,
  );
  if (res.errors?.length || !res.data) {
    throw new Error(`GraphQL failed: ${JSON.stringify(res.errors ?? res)}`);
  }
  return res.data;
}

/**
 * Which walls the engine on this page is actually drawing.
 *
 * Read from the engine rather than from the payload, because the payload is
 * the wrong place to look: the geometry is deliberately sent to every client,
 * so a check that it was withheld would prove the opposite of what this story
 * claims. What a player must not get is the *sprite*.
 */
async function drawnWalls(page: Page, sceneId: string): Promise<string[]> {
  return page.evaluate(async (scene: string) => {
    const sync = (await import(
      /* @vite-ignore */ "/src/engine/world/sync/walls.ts"
    )) as typeof import("../src/engine/world/sync/walls");
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const store = bevy.getBoundWorldStore();
    if (!store) return [];
    await sync.loadWallsIntoStore(store, scene);
    // A couple of frames for the engine to apply the walls and repaint.
    await new Promise((resolve) => setTimeout(resolve, 600));

    // Through a `/src/` module, not a bare specifier: only those resolve to
    // the wasm instance the application mounted. `/@fs/` gives a second,
    // uninitialised one, which reads as "the engine holds nothing".
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/interactionProbe.ts"
    )) as typeof import("../src/engine/bevy/interactionProbe");
    return probe.drawnWallIds();
  }, sceneId);
}

test("a secret door is not shown to the table until it is revealed", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Secrets ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  const wall = await gql<{ createWall: { wallId: string } }>(
    page,
    `mutation ($input: GraphQLCreateWallInput!) {
      createWall(input: $input) { wallId }
    }`,
    {
      input: {
        sceneId,
        x1: 0,
        y1: 0,
        x2: 200,
        y2: 0,
        blocksVision: true,
        blocksMovement: true,
      },
    },
  );
  const wallId = wall.createWall.wallId;

  await gql(
    page,
    `mutation ($wallId: UUID!) { setDoorDesignation(wallId: $wallId, isDoor: true) }`,
    { wallId },
  );
  await gql(
    page,
    `mutation ($wallId: UUID!) { setDoorSecret(wallId: $wallId, secret: true) }`,
    { wallId },
  );

  // The lever that opens it. A prop somewhere else in the room — the point of
  // a secret door is that something *else* reveals it.
  const lever = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: 400, y: 0, tokenType: "object" } },
  );
  const reveal = await gql<{ createInteractive: { interactiveId: string } }>(
    page,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "prop",
        subjectRef: lever.createToken.tokenId,
        effectId: "door.reveal",
        effectConfig: { target: wallId },
        trigger: "click",
        activation: "anyone",
      },
    },
  );

  // --- before ------------------------------------------------------------

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);

  const playerWalls = await gql<{
    walls: { wallId: string; secret: boolean; blocksVision: boolean }[];
  }>(
    playerPage,
    `query ($sceneId: UUID!) {
      walls(sceneId: $sceneId) { wallId secret blocksVision }
    }`,
    { sceneId },
  );
  const playerWall = playerWalls.walls.find((w) => w.wallId === wallId)!;

  // The geometry *does* reach the player, and it *does* still block. That is
  // the decision, stated as an assertion so a future change that withholds it
  // has to face what it would break: a wall the player can see through.
  expect(
    playerWall,
    "the geometry reaches every client, because a wall that did not arrive would stop blocking",
  ).toBeTruthy();
  expect(playerWall.blocksVision).toBe(true);
  expect(playerWall.secret).toBe(true);

  // The claim this story is actually about: the player's engine draws nothing
  // for it, and the Game Master's does.
  await playerPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(playerPage);
  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);

  expect(
    await drawnWalls(playerPage, sceneId),
    "a player is not shown a secret door",
  ).not.toContain(wallId);
  expect(
    await drawnWalls(page, sceneId),
    "the Game Master is — it is their own note about the scene",
  ).toContain(wallId);

  // --- the reveal --------------------------------------------------------

  const revealed = await gql<{ activateInteractive: { outcome: string } }>(
    playerPage,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: reveal.createInteractive.interactiveId },
  );
  expect(revealed.activateInteractive.outcome).toBe("performed");

  const afterReveal = await gql<{
    walls: { wallId: string; secret: boolean }[];
  }>(
    playerPage,
    `query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId secret } }`,
    { sceneId },
  );
  expect(
    afterReveal.walls.find((w) => w.wallId === wallId)?.secret,
    "revealing makes it an ordinary door",
  ).toBe(false);

  expect(
    await drawnWalls(playerPage, sceneId),
    "and now the table can see it",
  ).toContain(wallId);

  // --- and it stays revealed ---------------------------------------------

  await playerPage.goto(`/world/${worldId}/play`);
  await waitForEngineReady(playerPage);

  const afterReload = await gql<{
    walls: { wallId: string; secret: boolean }[];
  }>(
    playerPage,
    `query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId secret } }`,
    { sceneId },
  );
  expect(
    afterReload.walls.find((w) => w.wallId === wallId)?.secret,
    "a secret that un-reveals itself on refresh is worse than one never revealed",
  ).toBe(false);

  // --- now it works like any other door ----------------------------------

  const doorInteractive = await gql<{
    interactives: { interactiveId: string; subjectRef: string }[];
  }>(
    playerPage,
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) { interactiveId subjectRef }
    }`,
    { sceneId },
  );
  const onTheDoor = doorInteractive.interactives.find(
    (i) => i.subjectRef === wallId,
  )!;
  const opened = await gql<{ activateInteractive: { outcome: string } }>(
    playerPage,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: onTheDoor.interactiveId },
  );
  expect(
    opened.activateInteractive.outcome,
    "a revealed secret door is a door",
  ).toBe("performed");

  // --- revealing is one-way for a player ---------------------------------

  const rehide = await graphql<{ errors?: { message: string }[] }>(
    playerPage,
    `
      mutation ($wallId: UUID!) {
        setDoorSecret(wallId: $wallId, secret: true)
      }
    `,
    { wallId },
  );
  expect(
    rehide.errors?.length,
    "a player must not be able to hide a door from the rest of the table",
  ).toBeGreaterThan(0);

  console.log(
    `[secrets] geometryReachesPlayer=true drawnForPlayer=false drawnForGm=true revealed=true survivedReload=true`,
  );

  await playerPage.context().close();
});
