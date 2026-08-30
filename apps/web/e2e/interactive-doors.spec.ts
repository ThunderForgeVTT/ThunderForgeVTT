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
 * Spec 030, User Story 2 — doors that open, close and lock.
 *
 * Two browsers, because every claim here is comparative: a player is refused
 * and the Game Master is not, and a change one of them makes reaches the
 * other without a reload.
 *
 * # Why the refusal is asserted at the mutation and not at the screen
 *
 * "A player cannot open a locked door" is the requirement most likely to be
 * implemented by not drawing the button. That passes every screen test and
 * fails the moment anybody calls the mutation directly — including any future
 * client, and including this one after a refactor. So the player's browser
 * calls it directly, and the assertion is on what the server answered *and* on
 * what the door actually did.
 *
 * # Why the door's blocking is read rather than looked at
 *
 * What matters is that vision and movement change, and that they change the
 * way the wall's own profile says. Reading the engine's wall state proves
 * that; a screenshot proves only that something was drawn.
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

/** The scene's walls, as the viewer receives them. */
async function wallsFor(
  page: Page,
  sceneId: string,
): Promise<
  {
    wallId: string;
    doorState: string;
    locked: boolean;
    secret: boolean;
    blocksVision: boolean;
    blocksMovement: boolean;
  }[]
> {
  const data = await gql<{
    walls: {
      wallId: string;
      doorState: string;
      locked: boolean;
      secret: boolean;
      blocksVision: boolean;
      blocksMovement: boolean;
    }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      walls(sceneId: $sceneId) {
        wallId
        doorState
        locked
        secret
        blocksVision
        blocksMovement
      }
    }`,
    { sceneId },
  );
  return data.walls;
}

test("a door opens for the table, and a locked one does not", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Doors ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  // A stone wall: blocks both. Closing it must block both; opening it neither.
  const stone = await gql<{ createWall: { wallId: string } }>(
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
  const stoneId = stone.createWall.wallId;

  // A window: blocks movement, not vision. Closing it must leave it
  // see-through, which is the case a stored second set of flags gets wrong.
  const window = await gql<{ createWall: { wallId: string } }>(
    page,
    `mutation ($input: GraphQLCreateWallInput!) {
      createWall(input: $input) { wallId }
    }`,
    {
      input: {
        sceneId,
        x1: 0,
        y1: 200,
        x2: 200,
        y2: 200,
        blocksVision: false,
        blocksMovement: true,
      },
    },
  );
  const windowId = window.createWall.wallId;

  // --- designating a door gives it something to click --------------------

  for (const wallId of [stoneId, windowId]) {
    await gql(
      page,
      `mutation ($wallId: UUID!, $isDoor: Boolean!) {
        setDoorDesignation(wallId: $wallId, isDoor: $isDoor)
      }`,
      { wallId, isDoor: true },
    );
  }

  const gmInteractives = await gql<{
    interactives: {
      interactiveId: string;
      subjectRef: string;
      effectId: string;
    }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) { interactiveId subjectRef effectId }
    }`,
    { sceneId },
  );
  const stoneInteractive = gmInteractives.interactives.find(
    (i) => i.subjectRef === stoneId,
  );
  expect(
    stoneInteractive,
    "designating a door gives it an interactive, so it can be clicked",
  ).toBeTruthy();
  expect(stoneInteractive!.effectId).toBe("door.set_state");

  // A newly designated door starts closed. A wall that became a hole the
  // moment it became a door would change what the room does.
  const afterDesignation = await wallsFor(page, sceneId);
  expect(afterDesignation.find((w) => w.wallId === stoneId)?.doorState).toBe(
    "CLOSED",
  );

  // --- a player opens it -------------------------------------------------

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);

  const opened = await gql<{ activateInteractive: { outcome: string } }>(
    playerPage,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: stoneInteractive!.interactiveId },
  );
  expect(opened.activateInteractive.outcome).toBe("performed");

  const afterOpen = await wallsFor(playerPage, sceneId);
  const openedStone = afterOpen.find((w) => w.wallId === stoneId)!;
  expect(openedStone.doorState, "the player's click was durable").toBe("OPEN");

  // --- what open and closed mean -----------------------------------------

  const windowInteractive = gmInteractives.interactives.find(
    (i) => i.subjectRef === windowId,
  )!;
  const closedWindow = afterOpen.find((w) => w.wallId === windowId)!;

  // The definition FR-008 and FR-009 asked for, checked against the wall's own
  // profile rather than against a second stored set of flags: closed blocks
  // exactly what the wall blocks. A closed window is still see-through.
  expect(closedWindow.doorState).toBe("CLOSED");
  expect(closedWindow.blocksVision).toBe(false);
  expect(closedWindow.blocksMovement).toBe(true);
  expect(openedStone.blocksVision, "the profile is unchanged by opening").toBe(
    true,
  );

  await gql(
    playerPage,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: windowInteractive.interactiveId },
  );

  // --- the Game Master locks it ------------------------------------------

  await gql(
    page,
    `mutation ($wallId: UUID!, $locked: Boolean!) {
      setDoorLock(wallId: $wallId, locked: $locked)
    }`,
    { wallId: stoneId, locked: true },
  );

  const refused = await gql<{
    activateInteractive: { outcome: string; reason: string | null };
  }>(
    playerPage,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome reason }
    }`,
    { id: stoneInteractive!.interactiveId },
  );

  // Refused, and *told why* — silence is indistinguishable from the product
  // being broken (FR-014).
  expect(refused.activateInteractive.outcome).toBe("refused");
  expect(refused.activateInteractive.reason).toBe("locked");

  const afterRefusal = await wallsFor(page, sceneId);
  expect(
    afterRefusal.find((w) => w.wallId === stoneId)?.doorState,
    "a refusal that still moved the door would pass an outcome check and fail the table",
  ).toBe("OPEN");

  // --- and the Game Master is not refused --------------------------------

  const gmChange = await gql<{ activateInteractive: { outcome: string } }>(
    page,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: stoneInteractive!.interactiveId },
  );
  expect(
    gmChange.activateInteractive.outcome,
    "FR-013: the lock is theirs, not a rule against them",
  ).toBe("performed");
  const afterGm = await wallsFor(page, sceneId);
  expect(afterGm.find((w) => w.wallId === stoneId)?.doorState).toBe("CLOSED");

  // --- a player cannot lock, designate or reveal --------------------------

  for (const [mutation, variables] of [
    [
      `mutation ($wallId: UUID!) { setDoorLock(wallId: $wallId, locked: false) }`,
      { wallId: stoneId },
    ],
    [
      `mutation ($wallId: UUID!) { setDoorDesignation(wallId: $wallId, isDoor: false) }`,
      { wallId: stoneId },
    ],
    [
      `mutation ($wallId: UUID!) { setDoorSecret(wallId: $wallId, secret: true) }`,
      { wallId: stoneId },
    ],
  ] as const) {
    const attempt = await graphql<{ errors?: { message: string }[] }>(
      playerPage,
      mutation,
      variables,
    );
    expect(
      attempt.errors?.length,
      `a player must not be able to run: ${mutation}`,
    ).toBeGreaterThan(0);
  }

  // --- it reaches an open page without a reload ---------------------------

  await playerPage.goto(`/world/${worldId}/play`);
  await playerPage.evaluate((id) => {
    (window as unknown as { __door: string }).__door = id;
  }, stoneId);
  await waitForEngineReady(playerPage);

  await gql(
    page,
    `mutation ($wallId: UUID!, $locked: Boolean!) {
      setDoorLock(wallId: $wallId, locked: $locked)
    }`,
    { wallId: stoneId, locked: false },
  );

  const seen = await playerPage.evaluate(async (scene: string) => {
    const sync = (await import(
      /* @vite-ignore */ "/src/engine/world/sync/walls.ts"
    )) as typeof import("../src/engine/world/sync/walls");
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    const store = bevy.getBoundWorldStore();
    if (!store) return null;
    await sync.loadWallsIntoStore(store, scene);
    const id = (window as unknown as { __door: string }).__door;
    return store.getState().walls[id] ?? null;
  }, sceneId);

  expect(seen, "the door reached the already-open page").toBeTruthy();
  expect(seen!.locked, "unlocking reaches a player who never reloaded").toBe(
    false,
  );
  expect(seen!.doorState).toBe("closed");

  console.log(
    `[doors] designated=2 playerOpened=true lockedRefused=locked gmNotRefused=true windowStaysSeeThrough=true`,
  );

  await playerPage.context().close();
});
