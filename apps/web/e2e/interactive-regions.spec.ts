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
 * Spec 030, User Story 5 — something that happens when players arrive.
 *
 * A region fires when a token crosses into it. Four claims, and three of them
 * are about the trigger *not* firing, which is the harder half:
 *
 * 1. Crossing in fires it.
 * 2. Moving around inside does not fire it again (FR-030). A region that fired
 *    on every step reads at the table as the scene stuttering.
 * 3. A `once` region does not fire for the second token to arrive, and a Game
 *    Master can reset it (FR-031).
 * 4. Preparation movement fires nothing (FR-032). A GM arranging a scene and a
 *    GM running one make the same gesture, so the engine has to be told which
 *    it is — and the default is preparation, because a trigger that went off
 *    while nobody was looking has already spent itself.
 *
 * # Why the engine drives this and the server answers
 *
 * The engine is the only thing that knows both where a token was and where it
 * is now. It *reports* the crossing; whether it is permitted, whether a `once`
 * has spent itself, whether approval is needed — all of that is the server's,
 * because a click raises exactly the same questions and answering them twice
 * in two places is how the two answers drift.
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

/** Whether a `once` region has spent itself, as the Game Master sees it. */
async function hasFired(
  page: Page,
  sceneId: string,
  interactiveId: string,
): Promise<boolean> {
  const data = await gql<{
    interactives: { interactiveId: string; firedAt: string | null }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      interactives(sceneId: $sceneId) { interactiveId firedAt }
    }`,
    { sceneId },
  );
  const found = data.interactives.find(
    (i) => i.interactiveId === interactiveId,
  );
  return found?.firedAt != null;
}

test("a region fires once per crossing, and never while preparing", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Regions ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  // The thing the region does: turn a lamp off. Chosen because it is
  // observable in the database, so "did it fire" needs no instrumentation.
  const lamp = await gql<{ createLightSource: { lightId: string } }>(
    page,
    `mutation ($input: GraphQLCreateLightSourceInput!) {
      createLightSource(input: $input) { lightId }
    }`,
    {
      input: {
        sceneId,
        x: 0,
        y: 0,
        radius: 120,
        intensity: 0.8,
        castsShadows: false,
      },
    },
  );
  const lightId = lamp.createLightSource.lightId;

  const threshold = await gql<{
    createInteractive: { interactiveId: string };
  }>(
    page,
    `mutation ($input: GraphQLCreateInteractiveInput!) {
      createInteractive(input: $input) { interactiveId }
    }`,
    {
      input: {
        sceneId,
        subjectKind: "region",
        geometry: { shape: "rect", x: 200, y: -100, width: 200, height: 200 },
        effectId: "light.toggle",
        effectConfig: { lights: [lightId], mode: "off" },
        trigger: "enter",
        activation: "anyone",
        fireMode: "once",
      },
    },
  );
  const regionId = threshold.createInteractive.interactiveId;

  // --- a player is never shown a region ----------------------------------

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);
  const playerSees = await gql<{ interactives: { interactiveId: string }[] }>(
    playerPage,
    `query ($sceneId: UUID!) { interactives(sceneId: $sceneId) { interactiveId } }`,
    { sceneId },
  );
  expect(
    playerSees.interactives.some((i) => i.interactiveId === regionId),
    "a region is not an annotation — a player is not told one exists",
  ).toBe(false);

  // --- a region cannot be authored on something that cannot be crossed ----

  const badTrigger = await graphql<{ errors?: { message: string }[] }>(
    page,
    `
      mutation ($input: GraphQLCreateInteractiveInput!) {
        createInteractive(input: $input) {
          interactiveId
        }
      }
    `,
    {
      input: {
        sceneId,
        subjectKind: "region",
        geometry: { shape: "rect", x: 0, y: 0, width: 0, height: 50 },
        trigger: "enter",
        activation: "anyone",
      },
    },
  );
  expect(
    badTrigger.errors?.length,
    "a region with no width encloses nothing, so nothing could cross into it",
  ).toBeGreaterThan(0);

  // --- preparation moves nothing -----------------------------------------

  await page.goto(`/world/${worldId}/play`);
  await page.evaluate(
    (ids) => {
      (window as unknown as { __scene: string }).__scene = ids.scene;
    },
    { scene: sceneId },
  );
  await waitForEngineReady(page);

  const token = await gql<{ createToken: { tokenId: string } }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
      createToken(input: $input) { tokenId }
    }`,
    { input: { sceneId, x: -400, y: 0, tokenType: "character" } },
  );
  const tokenId = token.createToken.tokenId;

  /**
   * Walk a token from one point to another through the engine, and report
   * which interactives it triggered.
   *
   * Driven through the world store rather than by calling the detection
   * directly, so what is exercised is the path the application actually uses.
   */
  const walk = async (
    target: Page,
    from: [number, number],
    to: [number, number],
    playing: boolean,
  ): Promise<string[]> =>
    target.evaluate(
      async (args: {
        scene: string;
        token: string;
        from: [number, number];
        to: [number, number];
        playing: boolean;
      }) => {
        const bevy = (await import(
          /* @vite-ignore */ "/src/engine/bevy/index.ts"
        )) as typeof import("../src/engine/bevy/index");
        const sync = (await import(
          /* @vite-ignore */ "/src/engine/world/sync/interactives.ts"
        )) as typeof import("../src/engine/world/sync/interactives");
        const store = bevy.getBoundWorldStore();
        if (!store) return [];

        await sync.refreshInteractives(store, args.scene);
        sync.setScenePlaying(store, args.playing);

        const fired: string[] = [];
        const watching = bevy.onInteractionTriggered((event) => {
          fired.push(event.interactiveId);
        });
        // The application's own bridge: a detected crossing becomes the same
        // server activation a click makes. Started here so what the test
        // exercises is the real path rather than a stand-in for it.
        const bridge = sync.startTriggerBridge(store);

        const place = (x: number, y: number) =>
          store.dispatch(
            {
              type: "upsert_token",
              token: { id: args.token, x, y, z: 0 },
            },
            "sync",
          );

        place(args.from[0], args.from[1]);
        await new Promise((resolve) => setTimeout(resolve, 300));
        place(args.to[0], args.to[1]);
        await new Promise((resolve) => setTimeout(resolve, 500));

        // A moment for the round trip the bridge started.
        await new Promise((resolve) => setTimeout(resolve, 800));
        watching();
        bridge();
        return fired;
      },
      { scene: sceneId, token: tokenId, from, to, playing },
    );

  // Dragged across the threshold while the scene is being prepared. The GM's
  // gesture is identical to playing; the mode is what differs.
  const duringPreparation = await walk(page, [-400, 0], [300, 0], false);
  expect(
    duringPreparation,
    "arranging a scene must not set off what is in it (FR-032)",
  ).not.toContain(regionId);
  expect(await hasFired(page, sceneId, regionId)).toBe(false);

  // --- crossing in, in play ----------------------------------------------

  const crossing = await walk(page, [-400, 0], [300, 0], true);
  expect(crossing, "crossing in fires it").toContain(regionId);

  await expect
    .poll(async () => hasFired(page, sceneId, regionId), {
      message: "a once region records that it spent itself",
      timeout: 30_000,
    })
    .toBe(true);

  const lights = await gql<{
    lightSources: { lightId: string; intensity: number }[];
  }>(
    page,
    `query ($sceneId: UUID!) { lightSources(sceneId: $sceneId) { lightId intensity } }`,
    { sceneId },
  );
  expect(
    lights.lightSources.find((l) => l.lightId === lightId)?.intensity,
    "and the effect actually ran",
  ).toBe(0);

  // --- moving around inside does not fire it again ------------------------

  const within = await walk(page, [250, -50], [350, 50], true);
  expect(
    within,
    "a token already inside has entered nothing (FR-030)",
  ).not.toContain(regionId);

  // --- and a second arrival is refused until the GM resets ----------------

  const secondArrival = await gql<{
    activateInteractive: { outcome: string; reason: string | null };
  }>(
    page,
    `mutation ($id: UUID!) {
      activateInteractive(interactiveId: $id) { outcome reason }
    }`,
    { id: regionId },
  );
  expect(secondArrival.activateInteractive.outcome).toBe("refused");
  expect(secondArrival.activateInteractive.reason).toBe("alreadyFired");

  await gql(
    page,
    `mutation ($id: UUID!) { resetInteractive(interactiveId: $id) { interactiveId } }`,
    { id: regionId },
  );
  expect(
    await hasFired(page, sceneId, regionId),
    "the GM can let it happen again (FR-031)",
  ).toBe(false);

  const afterReset = await gql<{ activateInteractive: { outcome: string } }>(
    page,
    `mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome } }`,
    { id: regionId },
  );
  expect(afterReset.activateInteractive.outcome).toBe("performed");

  // A player cannot reset it — that is a Game Master's decision about the
  // scene, not a player's about their own turn.
  const playerReset = await graphql<{ errors?: { message: string }[] }>(
    playerPage,
    `
      mutation ($id: UUID!) {
        resetInteractive(interactiveId: $id) {
          interactiveId
        }
      }
    `,
    { id: regionId },
  );
  expect(playerReset.errors?.length).toBeGreaterThan(0);

  console.log(
    `[regions] preparationFired=false crossingFired=true withinRefired=false onceRespected=true resetWorks=true`,
  );

  await playerPage.context().close();
});
