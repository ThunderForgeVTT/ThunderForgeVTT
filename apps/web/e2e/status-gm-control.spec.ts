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
 * Spec 029, User Story 3a — the Game Master decides what the table knows.
 *
 * Written before the mutation existed, and asserting only what a person at the
 * table could observe: what reaches a player's machine before and after the GM
 * changes their mind. Nothing here names a table, a column, or a code path, so
 * the storage can be rebuilt without touching this file.
 *
 * # The three claims
 *
 * 1. A player cannot set disclosure. It changes what other people know, so it
 *    belongs to whoever runs the world.
 * 2. Two tokens of the *same creature* can disclose differently — the boss on
 *    screen and the identical one waiting in the wings are not the same
 *    situation.
 * 3. A change reaches a connected player without a reload, because a GM
 *    revealing a boss mid-fight cannot ask the table to refresh.
 */

const EXACT_CURRENT = 44;
const EXACT_MAX = 120;

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
 * Every number a status payload actually carries about a resource's value.
 *
 * Not a substring search over the response text: ids are hex UUIDs, so any
 * two-digit figure turns up inside one eventually and fails a payload that
 * leaked nothing. This one did — a token id containing "44" failed an
 * entirely correct chunked response.
 */
function valuesIn(body: string): number[] {
  const parsed = JSON.parse(body) as {
    data?: {
      tokenStatus?: {
        resources?: {
          proportion?: number | null;
          entries?: { current?: number; max?: number | null }[] | null;
        }[];
      }[];
    };
  };
  const found: number[] = [];
  for (const token of parsed.data?.tokenStatus ?? []) {
    for (const resource of token.resources ?? []) {
      if (typeof resource.proportion === "number") {
        found.push(resource.proportion);
      }
      for (const entry of resource.entries ?? []) {
        if (typeof entry.current === "number") found.push(entry.current);
        if (typeof entry.max === "number") found.push(entry.max);
      }
    }
  }
  return found;
}

/** The same query the app makes, so this observes what a client is sent. */
async function statusFor(page: Page, sceneId: string): Promise<string> {
  const res = await graphql<unknown>(
    page,
    `
      query ($sceneId: UUID!) {
        tokenStatus(sceneId: $sceneId) {
          tokenId
          resources {
            definitionId
            disclosure
            quarter
            proportion
            entries {
              current
              max
            }
          }
        }
      }
    `,
    { sceneId },
  );
  return JSON.stringify(res);
}

test("a Game Master changes what the table sees, and a player cannot", async ({
  page,
  browser,
}) => {
  test.setTimeout(5 * 60_000);

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Control ${suffix}`);

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

  const actor = await gql<{ createActor: { id: string } }>(
    page,
    `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
    {
      input: {
        worldId,
        label: `Boss ${suffix}`,
        isNpc: true,
        gameSystemId: "genie",
      },
    },
  );
  const actorId = actor.createActor.id;

  await gql(
    page,
    `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
      updateActorSystemData(input: $input) { id }
    }`,
    {
      input: {
        actorId,
        gameSystemId: "genie",
        dataType: "resource_data",
        data: {
          current_health: EXACT_CURRENT,
          max_health: EXACT_MAX,
          current_wish_points: 0,
          max_wish_points: 0,
        },
      },
    },
  );

  // Two tokens of the *same* creature: the one on screen and its twin.
  const tokenIds: string[] = [];
  for (let i = 0; i < 2; i += 1) {
    const created = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      { input: { sceneId, x: i * 200, y: 0, actorId, tokenType: "npc" } },
    );
    tokenIds.push(created.createToken.tokenId);
  }

  const playerPage = await inviteAndJoinAsPlayer(browser, page, worldId);

  // Both start chunked, from the actor-derived default.
  const before = await statusFor(playerPage, sceneId);
  expect(
    before,
    "an NPC starts coarse without anybody configuring it",
  ).toContain('"disclosure":"chunked"');
  expect(valuesIn(before)).not.toContain(EXACT_CURRENT);

  // --- claim 1: a player may not change it ------------------------------

  const refused = await graphql<{ errors?: { message: string }[] }>(
    playerPage,
    `
      mutation ($input: SetTokenDisclosureInput!) {
        setTokenDisclosure(input: $input) {
          tokenId
        }
      }
    `,
    {
      input: {
        tokenId: tokenIds[0],
        resourceId: "health",
        state: "VISIBLE",
      },
    },
  );
  expect(
    refused.errors?.length,
    "a player must not be able to reveal what the GM is hiding",
  ).toBeGreaterThan(0);

  // --- claim 2: the GM reveals one twin, not both -----------------------

  await gql(
    page,
    `mutation ($input: SetTokenDisclosureInput!) {
      setTokenDisclosure(input: $input) { tokenId }
    }`,
    {
      input: { tokenId: tokenIds[0], resourceId: "health", state: "VISIBLE" },
    },
  );

  const after = await statusFor(playerPage, sceneId);
  expect(
    valuesIn(after),
    "the revealed token now carries its real figure for the player",
  ).toContain(EXACT_CURRENT);
  expect(
    after,
    "and its twin is still coarse — disclosure is per token, not per creature",
  ).toContain('"disclosure":"chunked"');

  // --- claim 3: a connected player sees it without reloading ------------

  await playerPage.goto(`/world/${worldId}/play`);
  await playerPage.evaluate((ids) => {
    (window as unknown as { __twins: string[] }).__twins = ids;
  }, tokenIds);
  await waitForEngineReady(playerPage);

  await gql(
    page,
    `mutation ($input: SetTokenDisclosureInput!) {
      setTokenDisclosure(input: $input) { tokenId }
    }`,
    { input: { tokenId: tokenIds[1], resourceId: "health", state: "VISIBLE" } },
  );

  // Observed through the engine, on the page that was already open — no
  // navigation, no refresh.
  await expect
    .poll(
      async () =>
        playerPage.evaluate(async () => {
          const engine = (await import(
            /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
          )) as typeof import("../src/engine/bevy/tokenStatus");
          const ids = (window as unknown as { __twins: string[] }).__twins;
          const all = await Promise.all(
            ids.map((id) => engine.readTokenStatus(id)),
          );
          return all.filter((status) =>
            status?.some((r) => r.disclosed.disclosure === "visible"),
          ).length;
        }),
      {
        message: "a mid-session reveal must reach a player without a reload",
        timeout: 60_000,
      },
    )
    .toBe(2);

  console.log(`[control] playerRefused=true perToken=true liveUpdate=true`);

  await playerPage.context().close();
});
