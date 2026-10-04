import { expect, test, type Page } from "./fixtures/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  launchSceneByName,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";

/**
 * Scene levels: one scene, three floors, and a staircase between two of them.
 *
 * A level is a privacy boundary as much as a floor. What this proves, with a
 * Game Master and two players each in their own browser:
 *
 * - a player who walks onto the stairs is *taken* upstairs: their board
 *   reloads as the level above, and they did not choose it;
 * - the player left in the tavern stops seeing them, on the board and when
 *   they ask the server directly for the floor above;
 * - the Game Master finds them on the Upstairs tab;
 * - reloading does not put the traveller back downstairs;
 * - a Game Master dragging a token across the stairs arranges the board and
 *   sends nobody anywhere.
 *
 * The walk itself is `moveOwnToken`, the mutation a player's drag ends in:
 * the server is what travels a token, so the proof is that every board
 * follows the server, not that a mouse can find a staircase.
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

/** The ids of the tokens on a page's board, as its world store holds them. */
async function boardTokens(page: Page): Promise<string[]> {
  return page.evaluate(async () => {
    const bevy = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../src/engine/bevy/index");
    return Object.keys(bevy.getBoundWorldStore()?.getState().tokens ?? {});
  });
}

async function tokensOn(
  page: Page,
  sceneId: string,
  levelId: string,
): Promise<{ tokenId: string; x: number; y: number }[]> {
  const data = await gql<{
    tokens: { tokenId: string; x: number; y: number }[];
  }>(
    page,
    `query ($sceneId: UUID!, $levelId: UUID) {
      tokens(sceneId: $sceneId, levelId: $levelId) { tokenId x y }
    }`,
    { sceneId, levelId },
  );
  return data.tokens;
}

async function userIdOf(page: Page): Promise<string> {
  const { me } = await gql<{ me: { id: string } }>(
    page,
    `query { me { id } }`,
    {},
  );
  return me.id;
}

test("a player who walks onto the stairs is taken upstairs, and only they are", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const sceneName = `Inn ${suffix}`;
  const worldId = await registerAndCreateWorld(page, `Levels ${suffix}`);

  // --- One scene, three levels ------------------------------------------
  const { createScene } = await gql<{ createScene: { sceneId: string } }>(
    page,
    `mutation ($input: GraphQLCreateSceneInput!) {
      createScene(input: $input) { sceneId }
    }`,
    { input: { worldId, name: sceneName } },
  );
  const sceneId = createScene.sceneId;
  await gql(
    page,
    `mutation ($sceneId: UUID!) {
      updateSceneHidden(sceneId: $sceneId, hidden: false) { sceneId }
    }`,
    { sceneId },
  );

  // A scene is born with one level, the one it opens on. It becomes the
  // tavern; the other two are added above it.
  const born = await gql<{
    sceneLevels: { levelId: string; isEntry: boolean }[];
  }>(
    page,
    `query ($sceneId: UUID!) {
      sceneLevels(sceneId: $sceneId) { levelId isEntry }
    }`,
    { sceneId },
  );
  expect(born.sceneLevels, "a new scene has exactly one level").toHaveLength(1);
  const tavern = born.sceneLevels[0].levelId;
  expect(born.sceneLevels[0].isEntry).toBe(true);
  await gql(
    page,
    `mutation ($levelId: UUID!, $input: GraphQLUpdateSceneLevelInput!) {
      updateSceneLevel(levelId: $levelId, input: $input) { levelId }
    }`,
    { levelId: tavern, input: { name: "Tavern" } },
  );
  const addLevel = async (name: string): Promise<string> => {
    const added = await gql<{ createSceneLevel: { levelId: string } }>(
      page,
      `mutation ($input: GraphQLCreateSceneLevelInput!) {
        createSceneLevel(input: $input) { levelId }
      }`,
      { input: { sceneId, name } },
    );
    return added.createSceneLevel.levelId;
  };
  const upstairs = await addLevel("Upstairs");
  await addLevel("Street");

  // --- The stairs: a pair of regions, each naming the other --------------
  // The top is made first, as scenery, because the bottom has to name it.
  const region = async (
    levelId: string,
    x: number,
    y: number,
    partner?: string,
  ): Promise<string> => {
    const made = await gql<{ createInteractive: { interactiveId: string } }>(
      page,
      `mutation ($input: GraphQLCreateInteractiveInput!) {
        createInteractive(input: $input) { interactiveId }
      }`,
      {
        input: {
          sceneId,
          levelId,
          subjectKind: "region",
          geometry: { shape: "rect", x, y, width: 200, height: 200 },
          ...(partner
            ? { effectId: "nav.travel", effectConfig: { partner } }
            : {}),
          trigger: "enter",
          activation: "anyone",
          fireMode: "always",
        },
      },
    );
    return made.createInteractive.interactiveId;
  };
  const stairsTop = await region(upstairs, -400, -300);
  const stairsBottom = await region(tavern, 200, -100, stairsTop);
  await gql(
    page,
    `mutation ($id: UUID!, $input: GraphQLUpdateInteractiveInput!) {
      updateInteractive(interactiveId: $id, input: $input) { interactiveId }
    }`,
    {
      id: stairsTop,
      input: {
        effectId: "nav.travel",
        effectConfig: { partner: stairsBottom },
      },
    },
  );

  // --- Two players, each with a token in the tavern ----------------------
  const playerA = await inviteAndJoinAsPlayer(
    browser,
    page,
    worldId,
    "e2elevela",
  );
  const playerB = await inviteAndJoinAsPlayer(
    browser,
    page,
    worldId,
    "e2elevelb",
  );
  const seat = async (player: Page, y: number): Promise<string> => {
    const made = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      {
        input: {
          sceneId,
          levelId: tavern,
          x: -200,
          y,
          tokenType: "character",
        },
      },
    );
    await gql(
      page,
      `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: $tokenId, input: $input) { tokenId }
      }`,
      {
        tokenId: made.createToken.tokenId,
        input: { ownerUserId: await userIdOf(player), isPrimary: true },
      },
    );
    return made.createToken.tokenId;
  };
  const tokenA = await seat(playerA, 0);
  const tokenB = await seat(playerB, 200);

  // --- Everyone at the table ---------------------------------------------
  await launchSceneByName(page, worldId, sceneName);
  await waitForEngineReady(page);
  for (const player of [playerA, playerB]) {
    await player.goto(`/world/${worldId}/play`);
    await waitForEngineReady(player);
  }

  // The Game Master has a tab for every level and opens on the tavern.
  await expect(page.getByTestId("level-tab")).toHaveText([
    /^Tavern/,
    /^Upstairs/,
    /^Street/,
  ]);
  await expect(
    page.getByRole("tab", { selected: true }),
    "a Game Master opens on the entry level",
  ).toHaveAttribute("data-level-id", tavern);

  // Both players are in the tavern, see each other, and — standing on the
  // level the scene opens on — are shown no level chrome at all.
  for (const player of [playerA, playerB]) {
    await expect
      .poll(async () => (await boardTokens(player)).sort(), {
        message: "both tokens are on a tavern board",
        timeout: 60_000,
      })
      .toEqual([tokenA, tokenB].sort());
    await expect(player.getByTestId("level-name")).toHaveCount(0);
    await expect(player.getByTestId("level-tabs")).toHaveCount(0);
  }

  // --- Player A walks onto the stairs ------------------------------------
  const walked = await gql<{
    moveOwnToken: { tokenId: string; levelId: string };
  }>(
    playerA,
    `mutation ($tokenId: UUID!, $x: Float!, $y: Float!) {
      moveOwnToken(tokenId: $tokenId, x: $x, y: $y) { tokenId levelId }
    }`,
    { tokenId: tokenA, x: 300, y: 0 },
  );
  expect(
    walked.moveOwnToken.levelId,
    "the move comes back with the token already upstairs",
  ).toBe(upstairs);

  // A's board becomes Upstairs: named, and holding A alone.
  await expect(
    playerA.getByTestId("level-name"),
    "the traveller is told which floor they are now on",
  ).toHaveText("Upstairs", { timeout: 60_000 });
  await expect(playerA.getByTestId("level-name")).toHaveAttribute(
    "data-level-id",
    upstairs,
  );
  await expect
    .poll(async () => boardTokens(playerA), {
      message: "the traveller's board is the level above, with them on it",
      timeout: 60_000,
    })
    .toEqual([tokenA]);

  // B, left in the tavern, no longer sees A — on the board…
  await expect
    .poll(async () => boardTokens(playerB), {
      message: "the player left downstairs stops seeing the traveller",
      timeout: 60_000,
    })
    .toEqual([tokenB]);
  await expect(playerB.getByTestId("level-name")).toHaveCount(0);
  // …or by asking the server for the floor above, which answers nothing
  // rather than refusing: a refusal would say the floor is there.
  expect(
    await tokensOn(playerB, sceneId, upstairs),
    "a player is answered nothing for a level they do not stand on",
  ).toEqual([]);
  const levelsForB = await gql<{ sceneLevels: { levelId: string }[] }>(
    playerB,
    `query ($sceneId: UUID!) { sceneLevels(sceneId: $sceneId) { levelId } }`,
    { sceneId },
  );
  expect(levelsForB.sceneLevels.map((level) => level.levelId)).toEqual([
    tavern,
  ]);

  // The Game Master is still looking at the tavern, where A no longer is,
  // and finds them on the Upstairs tab.
  await expect
    .poll(async () => boardTokens(page), {
      message: "the Game Master's tavern no longer holds the traveller",
      timeout: 60_000,
    })
    .toEqual([tokenB]);
  const upstairsTab = page.locator(
    `[data-testid="level-tab"][data-level-id="${upstairs}"]`,
  );
  await expect(
    upstairsTab.getByTestId("level-tab-token-count"),
    "the tab counts who is up there",
  ).toHaveText("1", { timeout: 30_000 });
  await upstairsTab.click();
  await expect(upstairsTab).toHaveAttribute("aria-selected", "true");
  await expect
    .poll(async () => boardTokens(page), {
      message: "the Game Master sees the traveller on the Upstairs tab",
      timeout: 60_000,
    })
    .toEqual([tokenA]);

  // --- A reloads and is still upstairs -----------------------------------
  await playerA.reload();
  await waitForEngineReady(playerA);
  await expect(playerA.getByTestId("level-name")).toHaveText("Upstairs", {
    timeout: 60_000,
  });
  await expect
    .poll(async () => boardTokens(playerA), {
      message: "a reload does not put the traveller back downstairs",
      timeout: 60_000,
    })
    .toEqual([tokenA]);

  // --- A Game Master's drag across the stairs does not travel -------------
  // `updateToken` is what a Game Master's drag sends. B's token is put down
  // in the middle of the stairs and stays in the tavern.
  await gql(
    page,
    `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
      updateToken(tokenId: $tokenId, input: $input) { tokenId }
    }`,
    { tokenId: tokenB, input: { x: 300, y: 0 } },
  );
  expect(
    (await tokensOn(page, sceneId, tavern)).find(
      (token) => token.tokenId === tokenB,
    ),
    "a Game Master arranging the board sends nobody upstairs",
  ).toMatchObject({ x: 300, y: 0 });
  expect(
    (await tokensOn(page, sceneId, upstairs)).map((token) => token.tokenId),
  ).toEqual([tokenA]);
  // And B's own board agrees: still the tavern, still only them.
  await expect
    .poll(async () => boardTokens(playerB), { timeout: 30_000 })
    .toEqual([tokenB]);
  await expect(playerB.getByTestId("level-name")).toHaveCount(0);

  await playerA.context().close();
  await playerB.context().close();
});
