import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { sceneIds } from "./fixtures/world-cache";

/**
 * Spec 029 FR-010a–FR-012a (playtest 2026-09-10 P6) — selecting a token opens
 * no panel, because its bars above it are its display; double-clicking it pins
 * one, where the double-click was; the viewer drags it, it outlives the
 * selection, it closes, and where they left it is where the next one opens,
 * reload or not.
 *
 * # Why this is an e2e and not a unit test
 *
 * A drag is a real pointer, the double-click is the browser's own event on the
 * canvas, and persistence is `localStorage` across a page teardown — all
 * things a unit test would have to fake. `placement.test.ts` proves the panel
 * renders where it is told and exposes its controls; only a browser proves the
 * gesture, the drag and the reload.
 *
 * Positions are asserted from the panel's bounding box — where it was actually
 * drawn — rather than from its inline style.
 */

const PANEL = 'aside[aria-label="Pinned token status"]';

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

/** The canvas centre, in page pixels — where the origin token is drawn. */
async function canvasCentre(page: Page): Promise<{ x: number; y: number }> {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the canvas must be laid out before it is used");
  return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
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
  const centre = await canvasCentre(page);
  await page.mouse.move(centre.x + dx, centre.y + dy);
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

/** The pinned panel's top-left corner, as drawn. */
async function panelAt(page: Page): Promise<{ x: number; y: number }> {
  const box = await page.locator(PANEL).boundingBox();
  if (!box) throw new Error("the panel must be on screen to be located");
  return { x: Math.round(box.x), y: Math.round(box.y) };
}

test("a double-click pins the status panel, which is dragged, outlives the selection, closes, and reopens where it was left", async ({
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

  // FR-010a: selecting the token is not a request for a panel — its bars
  // above it are its display.
  await selectOriginToken(page, tokenId);
  await page.waitForTimeout(1_000);
  await expect(
    page.locator(PANEL),
    "selecting a token must not open a panel",
  ).toHaveCount(0);

  // FR-011a: a double-click pins one, where the double-click was.
  const centre = await canvasCentre(page);
  await page.mouse.dblclick(centre.x, centre.y);
  const panel = page.locator(PANEL);
  await expect(panel).toBeVisible({ timeout: 15_000 });
  await expect(panel).toContainText("7 / 12");
  const opened = await panelAt(page);
  expect(Math.abs(opened.x - Math.round(centre.x))).toBeLessThanOrEqual(2);
  expect(Math.abs(opened.y - Math.round(centre.y))).toBeLessThanOrEqual(2);

  // Dragged by its header to where it does not cover the token.
  const handle = page.getByLabel("Move status panel");
  const grip = await handle.boundingBox();
  if (!grip) throw new Error("the handle must be on screen to be dragged");
  await page.mouse.move(grip.x + 20, grip.y + grip.height / 2);
  await page.mouse.down();
  await page.mouse.move(grip.x + 20 - 300, grip.y + grip.height / 2 - 150, {
    steps: 10,
  });
  await page.mouse.up();
  const dragged = await panelAt(page);
  expect(dragged.x).toBeLessThan(opened.x - 250);
  expect(dragged.y).toBeLessThan(opened.y - 100);

  // FR-012a: a pin outlives the selection.
  await page.keyboard.press("Escape");
  await clickCanvasAt(page, 320, 240);
  await page.waitForTimeout(1_000);
  await expect(
    panel,
    "deselecting must not unpin — a pin is kept until it is closed",
  ).toBeVisible();

  // Closed by the viewer.
  await page.getByLabel("Unpin status panel").click();
  await expect(panel).toHaveCount(0);

  // A reload, and the next pin opens where the last one was left — not where
  // this double-click is.
  await page.reload();
  await waitForEngineReady(page);
  await expect(
    page.locator(PANEL),
    "a reload leaves nothing pinned",
  ).toHaveCount(0);
  await selectOriginToken(page, tokenId);
  const again = await canvasCentre(page);
  await page.mouse.dblclick(again.x, again.y);
  await expect(page.locator(PANEL)).toBeVisible({ timeout: 15_000 });
  const reopened = await panelAt(page);
  expect(Math.abs(reopened.x - dragged.x)).toBeLessThanOrEqual(2);
  expect(Math.abs(reopened.y - dragged.y)).toBeLessThanOrEqual(2);

  console.log(
    `[placement] pinned_on_dblclick=true dragged=true outlived_selection=true reopened_where_left=true`,
  );
});
