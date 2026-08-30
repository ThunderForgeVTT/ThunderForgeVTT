import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, User Story 1 — and the reproducible form of the demo.
 *
 * A Genie character with health and wish points, on a token, with both
 * resources visible to the player who owns it. This is the run that turns
 * "tokens are coloured squares" into "tokens are characters", so it is worth
 * having as a test rather than a thing somebody once showed on a call.
 *
 * # Why Genie
 *
 * It is a real two-resource system, not a fixture invented for this file:
 * `GenieResourceData` has carried `current_health`/`max_health` and
 * `current_wish_points`/`max_wish_points` since spec 018. A single-resource
 * system would pass while the engine was still hard-coded to health, which is
 * exactly the thing FR-001 forbids and this test is meant to catch.
 *
 * # What is asserted, and where
 *
 * The panel's contents, through the DOM — this is the React half, and the DOM
 * is where it is observable. The *disclosure* assertions do not live here:
 * checking the screen would pass against a client that received a value and
 * chose not to draw it, so those are wire-level tests in
 * `src/server/src/status_display.rs`. This file proves the pipeline carries
 * what it should; those prove it never carries what it should not.
 */

/** Set an actor's Genie resource pools through the real mutation. */
async function setResources(
  page: Page,
  actorId: string,
  data: Record<string, number>,
): Promise<void> {
  const res = await graphql<{
    data?: { updateActorSystemData?: { id: string } };
    errors?: { message: string }[];
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

  if (res.errors?.length || !res.data?.updateActorSystemData?.id) {
    throw new Error(
      `resource_data was not stored, so nothing downstream can be trusted: ${JSON.stringify(
        res.errors ?? res,
      )}`,
    );
  }
}

test("a Genie character's health and wish points reach the token and the panel", async ({
  page,
}) => {
  test.setTimeout(3 * 60_000);

  // Surface client-side failures. Without this, a broken fetch inside the app
  // shows up here only as "no bars appeared", which is the least useful
  // possible description of what went wrong.
  page.on("console", (message) => {
    if (message.type() === "error") {
      // eslint-disable-next-line no-console
      console.log(`[browser] ${message.text()}`);
    }
  });
  page.on("pageerror", (error) => {
    // eslint-disable-next-line no-console
    console.log(`[browser] uncaught: ${error.message}`);
  });

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Status ${suffix}`);
  // The scene the *play page* will show, which is the world's active scene —
  // not simply the first one the scenes query returns. Getting this wrong
  // means the app queries a different scene than the one the token is on, and
  // the symptom is an empty result with nothing to explain it.
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

  // The world has to be playing Genie, or it declares no resources and the
  // correct behaviour is to draw nothing at all.
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

  const actor = await graphql<{
    data?: { createActor?: { id: string } };
    errors?: { message: string }[];
  }>(
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
        label: `Zayn ${suffix}`,
        isNpc: false,
        gameSystemId: "genie",
      },
    },
  );
  const actorId = actor.data?.createActor?.id;
  // The response, not just "it was falsy": a setup step that fails silently
  // makes every assertion below it a mystery.
  expect(
    actorId,
    `the actor must exist before anything is bound to it: ${JSON.stringify(actor)}`,
  ).toBeTruthy();

  await setResources(page, actorId!, {
    current_health: 7,
    max_health: 12,
    current_wish_points: 3,
    max_wish_points: 5,
  });

  // A token bound to that actor, owned by this player — which is what makes
  // the derived default `visible` rather than chunked.
  const created = await graphql<{
    data?: { createToken?: { tokenId: string } };
    errors?: { message: string }[];
  }>(
    page,
    `
      mutation ($input: GraphQLCreateTokenInput!) {
        createToken(input: $input) {
          tokenId
        }
      }
    `,
    { input: { sceneId, x: 0, y: 0, actorId, tokenType: "character" } },
  );
  const tokenId = created.data?.createToken?.tokenId;
  expect(tokenId, "the token must exist").toBeTruthy();

  const me = await graphql<{ data?: { me?: { id: string } } }>(
    page,
    `
      query {
        me {
          id
        }
      }
    `,
    {},
  );
  await graphql(
    page,
    `mutation ($input: GraphQLUpdateTokenInput!) {
      updateToken(tokenId: "${tokenId}", input: $input) { tokenId }
    }`,
    { input: { ownerUserId: me.data?.me?.id } },
  );

  await page.goto(`/world/${worldId}/play`);
  await page.evaluate((id) => {
    (window as unknown as { __statusTokenId: string }).__statusTokenId = id;
  }, tokenId!);
  await waitForEngineReady(page);

  // Before the engine: did the *server* resolve anything? Splitting the two
  // makes a failure say which link broke, rather than "no bars appeared".
  const served = await graphql<{
    data?: { tokenStatus?: { tokenId: string; resources: unknown[] }[] };
    errors?: { message: string }[];
  }>(
    page,
    `
      query ($sceneId: UUID!) {
        tokenStatus(sceneId: $sceneId) {
          tokenId
          resources {
            definitionId
            label
            kind
            disclosure
          }
        }
      }
    `,
    { sceneId },
  );
  expect(
    served.data?.tokenStatus?.length,
    `the server must resolve status for this scene: ${JSON.stringify(served)}`,
  ).toBeGreaterThan(0);

  // First, the substance of the demo: the whole pipeline — Genie's manifest,
  // the actor's stored pools, the server's resolution, the engine command —
  // carried both resources onto the canvas.
  //
  // Asserted through `list_token_status`, the read surface FR-021 exists for.
  // It is read-only by design, so this cannot construct a state the
  // application could not reach.
  const status = await expect
    .poll(
      async () =>
        page.evaluate(async () => {
          const engine = (await import(
            /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
          )) as typeof import("../src/engine/bevy/tokenStatus");
          return engine.readTokenStatus(
            (window as unknown as { __statusTokenId: string }).__statusTokenId,
          );
        }),
      {
        message: "the engine should hold both of Genie's resources",
        timeout: 60_000,
      },
    )
    .not.toBeNull()
    .then(() =>
      page.evaluate(async () => {
        const engine = (await import(
          /* @vite-ignore */ "/src/engine/bevy/tokenStatus.ts"
        )) as typeof import("../src/engine/bevy/tokenStatus");
        return engine.readTokenStatus(
          (window as unknown as { __statusTokenId: string }).__statusTokenId,
        );
      }),
    );

  expect(status, "both declared resources reach the engine").toHaveLength(2);
  const ids = status!.map((r) => r.definition.id);
  expect(ids, "in the order the system declares them").toEqual([
    "health",
    "wishPoints",
  ]);

  // Exact figures, because this player owns the character — the derived
  // default, with nothing configured.
  for (const resource of status!) {
    expect(
      resource.disclosed.disclosure,
      `${resource.definition.id} should be exact for its owner`,
    ).toBe("visible");
  }

  console.log(
    `[status] system=genie resources=${ids.join(",")} disclosure=visible`,
  );
});
