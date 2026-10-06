import { expect, test, type Page } from "@playwright/test";

/**
 * Spec 074: the demo, as it ships. One visitor, one tab, one world, served
 * from static files with nothing behind them.
 *
 * The tests share a page and run in order, because that is what is being
 * claimed: one sitting in which a Game Master does each of these things and
 * the browser never once reaches past the demo's own files (SC-003).
 */

const WORLD_ID = "d0000000-0000-4000-0002-000000000001";

type Actor = {
  id: string;
  label: string;
  isNpc: boolean;
  ownedBy: string;
  visibleToPlayers: boolean;
  myPermissionLevel: string;
};

type Scene = {
  sceneId: string;
  name: string;
  walls: {
    wallId: string;
    doorState: string;
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  }[];
  lightSources: { lightId: string }[];
  tokens: { tokenId: string; x: number; y: number }[];
};

let page: Page;
/** Every request and socket that is not one of the demo's own files. */
const outside: string[] = [];

/**
 * Ask the demo's own backend, the way the app does. The page's `fetch` is the
 * demo's, so this goes nowhere; if it ever did, the request log would say so.
 */
async function ask<T>(query: string, variables: object = {}) {
  return page.evaluate(
    async ([query, variables]) => {
      const response = await fetch("/api/graphql", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ query, variables }),
      });
      return { status: response.status, body: await response.json() };
    },
    [query, variables] as const,
  ) as Promise<{
    status: number;
    body: { data?: T; errors?: { extensions?: { code?: string } }[] };
  }>;
}

async function scenes(): Promise<{ sceneId: string; name: string }[]> {
  const answer = await ask<{ scenes: { sceneId: string; name: string }[] }>(
    "query ($worldId: UUID!) { scenes(worldId: $worldId) { sceneId name } }",
    { worldId: WORLD_ID },
  );
  if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
  return answer.body.data.scenes;
}

/** The scene the play field is showing, as the demo's backend holds it now. */
async function currentScene(): Promise<Scene> {
  const world = await ask<{ world: { activeSceneId: string } }>(
    "query ($id: UUID!) { world(id: $id) { activeSceneId } }",
    { id: WORLD_ID },
  );
  const active = world.body.data?.world.activeSceneId;
  const scene = (await scenes()).find((s) => s.sceneId === active);
  if (!scene) throw new Error("no scene is active");
  const answer = await ask<Omit<Scene, "sceneId" | "name">>(
    `query ($sceneId: UUID!) {
      walls(sceneId: $sceneId) { wallId doorState x1 y1 x2 y2 }
      lightSources(sceneId: $sceneId) { lightId }
      tokens(sceneId: $sceneId) { tokenId x y }
    }`,
    { sceneId: scene.sceneId },
  );
  if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
  return { ...scene, ...answer.body.data };
}

async function cast(): Promise<Actor[]> {
  const answer = await ask<{ worldActors: Actor[] }>(
    `query ($worldId: UUID!) { worldActors(worldId: $worldId) {
      id label isNpc ownedBy visibleToPlayers myPermissionLevel
    } }`,
    { worldId: WORLD_ID },
  );
  if (!answer.body.data) throw new Error(JSON.stringify(answer.body.errors));
  return answer.body.data.worldActors;
}

/** Where the fighter stands when the demo ships: on the road, mid-map. */
const FIGHTER_START = { x: -297.5, y: 85 };

/**
 * Board coordinates to the screen, at the camera a scene opens with: centred
 * on the origin, one board unit to a pixel, y up. The built bundle carries no
 * engine probe to ask, and the tests never move the camera.
 */
async function toScreen(point: { x: number; y: number }) {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the board's canvas is not on the page");
  return {
    x: box.x + box.width / 2 + point.x,
    y: box.y + box.height / 2 - point.y,
  };
}

async function clickBoard(
  point: { x: number; y: number },
  button: "left" | "right" = "left",
) {
  const at = await toScreen(point);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down({ button });
  // The engine reads the pointer once a frame; a press and release inside
  // one frame is a click it never saw.
  await page.waitForTimeout(120);
  await page.mouse.up({ button });
}

test.describe.configure({ mode: "serial" });

test.beforeAll(async ({ browser, baseURL }) => {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  page = await context.newPage();
  const origin = new URL(baseURL as string).origin;
  context.on("request", (request) => {
    const url = new URL(request.url());
    if (url.protocol === "blob:" || url.protocol === "data:") return;
    if (url.origin === origin && url.pathname.startsWith("/demo/")) return;
    outside.push(`${request.method()} ${request.url()}`);
  });
  page.on("websocket", (socket) => outside.push(`SOCKET ${socket.url()}`));
});

test.afterAll(async () => {
  await page.context().close();
});

test.afterEach(() => {
  // SC-003: checked after every step, so the step that reached out is the
  // one that fails.
  expect(outside, "nothing leaves the demo's own static files").toEqual([]);
});

test("the demo opens on a world's dashboard, with the visitor as its Game Master", async () => {
  await page.goto("/demo/");
  await expect(page).toHaveURL(new RegExp(`/demo/world/${WORLD_ID}$`));
  await expect(page.getByTestId("demo-notice")).toContainText(
    "Nothing is saved anywhere but this browser",
  );
  // FR-018: whose maps these are, on the page.
  await expect(page.getByTestId("demo-notice")).toContainText("MBRound18");
  await expect(page.getByTestId("demo-notice")).toContainText("CC BY-SA 4.0");
  await expect(
    page
      .getByRole("link", { name: /enter world/i })
      .or(page.getByRole("button", { name: /enter world/i }))
      .first(),
  ).toBeVisible();
});

test("Enter world leads to a play field with the engine running", async () => {
  await page
    .getByRole("link", { name: /enter world/i })
    .or(page.getByRole("button", { name: /enter world/i }))
    .first()
    .click();
  await page.getByTestId("play-button").click();
  await expect(page).toHaveURL(/\/world\/[^/]+\/play$/);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
  await expect(page.getByTestId("demo-notice")).toBeVisible();
});

test("a token dragged across the board is saved where it was left", async () => {
  const before = (await currentScene()).tokens[0];
  const from = { x: before.x, y: before.y };
  const to = { x: before.x + 117 * 3, y: before.y - 117 * 2 };
  // The engine takes a while to be ready for a pointer after the canvas
  // appears, and the built bundle has no probe that says when. A drag it was
  // not ready for moves nothing, so it is simply made again.
  await expect(async () => {
    const start = await toScreen(from);
    const end = await toScreen(to);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(end.x, end.y, { steps: 14 });
    await page.waitForTimeout(150);
    await page.mouse.up();
    await expect
      .poll(async () => (await currentScene()).tokens[0].x, { timeout: 3_000 })
      .not.toBe(before.x);
  }).toPass({ timeout: 90_000 });
  const after = (await currentScene()).tokens[0];
  expect(Math.abs(after.x - to.x)).toBeLessThan(117);
  expect(Math.abs(after.y - to.y)).toBeLessThan(117);
});

test("a Game Master's keyboard walks the token they selected", async () => {
  // The Game Master owns no token, so until 2026-10-05 WASD moved nothing on
  // their board. Now the keys follow the single selected token, and the step
  // is saved the way a drag is.
  const before = (await currentScene()).tokens[0];
  await expect(async () => {
    await clickBoard({ x: before.x, y: before.y });
    await page.waitForTimeout(150);
    await page.keyboard.press("KeyD");
    await expect
      .poll(async () => (await currentScene()).tokens[0].x, { timeout: 3_000 })
      .toBeGreaterThan(before.x);
  }).toPass({ timeout: 60_000 });
  const after = (await currentScene()).tokens[0];
  // One cell east, give or take the snap a dragged token may still owe.
  expect(after.x - before.x).toBeLessThanOrEqual(117 * 1.5);
  expect(Math.abs(after.y - before.y)).toBeLessThan(117);
});

test("the ambush is on the board: two heroes a player owns, five monsters only the Game Master sees", async () => {
  const actors = await cast();
  expect(actors.map((a) => a.label).sort()).toEqual([
    "Brannoc Stoneward",
    "Dire Wolf",
    "Elowen Vire",
    "Goblin Warrior",
    "Hobgoblin Warrior",
  ]);
  const heroes = actors.filter((a) => !a.isNpc);
  expect(heroes).toHaveLength(2);
  expect(heroes.every((a) => a.ownedBy !== actors[2].ownedBy)).toBe(true);
  expect(actors.filter((a) => a.isNpc).every((a) => !a.visibleToPlayers)).toBe(
    true,
  );
  expect(actors.every((a) => a.myPermissionLevel === "OWNER")).toBe(true);
  expect((await currentScene()).tokens).toHaveLength(7);
});

test("the fighter's sheet opens with his numbers, and a check rolls from them", async () => {
  const fighter = (await cast()).find((a) => a.label === "Brannoc Stoneward");
  if (!fighter) throw new Error("the fighter is in the cast");
  await page.goto(`/demo/world/${WORLD_ID}/actor/${fighter.id}/edit`);
  await expect(page.getByTestId("dnd5e-actor-sheet")).toBeVisible();
  await expect(page.getByTestId("dnd5e-mod-strength")).toHaveText("+3");
  // Athletics: +3 Strength, proficient at third level.
  await expect(page.getByTestId("dnd5e-skill-athletics-bonus")).toHaveText(
    "+5",
  );
  const rolled = await ask<{
    rollCheck: { resultValue: number; dice: { finalValue: number }[] };
  }>(
    `mutation ($worldId: UUID!, $actorId: UUID!, $checkId: String!) {
      rollCheck(worldId: $worldId, actorId: $actorId, checkId: $checkId) {
        formula resultKind resultValue dice { finalValue kept }
      }
    }`,
    { worldId: WORLD_ID, actorId: fighter.id, checkId: "athletics" },
  );
  if (!rolled.body.data) throw new Error(JSON.stringify(rolled.body.errors));
  const { resultValue, dice } = rolled.body.data.rollCheck;
  expect(dice[0].finalValue).toBeGreaterThanOrEqual(1);
  expect(dice[0].finalValue).toBeLessThanOrEqual(20);
  expect(resultValue).toBe(dice[0].finalValue + 5);
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
});

test("a wall is drawn with the wall tool", async () => {
  const before = (await currentScene()).walls.length;
  await page.getByTestId("gm-tool-walls").click();
  await expect(page.getByTestId("gm-tool-panel-walls")).toBeVisible();
  await clickBoard({ x: -300, y: 300 });
  await page.waitForTimeout(300);
  await clickBoard({ x: -100, y: 300 });
  await page.waitForTimeout(300);
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => (await currentScene()).walls.length)
    .toBe(before + 1);
});

test("a door is opened from its own menu, on a scene the Game Master switched to", async () => {
  // The ambush is in open country. The Proving Ground has doors; launching
  // it is the same announcement a server makes, and the play field follows.
  const ground = (await scenes()).find((s) => s.name === "The Proving Ground");
  if (!ground) throw new Error("The Proving Ground is a scene of the demo");
  await ask(
    "mutation ($worldId: UUID!, $sceneId: UUID!) { launchScene(worldId: $worldId, sceneId: $sceneId) { id } }",
    { worldId: WORLD_ID, sceneId: ground.sceneId },
  );
  await expect
    .poll(async () => (await currentScene()).name)
    .toBe("The Proving Ground");
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  const door = (await currentScene()).walls.find(
    (wall) => wall.doorState === "CLOSED",
  );
  if (!door) throw new Error("the first scene ships with a closed door");
  await page.getByTestId("gm-tool-select").click();
  await clickBoard(
    { x: (door.x1 + door.x2) / 2, y: (door.y1 + door.y2) / 2 },
    "right",
  );
  await page.getByRole("menuitem", { name: "Open", exact: true }).click();
  await expect
    .poll(
      async () =>
        (await currentScene()).walls.find((wall) => wall.wallId === door.wallId)
          ?.doorState,
    )
    .toBe("OPEN");
});

test("a light is placed with the light tool", async () => {
  const before = (await currentScene()).lightSources.length;
  await page.getByTestId("gm-tool-lights").click();
  await expect(page.getByTestId("gm-tool-panel-lights")).toBeVisible();
  await clickBoard({ x: 300, y: 250 });
  await expect
    .poll(async () => (await currentScene()).lightSources.length)
    .toBe(before + 1);
});

test("what the visitor did is still there after a reload", async () => {
  const before = await currentScene();
  await page.reload();
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
  const after = await currentScene();
  expect(after.walls.length).toBe(before.walls.length);
  expect(after.lightSources.length).toBe(before.lightSources.length);
  expect(after.tokens[0]).toEqual(before.tokens[0]);
  expect(after.walls.filter((wall) => wall.doorState === "OPEN")).toHaveLength(
    1,
  );
});

test("every example map is a scene that opens, credited to its author", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/scenes`);
  const rows = page.getByTestId("scenes-table").locator("tbody tr");
  await expect(rows).toHaveCount(7);
  for (const scene of await scenes()) {
    await page.goto(`/demo/world/${WORLD_ID}/scenes/${scene.sceneId}`);
    await expect(page.getByTestId("scene-not-found")).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: scene.name }).first(),
    ).toBeVisible();
    await expect(page.getByTestId("demo-notice")).toContainText("MBRound18");
  }
  const notice = await page.request.get("/demo/maps/NOTICE.txt");
  expect(notice.ok()).toBe(true);
  expect(await notice.text()).toContain("MBRound18");
});

test("something the demo does not do says so, and is not a connection error", async () => {
  await page.goto(`/demo/world/${WORLD_ID}`);
  const answer = await ask(
    "mutation ($input: GenerateInviteCodeInput!) { generateInviteCode(input: $input) { id } }",
    { input: { worldId: WORLD_ID, maxUses: 1 } },
  );
  expect(answer.status).toBe(200);
  expect(answer.body.errors?.[0]?.extensions?.code).toBe("NOT_IN_DEMO");
  await expect(page.getByText("Not part of the demo")).toBeVisible();
});

test("as a player, the heroes are theirs and the ambush is not yet there to see", async () => {
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as a player",
  );
  // A player's dashboard: no owner's controls, and no "not part of the demo"
  // for anything it asks on the way in.
  await expect(page.getByRole("button", { name: "Delete world" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
  const seen = await cast();
  expect(seen.map((a) => a.label).sort()).toEqual([
    "Brannoc Stoneward",
    "Elowen Vire",
  ]);
  expect(seen.every((a) => a.myPermissionLevel === "OWNER")).toBe(true);
  // And they arrive already playing the fighter (spec 023), not choosing.
  const claim = await ask<{ myActorClaim: { actor: { label: string } } }>(
    "query ($worldId: UUID!) { myActorClaim(worldId: $worldId) { actorId actor { label } } }",
    { worldId: WORLD_ID },
  );
  expect(claim.body.data?.myActorClaim.actor.label).toBe("Brannoc Stoneward");
  const ambush = (await scenes()).find((s) => s.name === "Grassy Path Ambush");
  if (!ambush) throw new Error("Grassy Path Ambush is a scene of the demo");
  const tokens = await ask<{ tokens: { tokenId: string }[] }>(
    "query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId } }",
    { sceneId: ambush.sceneId },
  );
  expect(tokens.body.data?.tokens).toHaveLength(2);
  // And the play field is a player's: a board with no Game Master's tools.
  await page.goto(`/demo/world/${WORLD_ID}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toHaveCount(0);
  await page.getByRole("button", { name: "View as Game Master" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as the Game Master",
  );
  expect(await cast()).toHaveLength(5);
});

test("Start over restores the world the demo shipped with", async () => {
  await page.goto(`/demo/world/${WORLD_ID}`);
  await page.getByRole("button", { name: "Start over" }).click();
  await expect(page).toHaveURL(new RegExp(`/demo/world/${WORLD_ID}$`));
  await expect
    .poll(async () => {
      const scene = await currentScene().catch(() => null);
      return (
        scene && {
          name: scene.name,
          x: scene.tokens[0].x,
          y: scene.tokens[0].y,
        }
      );
    })
    .toEqual({ name: "Grassy Path Ambush", ...FIGHTER_START });
  const ground = (await scenes()).find((s) => s.name === "The Proving Ground");
  const answer = await ask<{ walls: { doorState: string }[] }>(
    "query ($sceneId: UUID!) { walls(sceneId: $sceneId) { doorState } }",
    { sceneId: ground!.sceneId },
  );
  expect(
    answer.body.data?.walls.filter((w) => w.doorState === "OPEN"),
  ).toHaveLength(0);
});
