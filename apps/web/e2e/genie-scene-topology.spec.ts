import { test, expect, type Page } from "@playwright/test";

/**
 * specs/018-genie-house-system, User Story 2 (Scenario 2): a GM switches
 * a scene between a measured Material grid and an abstract, gridless
 * Wish-Warped Zone, and tokens in each keep independent, uncorrupted
 * position data.
 *
 * Real gap found while building this: `SceneSwitcher`'s "New scene"
 * dialog (apps/web/src/components/world/SceneSwitcher/SceneSwitcher.tsx)
 * only has a name field — there is no UI to choose a scene's `gridType`
 * at creation, even though `createScene`'s GraphQL mutation and the
 * `scenes` table both support it (the `2026-08-23-195654` migration
 * widened the grid_type CHECK constraint to allow "gridless" for exactly
 * this feature). So creating a genuinely gridless scene is done here via
 * a direct `createScene` GraphQL call through the live authenticated
 * session (same pattern dice-roll.spec.ts uses for read-side
 * verification) rather than through a UI affordance that doesn't exist.
 */

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

interface Credentials {
  username: string;
  email: string;
  password: string;
}

function freshCredentials(prefix: string): Credentials {
  const suffix = uniqueSuffix();
  const username = `${prefix}${suffix}`;
  return {
    username,
    email: `${username}@example.test`,
    password: "Sup3r-Secret-Passphrase!",
  };
}

async function register(page: Page, creds: Credentials): Promise<void> {
  await page.goto("/register");
  await page.locator("#register-username").fill(creds.username);
  await page.locator("#register-email").fill(creds.email);
  await page.locator("#register-password").fill(creds.password);
  await page.locator("#register-password-confirmation").fill(creds.password);
  await page.getByRole("button", { name: "Create account" }).click();
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2egtopo"));
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)\/staging$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }
  return match[1];
}

async function graphql<T>(page: Page, query: string, variables: Record<string, unknown>): Promise<T> {
  return page.evaluate(
    async ({ query, variables }) => {
      const csrfToken = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);
      const res = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
        },
        body: JSON.stringify({ query, variables }),
      });
      const text = await res.text();
      try {
        return JSON.parse(text);
      } catch {
        throw new Error(`Non-JSON response (status ${res.status}): ${text.slice(0, 500)}`);
      }
    },
    { query, variables },
  );
}

/** The Play dock's Settings section (scene switcher included) is collapsed
 * by default — open it if it isn't already visible. */
async function ensureSidebarOpen(page: Page): Promise<void> {
  const switcher = page.getByTestId("scene-switcher");
  if (await switcher.isVisible().catch(() => false)) {
    return;
  }
  // Deselect any canvas-selected token first — a selected token's own
  // "SELECTED TOKEN" panel can otherwise steal focus/clicks from the
  // toggle button underneath a stray force-click.
  await page.keyboard.press("Escape");
  await page.waitForTimeout(200);
  // Dispatched rather than clicked. This dates from a real layering bug:
  // the old bottom-left "Tools" toggle sat under the dice-roller panel's
  // text input, and even a `force: true` click (which still routes through
  // browser hit-testing) landed on the input instead. The dock's icon rail
  // no longer overlaps the dice roller, but a synthetic event is still the
  // most robust way to reach the handler from this helper regardless of
  // what else the canvas has floating over it.
  await page.getByTestId("world-dock-tab-settings").dispatchEvent("click");
  await expect(switcher).toBeVisible({ timeout: 10_000 });
}

async function clickPlay(page: Page): Promise<void> {
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

async function waitForEngineReady(page: Page): Promise<void> {
  const canvas = page.locator("canvas");
  await expect(canvas).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(3_000);
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (box) {
    await page.mouse.click(box.x + box.width - 40, box.y + box.height - 40);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }
}

type Box = { x: number; y: number; width: number; height: number };

async function canvasBox(page: Page): Promise<Box> {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("Bevy canvas element not found");
  return box;
}

async function createTokenViaPanel(page: Page): Promise<void> {
  await page.getByTestId("token-panel-toggle-button").click({ force: true });
  await page.getByTestId("token-create-trigger").click({ force: true });
  await page.getByTestId("token-create-submit").click({ force: true });
  await expect(page.getByTestId("token-create-trigger")).toBeVisible({ timeout: 10_000 });
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
}

async function dragCanvas(
  page: Page,
  from: { dx: number; dy: number },
  to: { dx: number; dy: number },
): Promise<void> {
  const box = await canvasBox(page);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx + from.dx, cy + from.dy);
  await page.mouse.down();
  await page.mouse.move(cx + to.dx, cy + to.dy, { steps: 5 });
  await page.waitForTimeout(80);
  await page.mouse.up();
}

/** Every panel-created token starts at the canvas world-origin (screen
 * center). Drags it from there to a fixed offset — matches
 * token-authoring.spec.ts's identical helper. */
async function dragTokenAtOriginTo(page: Page, offset: { dx: number; dy: number }): Promise<void> {
  await dragCanvas(page, { dx: 0, dy: 0 }, offset);
  await page.waitForTimeout(1_000);
}

test.describe("Spec 018 Scenario 2: a GM switches a scene between Material (grid) and Wish-Warped Zone (gridless)", () => {
  test("a Material scene and a gridless Wish-Warped Zone scene each hold independent token positions across a scene switch", async ({
    page,
  }) => {
    test.setTimeout(120_000);
    const worldName = `E2E Genie Topology ${uniqueSuffix()}`;
    const worldId = await registerAndCreateWorld(page, worldName);

    // The world's auto-created default scene is Material (gridType:
    // "square", per create_world_impl's inlined default) — use it as-is
    // for the Material half of this scenario.
    await clickPlay(page);
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    // token-authoring.spec.ts's documented gap: a panel-created token
    // doesn't appear in the world store (and so isn't draggable on
    // canvas) until the scene's mount-effect re-runs, which a reload
    // forces.
    await page.reload();
    await waitForEngineReady(page);
    await dragTokenAtOriginTo(page, { dx: 120, dy: 40 });
    await page.waitForTimeout(1_000);

    const scenesBefore = await graphql<{ data: { scenes: { sceneId: string; gridType: string }[] } }>(
      page,
      `query($worldId: UUID!) { scenes(worldId: $worldId) { sceneId gridType } }`,
      { worldId },
    );
    const materialScene = scenesBefore.data.scenes[0];
    expect(materialScene.gridType).toBe("square");

    const materialTokens = await graphql<{ data: { tokens: { x: number; y: number }[] } }>(
      page,
      `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { x y } }`,
      { sceneId: materialScene.sceneId },
    );
    expect(materialTokens.data.tokens.length).toBe(1);
    const materialTokenPos = materialTokens.data.tokens[0];
    // Moved away from the origin the token was created at.
    expect(Math.abs(materialTokenPos.x) + Math.abs(materialTokenPos.y)).toBeGreaterThan(0);

    // Create a genuinely gridless Wish-Warped Zone scene directly via the
    // real createScene GraphQL mutation (no UI affordance exists to pick
    // gridType — see file header note).
    const createGridless = await graphql<{
      data: { createScene: { sceneId: string; gridType: string; name: string } };
    }>(
      page,
      `mutation($input: GraphQLCreateSceneInput!) { createScene(input: $input) { sceneId gridType name } }`,
      { input: { worldId, name: "Wish-Warped Zone", gridType: "gridless" } },
    );
    const gridlessScene = createGridless.data.createScene;
    expect(gridlessScene.gridType).toBe("gridless");

    // WorldPage's scene list is only fetched once on mount, so a scene
    // created via a raw GraphQL call (no UI exists to set gridType — see
    // file header) never appears in the real SceneSwitcher's options
    // until the page reloads and re-fetches — the same class of gap
    // token-authoring.spec.ts documents for panel-created tokens.
    await page.reload();
    await waitForEngineReady(page);

    // Switch the play canvas to the new gridless scene via the real
    // scene switcher UI.
    await ensureSidebarOpen(page);
    await page.getByTestId("scene-switcher").click();
    await page.getByRole("option", { name: gridlessScene.name }).click();
    await page.waitForTimeout(1_000);

    await createTokenViaPanel(page);
    await page.reload();
    // A reload navigates back to /play with no scene selected by
    // default, so re-select the gridless scene before waiting for the
    // canvas and dragging.
    await waitForEngineReady(page);
    await ensureSidebarOpen(page);
    await page.getByTestId("scene-switcher").click();
    await page.getByRole("option", { name: gridlessScene.name }).click();
    await page.waitForTimeout(1_000);
    await dragTokenAtOriginTo(page, { dx: -80, dy: 60 });
    await page.waitForTimeout(1_000);

    const gridlessTokens = await graphql<{ data: { tokens: { x: number; y: number }[] } }>(
      page,
      `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { x y } }`,
      { sceneId: gridlessScene.sceneId },
    );
    expect(gridlessTokens.data.tokens.length).toBe(1);
    const gridlessTokenPos = gridlessTokens.data.tokens[0];
    expect(Math.abs(gridlessTokenPos.x) + Math.abs(gridlessTokenPos.y)).toBeGreaterThan(0);

    await page.screenshot({
      path: "e2e/screenshots/genie-wish-warped-zone-gridless-scene.png",
      fullPage: false,
    });

    // Switch back to the Material scene and confirm its token's position
    // is exactly what we left it at — no corruption/cross-contamination
    // from having visited the gridless scene in between (Edge Cases).
    await ensureSidebarOpen(page);
    await page.getByTestId("scene-switcher").click();
    await page.getByRole("option", { name: worldName }).click();
    await page.waitForTimeout(1_000);

    const materialTokensAfter = await graphql<{ data: { tokens: { x: number; y: number }[] } }>(
      page,
      `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { x y } }`,
      { sceneId: materialScene.sceneId },
    );
    expect(materialTokensAfter.data.tokens.length).toBe(1);
    expect(materialTokensAfter.data.tokens[0].x).toBeCloseTo(materialTokenPos.x, 5);
    expect(materialTokensAfter.data.tokens[0].y).toBeCloseTo(materialTokenPos.y, 5);

    // And the gridless scene's token is still exactly where it was too.
    const gridlessTokensAfter = await graphql<{ data: { tokens: { x: number; y: number }[] } }>(
      page,
      `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { x y } }`,
      { sceneId: gridlessScene.sceneId },
    );
    expect(gridlessTokensAfter.data.tokens[0].x).toBeCloseTo(gridlessTokenPos.x, 5);
    expect(gridlessTokensAfter.data.tokens[0].y).toBeCloseTo(gridlessTokenPos.y, 5);
  });
});
