import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, User Story 4 — the engine renders whatever a system declares, and
 * knows nothing about what any of it means.
 *
 * One test body, four rulesets. If the engine had a built-in notion of
 * "health" it would pass for Genie and D&D and fail here, which is the point:
 * FR-001 forbids that notion, and a single-system test cannot tell whether it
 * is being honoured.
 *
 * # Why these four
 *
 * They differ in the ways that matter, not just in name:
 *
 * - **Genie** — two plain bars, both stored with their own maxima.
 * - **D&D 5e** — one bar that *stacks*: temporary hit points are a second
 *   layer, not an overflowing first one. That system stores temp HP by
 *   letting current exceed max, and this is the shape that retires it.
 * - **Pathfinder 2e** — a bar plus two **counters**. Focus and hero points
 *   are discrete things you spend, not fractions of a pool, and drawing them
 *   as partly-filled bars would misrepresent the rules.
 * - **Blades in the Dark** — bars whose maxima are **rules, not data**.
 *   Stress caps at nine and trauma at four; no character stores either,
 *   because neither varies. Without a rules-fixed maximum these could only
 *   be bare counts, losing the one thing a player needs: how close to the cap
 *   they are.
 *
 * The last of those was found by writing this test. The model had no way to
 * express a cap that lives in the rulebook rather than the character sheet.
 */

interface SystemCase {
  systemId: string;
  /** Stored `resource_data`, in that system's own field names. */
  stored: Record<string, number>;
  /** Resource ids expected, in declared order. */
  expect: string[];
  /** What the first resource should read as, once resolved. */
  firstReads: { current: number; max: number | null };
}

const SYSTEMS: SystemCase[] = [
  {
    systemId: "genie",
    stored: {
      current_health: 9,
      max_health: 15,
      current_wish_points: 2,
      max_wish_points: 4,
    },
    expect: ["health", "wishPoints"],
    firstReads: { current: 9, max: 15 },
  },
  {
    systemId: "dnd5e",
    // 20 of 20, plus 5 temporary — expressible only because temp HP is a
    // second entry rather than an overflow of the first.
    stored: { current_hp: 20, max_hp: 20, temporary_hp: 5 },
    expect: ["hitPoints"],
    firstReads: { current: 20, max: 20 },
  },
  {
    systemId: "pathfinder2e",
    stored: { current_hp: 33, max_hp: 48, focus_points: 2, hero_points: 1 },
    expect: ["hitPoints", "focusPoints", "heroPoints"],
    firstReads: { current: 33, max: 48 },
  },
  {
    systemId: "blades_in_the_dark",
    // No maxima stored anywhere. Nine and four come from the rulebook.
    stored: { stress: 6, trauma: 1, coin: 3 },
    expect: ["stress", "trauma", "coin"],
    firstReads: { current: 6, max: 9 },
  },
];

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

for (const system of SYSTEMS) {
  test(`${system.systemId} declares its own resources and the engine draws them`, async ({
    page,
  }) => {
    test.setTimeout(4 * 60_000);

    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      page,
      `${system.systemId} ${suffix}`,
    );

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
      { input: { worldId, gameSystemId: system.systemId } },
    );

    const actor = await gql<{ createActor: { id: string } }>(
      page,
      `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
      {
        input: {
          worldId,
          label: `Hero ${suffix}`,
          isNpc: false,
          gameSystemId: system.systemId,
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
          gameSystemId: system.systemId,
          dataType: "resource_data",
          data: system.stored,
        },
      },
    );

    const me = await gql<{ me: { id: string } }>(
      page,
      `query { me { id } }`,
      {},
    );
    const created = await gql<{ createToken: { tokenId: string } }>(
      page,
      `mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) { tokenId }
      }`,
      {
        input: {
          sceneId,
          x: 0,
          y: 0,
          actorId: actor.createActor.id,
          tokenType: "character",
        },
      },
    );
    const tokenId = created.createToken.tokenId;

    await gql(
      page,
      `mutation ($input: GraphQLUpdateTokenInput!) {
        updateToken(tokenId: "${tokenId}", input: $input) { tokenId }
      }`,
      { input: { ownerUserId: me.me.id } },
    );

    await page.goto(`/world/${worldId}/play`);
    await page.evaluate((id) => {
      (window as unknown as { __sysToken: string }).__sysToken = id;
    }, tokenId);
    await waitForEngineReady(page);

    const reading = await expect
      .poll(
        async () =>
          page.evaluate(async () => {
            const engine = (await import(
              /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
            )) as typeof import("../src/engine/bevy/tokenStatus");
            const status = await engine.readTokenStatus(
              (window as unknown as { __sysToken: string }).__sysToken,
            );
            if (!status) return null;
            const first = status[0];
            const entry =
              first.disclosed.disclosure === "visible"
                ? first.disclosed.entries[0]
                : null;
            return {
              ids: status.map((r) => r.definition.id),
              entryCount:
                first.disclosed.disclosure === "visible"
                  ? first.disclosed.entries.length
                  : 0,
              current: entry?.current ?? null,
              max: entry?.max ?? null,
            };
          }),
        {
          message: `${system.systemId} should reach the engine`,
          timeout: 60_000,
        },
      )
      .not.toBeNull()
      .then(() =>
        page.evaluate(async () => {
          const engine = (await import(
            /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
          )) as typeof import("../src/engine/bevy/tokenStatus");
          const status = await engine.readTokenStatus(
            (window as unknown as { __sysToken: string }).__sysToken,
          );
          const first = status![0];
          const entry =
            first.disclosed.disclosure === "visible"
              ? first.disclosed.entries[0]
              : null;
          return {
            ids: status!.map((r) => r.definition.id),
            entryCount:
              first.disclosed.disclosure === "visible"
                ? first.disclosed.entries.length
                : 0,
            current: entry?.current ?? null,
            max: entry?.max ?? null,
          };
        }),
      );

    expect(reading.ids, "the system's own resources, in its own order").toEqual(
      system.expect,
    );
    expect(reading.current).toBe(system.firstReads.current);
    expect(reading.max).toBe(system.firstReads.max);

    // D&D's temporary hit points are the second entry, not an overflow.
    if (system.systemId === "dnd5e") {
      expect(
        reading.entryCount,
        "temporary hit points are a layer above the pool",
      ).toBe(2);
    }

    console.log(
      `[systems] ${system.systemId} resources=${reading.ids.join(",")} first=${reading.current}/${reading.max}`,
    );
  });
}
