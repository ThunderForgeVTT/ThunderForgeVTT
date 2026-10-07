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
  tokens: { tokenId: string; x: number; y: number; photoUrl: string | null }[];
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
    body: {
      data?: T;
      errors?: { message?: string; extensions?: { code?: string } }[];
    };
  }>;
}

/** What this page load has refused as "not part of the demo", by name. */
async function refusedSoFar(): Promise<string[]> {
  return page.evaluate(() => [
    ...((window as { __thunderforgeDemoRefused?: string[] })
      .__thunderforgeDemoRefused ?? []),
  ]);
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
      tokens(sceneId: $sceneId) { tokenId x y photoUrl }
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
  // Which actor pictures were asked of the page's `fetch`, and by whom it was
  // called: whatever the guard installs as `window.fetch` is wrapped as it is
  // read, so a call from the engine is seen as well as one from a script.
  await page.addInitScript(() => {
    const seen: string[] = [];
    let current = window.fetch;
    Object.assign(window, { __actorArtFetched: seen });
    Object.defineProperty(window, "fetch", {
      configurable: true,
      get() {
        const target = current;
        return (input: RequestInfo | URL, init?: RequestInit) => {
          const url = input instanceof Request ? input.url : String(input);
          if (url.includes("/api/actor-assets/")) seen.push(url);
          return target.call(window, input, init);
        };
      },
      set(value: typeof fetch) {
        current = value;
      },
    });
  });
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

test("the ambush is drawn with the cast's own faces, not coloured squares", async () => {
  const tokens = (await currentScene()).tokens;
  expect(tokens).toHaveLength(7);
  const urls = tokens.map((token) => token.photoUrl);
  for (const url of urls) {
    expect(url).toMatch(/^\/api\/actor-assets\/[0-9a-f-]{36}\.png$/);
  }
  // The engine loads each one through the page's `fetch`, which is the
  // guard's: Bevy's wasm asset reader calls `window.fetch`.
  await expect
    .poll(
      () =>
        page.evaluate(() =>
          (
            window as unknown as { __actorArtFetched: string[] }
          ).__actorArtFetched.map(
            (url) => new URL(url, location.href).pathname,
          ),
        ),
      { timeout: 30_000 },
    )
    .toEqual(expect.arrayContaining([...new Set(urls)]));
  const answers = await page.evaluate(
    (urls) =>
      Promise.all(
        urls.map(async (url) => {
          const response = await fetch(url);
          const blob = await response.blob();
          const bytes = new Uint8Array(await blob.arrayBuffer());
          const bitmap = await createImageBitmap(blob);
          return {
            status: response.status,
            type: response.headers.get("content-type"),
            head: [...bytes.slice(0, 8)],
            width: bitmap.width,
            height: bitmap.height,
          };
        }),
      ),
    [...new Set(urls)] as string[],
  );
  for (const answer of answers) {
    expect(answer).toEqual({
      status: 200,
      type: "image/png",
      head: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
      width: 512,
      height: 512,
    });
  }
  // For the record: what the board looks like with them on it.
  await page.waitForTimeout(1_000);
  await page.screenshot({ path: test.info().outputPath("ambush.png") });
});

test("a lore entry is written, edited and listed, and its link reaches the fighter's sheet", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/compendium?tab=lore`);
  const table = page.getByTestId("lore-catalog-table");
  // The ambush's own entries are there to read before anything is written.
  await expect(table.getByText("The Stoneward Oath")).toBeVisible({
    timeout: 30_000,
  });

  await page.getByTestId("new-lore-entry-title-input").fill("Camp Notes");
  await page.getByTestId("add-lore-entry-button").click();
  await expect(table.getByText("Camp Notes")).toBeVisible();

  await page.goto(`/demo/world/${WORLD_ID}/lore/camp-notes/edit`);
  const editor = page
    .getByTestId("lore-markdown-editor-textarea")
    .locator(".cm-content");
  await editor.click();
  await page.keyboard.type("Watch kept by [[Brannoc Stoneward]] until dawn.");
  await page.locator("#lore-entry-title").fill("Camp Notes, Night One");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Saved.")).toBeVisible();
  await expect(page).toHaveURL(/\/lore\/camp-notes-night-one\/edit$/);

  await page.goto(`/demo/world/${WORLD_ID}/lore/camp-notes-night-one/view`);
  const link = page.locator("a.lore-link", { hasText: "Brannoc Stoneward" });
  await expect(link).toBeVisible();
  await expect(link).toHaveAttribute(
    "href",
    new RegExp(`^/demo/world/${WORLD_ID}/actor/[^/]+/view$`),
  );
  await link.click();
  await expect(page.getByTestId("dnd5e-actor-sheet")).toBeVisible();

  await page.goto(`/demo/world/${WORLD_ID}/compendium?tab=lore`);
  await expect(table.getByText("Camp Notes, Night One")).toBeVisible();
  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
});

test("the compendium's Books tab says the world holds no books, rather than refusing", async () => {
  await page.goto(`/demo/world/${WORLD_ID}/compendium?tab=books`);
  await expect(page.getByRole("tab", { name: /books/i })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await page.waitForLoadState("networkidle");
  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
});

test("controls that only a real instance has are not offered, and session notes are kept", async () => {
  // FR-013: offered and then refused is worse than not offered.
  await page.goto(`/demo/world/${WORLD_ID}`);
  await expect(
    page.getByRole("link", { name: /enter world/i }).first(),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Delete world" })).toHaveCount(
    0,
  );
  await expect(page.getByText("Create another world")).toHaveCount(0);
  await expect(page.getByText("Permanently delete this world")).toHaveCount(0);
  await expect(page.getByText("Generate Join Link")).toHaveCount(0);

  await page.getByRole("button", { name: "Menu" }).click();
  await expect(
    page.getByRole("menuitem", { name: "World archive" }),
  ).toBeVisible();
  await expect(page.getByRole("menuitem", { name: "Sign out" })).toHaveCount(0);
  await page.keyboard.press("Escape");

  await page.goto("/demo/settings/account");
  await expect(page.getByTestId("account-demo")).toBeVisible();
  await expect(
    page.getByRole("button", { name: /delete account/i }),
  ).toHaveCount(0);
  await expect(page.getByRole("button", { name: /download/i })).toHaveCount(0);

  await page.goto("/demo/status");
  await expect(page.getByTestId("status-demo")).toContainText(
    "Running in your browser",
  );

  await page.goto(`/demo/world/${WORLD_ID}/staging`);
  await expect(page.getByTestId("session-setup-invite-link")).toHaveCount(0);
  const notes = page.getByTestId("session-notes-editor").locator(".cm-content");
  await notes.click();
  await page.keyboard.type("The goblins owe the party a cart.");
  await page.getByTestId("session-notes-save-button").click();
  await expect(page.getByText("Saved.")).toBeVisible();
  const saved = await ask<{ world: { sessionNotes: string } }>(
    `query ($id: UUID!) { world(id: $id) { sessionNotes } }`,
    { id: WORLD_ID },
  );
  expect(saved.body.data?.world.sessionNotes).toContain("owe the party a cart");

  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
});

test("every page of the Game Master's sidebar opens without the demo refusing anything", async () => {
  // T023. A fresh page load, so what was refused is this walk's alone.
  await page.goto(`/demo/world/${WORLD_ID}/staging`);
  const nav = page.getByTestId("world-sidebar-nav");
  await expect(nav).toBeVisible();
  const links = await nav.getByRole("link").evaluateAll((anchors) =>
    anchors.map((a) => ({
      name: (a.textContent ?? "").trim(),
      href: a.getAttribute("href") ?? "",
    })),
  );
  expect(links.length).toBeGreaterThanOrEqual(8);

  for (const link of links) {
    await nav.getByRole("link", { name: link.name, exact: true }).click();
    await expect(page, link.name).toHaveURL(
      new RegExp(link.href.replace(/[?]/g, "\\?") + "$"),
    );
    await page.waitForLoadState("networkidle");
    expect(await refusedSoFar(), `refused on ${link.name}`).toEqual([]);
    await expect(
      page.getByText("Not part of the demo"),
      `toast on ${link.name}`,
    ).toHaveCount(0);
  }
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
  // Opening a character asks for the world's lore and the actor's ownership
  // block, and both are answered: nothing on the way in is "not part of the
  // demo".
  await expect(page.getByTestId("actor-ownership-block")).toBeVisible();
  await expect(page.getByTestId("actor-lore-attach-select")).toBeEnabled();
  expect(await refusedSoFar()).toEqual([]);
  await expect(page.getByText("Not part of the demo")).toHaveCount(0);
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

/** Spec 076: what the engine shows of the map at a board point. */
async function sightAt(point: { x: number; y: number }) {
  return page.evaluate(
    ([x, y]) =>
      (
        window as unknown as {
          __engineProbe?: {
            sight?: (
              x: number,
              y: number,
            ) => { looking: boolean; seen: boolean };
          };
        }
      ).__engineProbe?.sight?.(x, y) ?? { looking: false, seen: true },
    [point.x, point.y] as const,
  );
}

test("a wall across the road hides the road beyond it: for the player always, for the Game Master through a selected token", async () => {
  // Spec 076 FR-014. The fighter is wherever the drag and the keyboard left
  // him; the wall goes up a cell and a half east of him, and the road beyond
  // it is the far side.
  const fighter = (await currentScene()).tokens[0];
  const wallX = fighter.x + 117 * 1.5;
  const near = { x: fighter.x + 40, y: fighter.y };
  const far = { x: fighter.x + 117 * 3, y: fighter.y };
  const walls = (await currentScene()).walls.length;
  // The wall tool is still open from the wall before; a click would close it.
  if (!(await page.getByTestId("gm-tool-panel-walls").isVisible())) {
    await page.getByTestId("gm-tool-walls").click();
  }
  await expect(page.getByTestId("gm-tool-panel-walls")).toBeVisible();
  await clickBoard({ x: wallX, y: fighter.y + 200 });
  await page.waitForTimeout(300);
  await clickBoard({ x: wallX, y: fighter.y - 200 });
  await page.waitForTimeout(300);
  await page.keyboard.press("Enter");
  await expect
    .poll(async () => (await currentScene()).walls.length)
    .toBe(walls + 1);
  await page.keyboard.press("Escape");

  // The Game Master: everything until a token is selected.
  await expect.poll(() => sightAt(far)).toEqual({ looking: false, seen: true });
  await clickBoard({ x: fighter.x, y: fighter.y });
  await expect
    .poll(() => sightAt(far), { timeout: 10_000 })
    .toEqual({ looking: true, seen: false });
  expect(await sightAt(near)).toEqual({ looking: true, seen: true });
  // Escape, and the board is the Game Master's again.
  await page.keyboard.press("Escape");
  await expect
    .poll(() => sightAt(far), { timeout: 10_000 })
    .toEqual({ looking: false, seen: true });

  // The player sees through the fighter with nothing selected.
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as a player",
  );
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect
    .poll(() => sightAt(far), { timeout: 30_000 })
    .toEqual({ looking: true, seen: false });
  expect(await sightAt(near)).toEqual({ looking: true, seen: true });
  await page.getByRole("button", { name: "View as Game Master" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as the Game Master",
  );
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
});

/** A press, a move, a release between two board points. */
async function dragBoard(
  from: { x: number; y: number },
  to: { x: number; y: number },
) {
  const a = await toScreen(from);
  const b = await toScreen(to);
  await page.mouse.move(a.x, a.y);
  await page.mouse.down();
  await page.waitForTimeout(120);
  await page.mouse.move(b.x, b.y, { steps: 12 });
  await page.waitForTimeout(120);
  await page.mouse.up();
}

test("a box of walls goes up around the fighter, one edge becomes a door, and the road outside is seen only through it", async () => {
  // Spec 077 FR-024. The fighter stands in the middle of a cell, so its
  // corners are half a cell from him. A Box drag aimed a little inside two
  // corners, two cells apart, is eight walls on the grid (one per cell
  // edge); the Door primitive turns the west edge beside him into a closed
  // door; spec 076's probe says what he sees of the road west of the box
  // through the door closed, then open.
  const scene = await currentScene();
  const fighter = scene.tokens[0];
  const grid = await ask<{ scene: { gridSize: number } }>(
    "query ($sceneId: UUID!) { scene(sceneId: $sceneId) { gridSize } }",
    { sceneId: scene.sceneId },
  );
  const cell = grid.body.data?.scene.gridSize;
  if (!cell) throw new Error("the ambush has a grid");
  const half = cell / 2;
  const west = fighter.x - half - cell;
  const east = fighter.x + half;
  const south = fighter.y - half;
  const north = fighter.y + half + cell;
  const before = scene.walls;

  await page.getByTestId("gm-tool-walls").click();
  await expect(page.getByTestId("gm-tool-panel-walls")).toBeVisible();
  await expect(page.getByTestId("gm-snap-toggle")).toHaveAttribute(
    "data-enabled",
    "true",
  );
  await page.getByTestId("wall-primitive-room").click();
  await dragBoard({ x: west + 9, y: south + 9 }, { x: east - 9, y: north - 9 });
  await expect
    .poll(async () => (await currentScene()).walls.length)
    .toBe(before.length + 8);
  const box = (await currentScene()).walls.filter(
    (w) => !before.some((b) => b.wallId === w.wallId),
  );
  for (const wall of box) {
    expect(Math.hypot(wall.x2 - wall.x1, wall.y2 - wall.y1)).toBeCloseTo(
      cell,
      0,
    );
  }

  // The west edge at the fighter's height becomes a door, creating nothing.
  const edge = box.find(
    (w) =>
      Math.abs(w.x1 - west) < 1 &&
      Math.abs(w.x2 - west) < 1 &&
      Math.min(w.y1, w.y2) < fighter.y &&
      Math.max(w.y1, w.y2) > fighter.y,
  );
  if (!edge) throw new Error("the box has a west edge beside the fighter");
  await page.getByTestId("wall-primitive-door").click();
  await clickBoard({ x: west, y: fighter.y });
  await expect
    .poll(
      async () =>
        (await currentScene()).walls.find((w) => w.wallId === edge.wallId)
          ?.doorState,
    )
    .toBe("CLOSED");
  expect((await currentScene()).walls.length).toBe(before.length + 8);

  // Through the fighter, the road west of the box is behind the closed door.
  await page.keyboard.press("Escape");
  await expect(page.getByTestId("gm-tool-panel-walls")).toBeHidden();
  const road = { x: west - 150, y: fighter.y };
  await clickBoard({ x: fighter.x, y: fighter.y });
  await expect
    .poll(() => sightAt(road), { timeout: 10_000 })
    .toEqual({ looking: true, seen: false });

  // Open the door from its menu, look again: the road is there.
  await clickBoard({ x: west, y: fighter.y }, "right");
  await page.getByRole("menuitem", { name: "Open", exact: true }).click();
  await expect
    .poll(
      async () =>
        (await currentScene()).walls.find((w) => w.wallId === edge.wallId)
          ?.doorState,
    )
    .toBe("OPEN");
  await clickBoard({ x: fighter.x, y: fighter.y });
  await expect
    .poll(() => sightAt(road), { timeout: 10_000 })
    .toEqual({ looking: true, seen: true });
  await page.keyboard.press("Escape");
  await expect
    .poll(() => sightAt(road), { timeout: 10_000 })
    .toEqual({ looking: false, seen: true });
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

/** The icon the board draws on a door, from the engine probe; `null` for none. */
async function doorIconOf(wallId: string): Promise<string | null> {
  return page.evaluate(
    (id) =>
      (
        window as unknown as {
          __engineProbe?: {
            doorIcons?: () => { wallId: string; icon: string }[];
          };
        }
      ).__engineProbe
        ?.doorIcons?.()
        .find((row) => row.wallId === id)?.icon ?? null,
    wallId,
  );
}

/**
 * A board point on the screen wherever the camera is now: a player's board
 * opens on their own token, not on the origin `toScreen` assumes.
 */
async function toScreenNow(point: { x: number; y: number }) {
  const box = await page.locator("canvas").boundingBox();
  if (!box) throw new Error("the board's canvas is not on the page");
  const cam = (await page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe?: {
            camera?: () => { x: number; y: number; scale: number } | null;
          };
        }
      ).__engineProbe?.camera?.() ?? null,
  )) ?? { x: 0, y: 0, scale: 1 };
  const scale = cam.scale > 0 ? cam.scale : 1;
  return {
    x: box.x + box.width / 2 + (point.x - cam.x) / scale,
    y: box.y + box.height / 2 - (point.y - cam.y) / scale,
  };
}

async function pressDoorIcon(point: { x: number; y: number }) {
  const at = await toScreenNow(point);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.waitForTimeout(120);
  await page.mouse.up();
}

async function doorLocked(wallId: string) {
  const { sceneId } = await currentScene();
  const answer = await ask<{ walls: { wallId: string; locked: boolean }[] }>(
    "query ($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId locked } }",
    { sceneId },
  );
  return answer.body.data?.walls.find((w) => w.wallId === wallId)?.locked;
}

test("a door's icon shuts it, and a locked door stays shut for a player whatever they press", async () => {
  // The door the menu just opened, on The Proving Ground.
  const door = (await currentScene()).walls.find(
    (wall) => wall.doorState === "OPEN",
  );
  if (!door) throw new Error("the previous step left a door open");
  const middle = { x: (door.x1 + door.x2) / 2, y: (door.y1 + door.y2) / 2 };
  const stateOf = async () =>
    (await currentScene()).walls.find((wall) => wall.wallId === door.wallId)
      ?.doorState;

  // The Game Master's pointer on it shows close, and pressing that shuts it.
  const at = await toScreenNow(middle);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => doorIconOf(door.wallId)).toBe("close");
  await pressDoorIcon(middle);
  await expect.poll(stateOf).toBe("CLOSED");

  // Locked from its menu, it shows the padlock.
  await clickBoard(middle, "right");
  await page.getByRole("menuitem", { name: "Lock", exact: true }).click();
  await expect.poll(() => doorLocked(door.wallId)).toBe(true);
  await page.mouse.move(at.x, at.y);
  await expect.poll(() => doorIconOf(door.wallId)).toBe("padlock");

  // To a player it is the same padlock, and pressing it changes nothing.
  await page.getByRole("button", { name: "View as player" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as a player",
  );
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect(page.getByTestId("gm-tool-walls")).toHaveCount(0);
  await expect
    .poll(
      async () => {
        const now = await toScreenNow(middle);
        await page.mouse.move(now.x, now.y);
        return doorIconOf(door.wallId);
      },
      { timeout: 30_000 },
    )
    .toBe("padlock");
  await pressDoorIcon(middle);
  await expect(page.getByText("It is locked.")).toBeVisible();
  // And the demo refuses a player's attempt as the server does (FR-010),
  // whatever a page might send it.
  const { sceneId } = await currentScene();
  const interactives = await ask<{
    interactives: {
      interactiveId: string;
      subjectKind: string;
      subjectRef: string;
    }[];
  }>(
    "query ($sceneId: UUID!) { interactives(sceneId: $sceneId) { interactiveId subjectKind subjectRef } }",
    { sceneId },
  );
  const interactive = interactives.body.data?.interactives.find(
    (row) => row.subjectKind === "door" && row.subjectRef === door.wallId,
  );
  if (!interactive) throw new Error("a door is an interactive");
  const tried = await ask<{
    activateInteractive: { outcome: string; reason: string | null };
  }>(
    "mutation ($id: UUID!) { activateInteractive(interactiveId: $id) { outcome reason } }",
    { id: interactive.interactiveId },
  );
  expect(tried.body.data?.activateInteractive).toEqual({
    outcome: "refused",
    reason: "locked",
  });
  expect(await stateOf()).toBe("CLOSED");

  // The Game Master's padlock unlocks it.
  await page.getByRole("button", { name: "View as Game Master" }).click();
  await expect(page.getByTestId("demo-viewer")).toContainText(
    "Viewing as the Game Master",
  );
  await expect(page.getByTestId("gm-tool-walls")).toBeVisible({
    timeout: 60_000,
  });
  await page.getByTestId("gm-tool-select").click();
  await expect
    .poll(
      async () => {
        const now = await toScreenNow(middle);
        await page.mouse.move(now.x, now.y);
        return doorIconOf(door.wallId);
      },
      { timeout: 30_000 },
    )
    .toBe("padlock");
  await pressDoorIcon(middle);
  await expect.poll(() => doorLocked(door.wallId)).toBe(false);
  expect(await stateOf()).toBe("CLOSED");

  // And its open icon opens it again, as the reload below expects to find it.
  await expect.poll(() => doorIconOf(door.wallId)).toBe("open");
  await pressDoorIcon(middle);
  await expect.poll(stateOf).toBe("OPEN");
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
  // Named for the visitor, not for the schema.
  expect(answer.body.errors?.[0]?.message).toBe(
    "Invite links is not part of the demo.",
  );
  await expect(page.getByText("Not part of the demo")).toBeVisible();
  await expect(
    page.getByText("Invite links needs a real ThunderForge instance."),
  ).toBeVisible();
  await expect(page.getByText("generateInviteCode")).toHaveCount(0);

  // The one XMLHttpRequest the client makes, the book importer's upload, is
  // answered as a refusal it can read, not thrown at as it is constructed.
  const upload = await page.evaluate(
    () =>
      new Promise<{ status: number; message: string }>((resolve, reject) => {
        const request = new XMLHttpRequest();
        request.open("POST", "/api/graphql");
        request.onerror = () => reject(new Error("the upload errored"));
        request.onload = () =>
          resolve({
            status: request.status,
            message: JSON.parse(request.responseText).errors[0].message,
          });
        request.send("{}");
      }),
  );
  expect(upload).toEqual({
    status: 404,
    message: "Importing a book is not part of the demo.",
  });
  await expect(
    page.getByText("Importing a book needs a real ThunderForge instance."),
  ).toBeVisible();
});

test("as a player, the heroes are theirs and the ambushers stand on the board unnamed", async () => {
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
  // Every figure is on a player's board, as the server sends it: the
  // monsters are there to be seen, only their names are the GM's.
  const tokens = await ask<{
    tokens: { tokenId: string; name: string | null }[];
  }>("query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId name } }", {
    sceneId: ambush.sceneId,
  });
  const board = tokens.body.data?.tokens ?? [];
  expect(board).toHaveLength(7);
  expect(board.filter((t) => t.name === null)).toHaveLength(5);
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
