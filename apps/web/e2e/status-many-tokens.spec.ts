import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, User Story 2 — a Game Master reads the whole board at once.
 *
 * The claim is *concurrency*: several tokens showing their own state
 * simultaneously, with no click and no selection. A test that checked one
 * token would pass against an implementation that could only ever display one,
 * which is the failure mode worth guarding — the panel follows selection, and
 * it would be easy to build bars that did too.
 *
 * Each creature is given a deliberately different pool so the assertion is
 * that each token carries *its own* numbers, not merely that three tokens
 * carry something.
 */

interface Creature {
  label: string;
  current: number;
  max: number;
}

const CREATURES: Creature[] = [
  { label: "Ogre", current: 41, max: 60 },
  { label: "Wisp", current: 3, max: 8 },
  { label: "Warden", current: 96, max: 100 },
];

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  // The e2e helper returns the whole `{ data, errors }` envelope — unlike the
  // app's `postGraphQL`, which unwraps it. Getting these the wrong way round
  // has cost this suite time twice, so the shape is named here once.
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

test("a Game Master sees every token's own resources at once", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Board ${suffix}`);

  const active = await gql<{ world?: { activeSceneId: string | null } }>(
    page,
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world?.activeSceneId ?? firstScene;

  await gql(
    page,
    `mutation ($input: UpdateWorldGameSystemInput!) {
      updateWorldGameSystem(input: $input) { id }
    }`,
    { input: { worldId, gameSystemId: "genie" } },
  );

  const tokenIds: string[] = [];
  for (const creature of CREATURES) {
    const actor = await gql<{ createActor: { id: string } }>(
      page,
      `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      {
        input: {
          worldId,
          label: `${creature.label} ${suffix}`,
          isNpc: true,
          gameSystemId: "genie",
        },
      },
    );

    await gql(
      page,
      `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) { id }
      }`,
      {
        input: {
          actorId: actor.createActor.id,
          gameSystemId: "genie",
          dataType: "resource_data",
          data: {
            current_health: creature.current,
            max_health: creature.max,
            current_wish_points: 0,
            max_wish_points: 0,
          },
        },
      },
    );

    const created = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      {
        input: {
          sceneId,
          x: tokenIds.length * 200,
          y: 0,
          actorId: actor.createActor.id,
          tokenType: "npc",
        },
      },
    );
    tokenIds.push(created.createToken.tokenId);
  }

  await page.goto(`/world/${worldId}/play`);
  await page.evaluate((ids) => {
    (window as unknown as { __boardTokens: string[] }).__boardTokens = ids;
  }, tokenIds);
  await waitForEngineReady(page);

  // Every token, concurrently, with no selection and no interaction.
  await expect
    .poll(
      async () =>
        page.evaluate(async () => {
          const engine = (await import(
            /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
          )) as typeof import("../src/engine/bevy/tokenStatus");
          const ids = (window as unknown as { __boardTokens: string[] })
            .__boardTokens;
          const all = await Promise.all(
            ids.map((id) => engine.readTokenStatus(id)),
          );
          return all.filter((status) => status !== null).length;
        }),
      {
        message: "every token should carry its own status simultaneously",
        timeout: 60_000,
      },
    )
    .toBe(CREATURES.length);

  // And each one carries *its own* figures — the GM sees exact values.
  const readings = await page.evaluate(async () => {
    const engine = (await import(
      /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
    )) as typeof import("../src/engine/bevy/tokenStatus");
    const ids = (window as unknown as { __boardTokens: string[] })
      .__boardTokens;
    const all = await Promise.all(ids.map((id) => engine.readTokenStatus(id)));
    return all.map((status) => {
      const health = status?.find((r) => r.definition.id === "health");
      if (health?.disclosed.disclosure !== "visible") return null;
      const entry = health.disclosed.entries[0];
      return { current: entry.current, max: entry.max };
    });
  });

  expect(readings, "each token reports its own pool, not a shared one").toEqual(
    CREATURES.map((c) => ({ current: c.current, max: c.max })),
  );

  console.log(
    `[board] tokens=${CREATURES.length} concurrent=true readings=${readings
      .map((r) => `${r?.current}/${r?.max}`)
      .join(",")}`,
  );
});
