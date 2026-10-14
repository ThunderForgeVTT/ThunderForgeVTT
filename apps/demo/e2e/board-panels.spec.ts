import { expect, test, type Page } from "@playwright/test";
import { openSitting, sitting, WORLD_ID } from "./sitting";

/**
 * Spec 074 T021: the rest of what FR-005 promises a Game Master on the demo's
 * board, each through the control a Game Master uses, in one sitting on one
 * page, with the browser never reaching past the demo's own files (SC-003).
 *
 * - A rectangle is drawn with the shape tool, and it is the Game Master's.
 * - A token is placed from the actors pane: Place, then a click on the board.
 * - A message is sent from the chat panel, and is still there after a reload.
 * - A scene is switched with the board's own scene switcher, and the canvas
 *   holds the switched-to scene's walls. `demo-board.spec.ts` switches by
 *   asking `launchScene` directly; this is the control.
 */

let page: Page;
let outside: string[];

const { ask, currentScene, cast, scenes, clickBoard, dragBoard } = sitting(
  () => page,
);

/** The visitor, as the seed names them. */
const GAME_MASTER_ID = "d0000000-0000-4000-0001-000000000001";

type Shape = { shapeId: string; kind: string; createdBy: string };

async function shapesOf(sceneId: string): Promise<Shape[]> {
  const answer = await ask<{ shapes: Shape[] }>(
    "query ($sceneId: UUID!) { shapes(sceneId: $sceneId) { shapeId kind createdBy } }",
    { sceneId },
  );
  if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
  return answer.body.data.shapes;
}

async function tokensOf(
  sceneId: string,
): Promise<{ tokenId: string; actorId: string | null }[]> {
  const answer = await ask<{
    tokens: { tokenId: string; actorId: string | null }[];
  }>(
    "query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId actorId } }",
    {
      sceneId,
    },
  );
  if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
  return answer.body.data.tokens;
}

async function wallIdsOf(sceneId: string): Promise<string[]> {
  const answer = await ask<{ walls: { wallId: string }[] }>(
    "query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId } }",
    { sceneId },
  );
  if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
  return answer.body.data.walls.map((w) => w.wallId).sort();
}

/** Spec 085: the walls this canvas draws, from the engine probe, sorted. */
async function drawnWalls(): Promise<string[]> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: { drawnWalls?: () => string[] };
        }
      ).__engineProbe
        ?.drawnWalls?.()
        .slice()
        .sort() ?? [],
  );
}

async function openDockTab(tab: string): Promise<void> {
  const trigger = page.getByTestId(`world-dock-tab-${tab}`);
  await expect(trigger).toBeVisible({ timeout: 10_000 });
  if ((await trigger.getAttribute("aria-expanded")) !== "true") {
    await trigger.click();
  }
  await expect(page.getByTestId(`world-dock-panel-${tab}`)).toBeVisible();
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  ({ page, outside } = await openSitting(browser, baseURL));
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  // SC-003: checked after every step, so the step that reached out is the
  // one that fails.
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

test("the demo opens on the ambush with the engine running", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
  expect((await currentScene()).name).toBe("Grassy Path Ambush");
});

test("a rectangle is drawn with the shape tool, and it is the Game Master's", async () => {
  const { sceneId } = await currentScene();
  const before = await shapesOf(sceneId);
  await page.getByTestId("gm-tool-shapes").click();
  await expect(page.getByTestId("shape-tool")).toBeVisible();
  const rectangle = page.getByRole("button", { name: "Rectangle" });
  await rectangle.click();
  await expect(rectangle).toHaveAttribute("aria-pressed", "true");
  // The engine takes a while to be ready for a pointer after the canvas
  // appears; a drag it was not ready for draws nothing, so it is made again.
  await expect(async () => {
    await dragBoard({ x: 200, y: -200 }, { x: 320, y: -280 });
    await expect
      .poll(async () => (await shapesOf(sceneId)).length, { timeout: 3_000 })
      .toBe(before.length + 1);
  }).toPass({ timeout: 60_000 });
  const drawn = (await shapesOf(sceneId)).filter(
    (shape) => !before.some((b) => b.shapeId === shape.shapeId),
  );
  expect(drawn).toHaveLength(1);
  expect(drawn[0].kind).toBe("RECT");
  expect(drawn[0].createdBy).toBe(GAME_MASTER_ID);
  await page.keyboard.press("Escape");
});

test("a token is placed from the actors pane: Place, then a click on the board", async () => {
  const { sceneId } = await currentScene();
  const wizard = (await cast()).find((a) => a.label === "Elowen Vire");
  if (!wizard) throw new Error("the wizard is in the cast");
  const before = await tokensOf(sceneId);

  await openDockTab("actors");
  await page.getByTestId(`actor-place-${wizard.id}`).click();
  await expect(page.getByTestId("placement-preview")).toContainText(
    "Elowen Vire",
  );
  // Where nothing stands: the far south-west of the ambush.
  await expect(async () => {
    await clickBoard({ x: -300, y: -250 });
    await expect
      .poll(async () => (await tokensOf(sceneId)).length, { timeout: 3_000 })
      .toBe(before.length + 1);
  }).toPass({ timeout: 60_000 });
  await expect(page.getByTestId("placement-placed")).toContainText(
    "Elowen Vire is on the map.",
  );
  const placed = (await tokensOf(sceneId)).filter(
    (token) => !before.some((b) => b.tokenId === token.tokenId),
  );
  expect(placed).toHaveLength(1);
  expect(placed[0].actorId).toBe(wizard.id);
});

test("a message is sent from the chat panel, and is still there after a reload", async () => {
  const body = "The goblins are in the grass.";
  await openDockTab("chat");
  await page.getByTestId("chat-input").fill(body);
  await page.getByTestId("chat-send-button").click();
  // The composer clears only after the mutation resolves.
  await expect(page.getByTestId("chat-input")).toHaveValue("", {
    timeout: 20_000,
  });
  await expect(
    page.getByTestId("chat-message").filter({ hasText: body }),
  ).toHaveCount(1, { timeout: 20_000 });

  await page.reload();
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
  await openDockTab("chat");
  await expect(
    page.getByTestId("chat-message").filter({ hasText: body }),
  ).toHaveCount(1, { timeout: 20_000 });
  // And the backend holds it as the sender's own.
  const answer = await ask<{
    worldChatMessages: { body: string; authorUserId: string }[];
  }>(
    "query ($worldId: UUID!) { worldChatMessages(worldId: $worldId) { body authorUserId } }",
    { worldId: WORLD_ID },
  );
  expect(answer.body.data?.worldChatMessages).toEqual([
    { body, authorUserId: GAME_MASTER_ID },
  ]);
});

test("the board's scene switcher puts The Proving Ground on the canvas", async () => {
  const ambush = await currentScene();
  const ground = (await scenes()).find((s) => s.name === "The Proving Ground");
  if (!ground) throw new Error("The Proving Ground is a scene of the demo");
  const ambushWalls = await wallIdsOf(ambush.sceneId);
  const groundWalls = await wallIdsOf(ground.sceneId);
  expect(groundWalls.length).toBeGreaterThan(0);
  expect(groundWalls).not.toEqual(ambushWalls);
  // The canvas draws the ambush's walls before the switch.
  await expect.poll(drawnWalls, { timeout: 30_000 }).toEqual(ambushWalls);

  await openDockTab("settings");
  const switcher = page.getByTestId("scene-switcher");
  await expect(switcher).toContainText("Grassy Path Ambush");
  await switcher.click();
  await page.getByRole("option", { name: "The Proving Ground" }).click();
  await expect(switcher).toContainText("The Proving Ground", {
    timeout: 10_000,
  });
  // The canvas now draws the Proving Ground's walls, and none of the
  // ambush's.
  await expect.poll(drawnWalls, { timeout: 60_000 }).toEqual(groundWalls);
});
