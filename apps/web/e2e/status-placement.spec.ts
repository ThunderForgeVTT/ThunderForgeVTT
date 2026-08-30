import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029, User Story 5 — the viewer decides where the panel lives, and the
 * decision outlives the page (FR-011); nothing is selected, so nothing is
 * shown (FR-012).
 *
 * # Why this is an e2e and not a unit test
 *
 * Persistence is the whole claim, and persistence is made of things a unit
 * test has to fake: `localStorage`, a page teardown, a fresh module graph on
 * the way back. `apps/web/src/components/StatusPanel/__tests__/placement.test.ts`
 * proves the panel renders each corner and reports a change; only a real
 * reload in a real browser proves the corner is still there afterwards.
 *
 * The corner is asserted geometrically — which half of the viewport the panel
 * is actually drawn in — rather than by class name. A class is a promise; a
 * bounding box is where the thing ended up.
 */

const PANEL = 'aside[aria-label="Selected token status"]';

async function gql<T>(
  page: Page,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  // The e2e helper returns the whole `{ data, errors }` envelope — unlike the
  // app's `postGraphQL`, which unwraps it.
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

/** Click at a pixel offset from the canvas centre.
 *
 * Not `page.mouse.click()`: Bevy reads `just_pressed`/`just_released` from
 * window events it has polled, and a zero-delay synthetic down+up pair can
 * land inside one frame and collapse into a press with no release. The
 * explicit delay guarantees a frame boundary between them, the way a real
 * click's tens of milliseconds do. */
async function clickCanvasAt(
  page: Page,
  dx: number,
  dy: number,
): Promise<void> {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the canvas must be laid out before it is clicked");
  await page.mouse.move(
    box.x + box.width / 2 + dx,
    box.y + box.height / 2 + dy,
  );
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();
}

/** Select the token at the world origin, and wait until the store agrees. */
async function selectOriginToken(page: Page, tokenId: string): Promise<void> {
  await expect
    .poll(
      async () => {
        await clickCanvasAt(page, 0, 0);
        return page.evaluate(
          () =>
            (
              window as unknown as {
                __worldProbe?: {
                  state: () => { selectedTokenId: string | null };
                };
              }
            ).__worldProbe?.state().selectedTokenId ?? null,
        );
      },
      {
        message: "clicking the token at the world origin should select it",
        timeout: 60_000,
        intervals: [1_000],
      },
    )
    .toBe(tokenId);
}

/** Which half of the viewport the panel's centre falls in. */
async function panelQuadrant(page: Page): Promise<{
  vertical: "top" | "bottom";
  horizontal: "left" | "right";
}> {
  const box = await page.locator(PANEL).boundingBox();
  if (!box) throw new Error("the panel must be on screen to be located");
  const viewport = page.viewportSize();
  if (!viewport) throw new Error("no viewport");
  return {
    vertical: box.y + box.height / 2 < viewport.height / 2 ? "top" : "bottom",
    horizontal: box.x + box.width / 2 < viewport.width / 2 ? "left" : "right",
  };
}

test("the corner a viewer chooses for the status panel survives a reload", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  page.on("pageerror", (error) => {
    console.log(`[browser] uncaught: ${error.message}`);
  });

  const suffix = uniqueSuffix();
  const worldId = await registerAndCreateWorld(page, `Placement ${suffix}`);

  const active = await gql<{ world: { activeSceneId: string | null } }>(
    page,
    // `world` takes `id`, not `worldId`.
    `query ($id: UUID!) { world(id: $id) { activeSceneId } }`,
    { id: worldId },
  );
  const [firstScene] = await sceneIds(page, worldId);
  const sceneId = active.world.activeSceneId ?? firstScene;

  // Without a system declaring resources the correct behaviour is to draw
  // nothing, and this test would pass against a panel that never appeared.
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
        label: `Zayn ${suffix}`,
        isNpc: false,
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
          current_health: 7,
          max_health: 12,
          current_wish_points: 3,
          max_wish_points: 5,
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
        x: 0,
        y: 0,
        actorId: actor.createActor.id,
        tokenType: "character",
      },
    },
  );
  const tokenId = created.createToken.tokenId;

  // Owned by this player, which is what makes the figures exact rather than
  // chunked — and makes "7 / 12" a thing the panel can be asked to show.
  const me = await gql<{ me: { id: string } }>(page, `query { me { id } }`, {});
  await gql(
    page,
    `mutation ($input: GraphQLUpdateTokenInput!) {
      updateToken(tokenId: "${tokenId}", input: $input) { tokenId }
    }`,
    { input: { ownerUserId: me.me.id } },
  );

  await page.goto(`/world/${worldId}/play`);
  await waitForEngineReady(page);

  // FR-012, before anything is selected: no panel, not an empty one.
  await expect(
    page.locator(PANEL),
    "nothing is selected, so nothing should be in the corner",
  ).toHaveCount(0);

  await selectOriginToken(page, tokenId);

  const panel = page.locator(PANEL);
  await expect(panel).toBeVisible({ timeout: 30_000 });
  await expect(panel).toContainText("7 / 12");

  // The default, and the thing the reload has to change away from.
  expect(await panelQuadrant(page)).toEqual({
    vertical: "bottom",
    horizontal: "right",
  });

  // The viewer moves it, because the bottom right is where their initiative
  // tracker is.
  await page.getByLabel("Panel position").selectOption("top-left");
  await expect
    .poll(() => panelQuadrant(page), {
      message: "the panel should move to the corner that was chosen",
      timeout: 5_000,
    })
    .toEqual({ vertical: "top", horizontal: "left" });

  // The whole point: a real reload, a fresh module graph, a fresh store.
  await page.reload();
  await waitForEngineReady(page);
  await expect(
    page.locator(PANEL),
    "a reload deselects, so the panel starts absent again",
  ).toHaveCount(0);

  await selectOriginToken(page, tokenId);
  await expect(page.locator(PANEL)).toBeVisible({ timeout: 30_000 });

  expect(
    await panelQuadrant(page),
    "the corner chosen before the reload is still the corner after it",
  ).toEqual({ vertical: "top", horizontal: "left" });
  await expect(page.getByLabel("Panel position")).toHaveValue("top-left");

  // FR-012 again, this time on the path that could actually go wrong:
  // deselecting a token whose figures are already on screen.
  await page.keyboard.press("Escape");
  await clickCanvasAt(page, 320, 240);
  await expect(
    page.locator(PANEL),
    "deselecting must take the previous token's figures with it",
  ).toHaveCount(0, { timeout: 15_000 });

  console.log(
    `[placement] chosen=top-left survived_reload=true cleared_on_deselect=true`,
  );
});
