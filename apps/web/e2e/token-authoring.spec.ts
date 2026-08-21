import { test, expect, type Page, type Browser } from "@playwright/test";

/**
 * specs/004-token-canvas-authoring, User Story 1 (T008/T009): a GM can
 * reposition an existing token directly on the canvas (no TokenPanel
 * needed to move it), the move persists across reload, and TokenPanel's
 * own displayed position never disagrees with where the canvas last put
 * it (FR-005) — both read/write the same `tokens` row per T006's
 * backing-store unification (ADR-040).
 *
 * Helpers below duplicate (rather than import) canvas-authoring.spec.ts's
 * `registerAndCreateWorld`/`createScene`/`waitForEngineReady`/
 * `canvasBox`/`dragCanvas` and map-editor-tooling.spec.ts's
 * `secondSessionSameLogin` — neither file exports them, and this
 * project's established convention (see map-editor-tooling.spec.ts's
 * top-of-file note) is not to introduce shared test infrastructure
 * beyond what a given feature needs.
 *
 * Cross-session live sync note (matches map-editor-tooling.spec.ts's own
 * documented limitation): specs 003/004/005's research all confirm no
 * live GraphQL subscription transport exists client-side yet (tracked as
 * spec 005). TokenPanel does not subscribe to anything — it re-fetches
 * via a plain `getTokens` query each time it's opened. So "visible to a
 * second connected session" below is verified as "a second session,
 * opening the panel fresh, sees the persisted position" (a real,
 * correct assertion), not "an already-open second session updates
 * without any action" (which would require spec 005's transport and
 * isn't claimed here).
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
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
}

async function registerAndCreateWorld(page: Page, worldName: string): Promise<void> {
  await register(page, freshCredentials("e2etok"));

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+$/, { timeout: 15_000 });

  await page.getByRole("link", { name: "Enter world" }).first().click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

async function createScene(page: Page, name: string): Promise<void> {
  await page.getByTestId("new-scene-button").click();
  await page.getByTestId("new-scene-name-input").fill(name);
  await page.getByTestId("create-scene-submit").click();
  await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
    timeout: 10_000,
  });
  await expect(page.getByTestId("scene-switcher")).toContainText(name);
}

type Box = { x: number; y: number; width: number; height: number };

/** See canvas-authoring.spec.ts's identical helper for the full
 * rationale (Bevy mounts its canvas to `<body>`, not the named
 * container). */
async function canvasBox(page: Page): Promise<Box> {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("Bevy canvas element not found");
  }
  return box;
}

/** See canvas-authoring.spec.ts's identical helper for the full
 * rationale (GM flag / bridge-ready / canvas-focus race). */
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

/** See canvas-authoring.spec.ts's identical helper for the full
 * rationale (real click timing vs. synthetic same-frame down/up). */
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

/** Opens a second, independent browser context reusing the first
 * session's login via `storageState` (same pattern as
 * map-editor-tooling.spec.ts / canvas-authoring.spec.ts's "sync across
 * sessions" tests). Navigates the new page to the same URL as
 * `sourcePage`. */
async function secondSessionSameLogin(
  browser: Browser,
  sourcePage: Page,
): Promise<{ context: Awaited<ReturnType<Browser["newContext"]>>; page: Page }> {
  const sourceContext = sourcePage.context();
  const storageState = await sourceContext.storageState();
  const context = await browser.newContext({ storageState });
  const page = await context.newPage();
  await page.goto(sourcePage.url());
  return { context, page };
}

/** Creates a token via TokenPanel (position defaults to (0,0) — the panel's
 * create dialog has no x/y input, per TokenPanel.tsx's `handleCreateToken`)
 * and leaves the panel closed afterward so the canvas is unobstructed. */
async function createTokenViaPanel(page: Page): Promise<void> {
  await page.getByTestId("token-panel-toggle-button").click({ force: true });
  await page.getByTestId("token-create-trigger").click({ force: true });
  await page.getByTestId("token-create-submit").click({ force: true });
  await expect(page.getByTestId("token-create-trigger")).toBeVisible({
    timeout: 10_000,
  });
  // Close the dialog (Escape) then the panel itself, leaving a clean canvas.
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
}

test.describe("Canvas-native token drag (US1, T008/T009)", () => {
  test.setTimeout(120_000);

  test("GM drags a token on the canvas; the move persists after reload and TokenPanel agrees with the canvas", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Token Drag ${uniqueSuffix()}`);
    await createScene(page, "Token Drag Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);

    // Real, documented gap found while writing this test (see tasks.md
    // T010-note and MVP.md): TokenPanel's `createToken` only performs the
    // GraphQL mutation — it never dispatches into the world store, so a
    // token created via the panel does not appear on canvas until the
    // scene's `loadTokensIntoStore` mount-effect re-runs. A reload forces
    // that re-run, matching the Independent Test's framing of "a scene
    // with an *existing* token" (quickstart.md Scenario 1's prerequisite)
    // rather than "create and immediately drag in the same session,
    // no refresh" — the latter is a real follow-up gap, not this test's
    // premise.
    await page.reload();
    await waitForEngineReady(page);

    // The token was created at (0, 0) — world origin, which the camera
    // centers at canvas-center per every other canvas-authoring test's
    // coordinate convention. Drag it a fixed, easily-asserted offset.
    await dragCanvas(page, { dx: 0, dy: 0 }, { dx: 120, dy: -80 });

    // Give the drop's `upsert_token` → mutation-bridge → `updateToken`
    // round-trip a moment to actually persist server-side before we
    // reload and re-query it.
    await page.waitForTimeout(1_000);

    await page.reload();
    await waitForEngineReady(page);

    await page.getByTestId("token-panel-toggle-button").click({ force: true });
    const tokenItem = page.locator('[data-testid^="token-list-item-"]').first();
    await expect(tokenItem).toBeVisible({ timeout: 10_000 });
    // `force: true`: the token list item's bounding box appears to churn
    // frame-to-frame while the WorldLayout party-roster sidebar is also
    // live-rendering this same token's position, which starves
    // Playwright's default actionability "stable for 2 consecutive
    // frames" check — the element is genuinely visible/clickable
    // (confirmed via screenshots during debugging), so bypass that check
    // rather than fight it.
    await tokenItem.click({ force: true });

    const positionText = page.locator('[data-testid^="token-position-"]').first();
    await expect(positionText).toBeVisible({ timeout: 10_000 });
    const text = await positionText.textContent();
    const match = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(text ?? "");
    expect(match).not.toBeNull();
    const [, xStr, yStr] = match!;
    // Dragged +120 on screen x / -80 on screen y from world origin; canvas
    // y is screen-down while Bevy world-space y is screen-up (matching
    // canvas-authoring.spec.ts's own wall-coordinate assertions), so the
    // persisted world y is the *positive* of the screen-space dy.
    expect(Number(xStr)).toBeCloseTo(120, 0);
    expect(Number(yStr)).toBeCloseTo(80, 0);
  });

  test("a second session opening the scene fresh sees the GM's dragged token position (no live-subscription transport yet — spec 005)", async ({
    page,
    browser,
  }) => {
    await registerAndCreateWorld(page, `E2E Token Cross-Session ${uniqueSuffix()}`);
    await createScene(page, "Token Cross-Session Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    // See the sibling test's comment: a reload is required for a
    // panel-created token to become canvas-rendered/draggable today.
    await page.reload();
    await waitForEngineReady(page);
    // Extra buffer beyond waitForEngineReady's own wait: a real race is
    // possible here where the drag fires before the reloaded scene's
    // token entity has actually been spawned by apply_external_commands,
    // silently dragging nothing and leaving the token at its original
    // (0, 0).
    await page.waitForTimeout(1_500);

    // Deliberately drags into negative-y territory: a real check
    // constraint (`tokens_valid_coordinates`, `x >= 0 AND y >= 0`) used to
    // reject this silently as "not found or not owned by you" (the
    // mutation's blanket error-mapping swallowed the real Postgres check
    // violation) — found live while writing this test. Fixed via the
    // `drop_token_valid_coordinates_check` migration so tokens can use the
    // same center-origin coordinate system walls/shapes/lights already do;
    // this offset is kept as regression coverage for that fix.
    await dragCanvas(page, { dx: 0, dy: 0 }, { dx: 60, dy: 40 });
    await page.waitForTimeout(1_000);

    // Confirm the drag actually persisted on the GM's own page before
    // involving a second session at all — isolates "did the drag work"
    // from "did the second session read stale/fresh data".
    await page.getByTestId("token-panel-toggle-button").click({ force: true });
    const gmTokenItem = page.locator('[data-testid^="token-list-item-"]').first();
    await expect(gmTokenItem).toBeVisible({ timeout: 10_000 });
    await gmTokenItem.click({ force: true });
    const gmPositionText = page.locator('[data-testid^="token-position-"]').first();
    await expect(gmPositionText).toBeVisible({ timeout: 10_000 });
    const gmText = await gmPositionText.textContent();
    const gmMatch = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(gmText ?? "");
    expect(gmMatch).not.toBeNull();
    expect(Number(gmMatch![1])).toBeCloseTo(60, 0);
    expect(Number(gmMatch![2])).toBeCloseTo(-40, 0);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");

    const { context: secondContext, page: secondPage } = await secondSessionSameLogin(
      browser,
      page,
    );
    try {
      await waitForEngineReady(secondPage);
      await secondPage.getByTestId("token-panel-toggle-button").click({ force: true });
      const tokenItem = secondPage
        .locator('[data-testid^="token-list-item-"]')
        .first();
      await expect(tokenItem).toBeVisible({ timeout: 10_000 });
      // `force: true`: the token list item's bounding box appears to churn
    // frame-to-frame while the WorldLayout party-roster sidebar is also
    // live-rendering this same token's position, which starves
    // Playwright's default actionability "stable for 2 consecutive
    // frames" check — the element is genuinely visible/clickable
    // (confirmed via screenshots during debugging), so bypass that check
    // rather than fight it.
    await tokenItem.click({ force: true });

      const positionText = secondPage
        .locator('[data-testid^="token-position-"]')
        .first();
      await expect(positionText).toBeVisible({ timeout: 10_000 });
      const text = await positionText.textContent();
      const match = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(text ?? "");
      expect(match).not.toBeNull();
      const [, xStr, yStr] = match!;
      expect(Number(xStr)).toBeCloseTo(60, 0);
      expect(Number(yStr)).toBeCloseTo(-40, 0);
    } finally {
      await secondContext.close();
    }
  });
});

/**
 * User Story 2 (T013/T014): GM resizes a selected token in whole
 * grid-cell increments and rotates its facing independently.
 *
 * Interim mechanism note (see `selection.rs`'s
 * `handle_token_resize_rotate_keyboard` doc comment for the full
 * rationale): resize/rotate are keyboard shortcuts on the selected token
 * (`]`/`[` to grow/shrink by one grid cell, `,`/`.` to rotate by a fixed
 * step) rather than literal canvas-rendered drag handles — a real
 * engineering-time tradeoff made while implementing this feature live,
 * not what research.md originally planned. Functionally this still
 * satisfies FR-006/FR-007 (GM-only, grid-snapped resize, independent
 * rotate, persisted, synced) — a follow-up should replace the mechanism
 * with actual draggable handle sprites for interaction-affordance parity
 * with walls/shapes, without changing the underlying `scale`/`rotation`
 * data path this session wired end-to-end (WorldTokenPayload ->
 * apply_external_commands -> Transform -> upsert_token ->
 * startTokenMutationBridge -> updateToken).
 */
test.describe("Token resize/rotate (US2, T013/T014)", () => {
  test.setTimeout(90_000);

  test("GM resizes a selected token in whole grid-cell increments, independent of rotation, and both persist", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Token Resize ${uniqueSuffix()}`);
    await createScene(page, "Token Resize Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);

    // Select the token by clicking it (world origin, per every other test
    // in this file), then resize it twice (+2 grid cells) and rotate it
    // twice, independently.
    const box = await canvasBox(page);
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    // Real press+release with a delay, not a synthetic same-frame
    // `.click()` — see this file's `dragCanvas`/canvas-authoring.spec.ts's
    // `clickCanvasAt` for the identical rationale (frame-collapsing risk).
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await page.waitForTimeout(300);

    await page.keyboard.press("BracketRight");
    await page.waitForTimeout(150);
    await page.keyboard.press("BracketRight");
    await page.waitForTimeout(150);
    await page.keyboard.press("Comma");
    await page.waitForTimeout(150);
    await page.keyboard.press("Comma");
    await page.waitForTimeout(1_000);

    await page.reload();
    await waitForEngineReady(page);

    // TokenPanel doesn't display scale/rotation directly today (it shows
    // position only, per T029's scope) — assert via the same `tokens
    // (sceneId)` GraphQL query `getTokens` already uses elsewhere in this
    // file, the "verify the actual persisted row" standard spec 003's
    // round-trip tests established. sceneId comes straight off the URL,
    // same as every other scene-scoped call in this app.
    const sceneId = /[?&]sceneId=([^&]+)/.exec(page.url())?.[1];
    const tokens: { x: number; y: number; rotation: number; scale: number }[] =
      await page.evaluate(async (sceneIdArg: string | undefined) => {
        // sceneId isn't actually in the URL (see SceneSwitcher's
        // client-only selectedSceneId state) — read it off the
        // TokenPanel toggle button's nearest scene context isn't
        // available either, so query all tokens for every scene this
        // world has via the scenes list, then flatten. Simpler: the
        // world/play page only ever has this test's one scene, so query
        // tokens for it directly via the world's scene list.
        // POST requests to a session-authenticated endpoint require the
        // `x-csrf-token` header matching the `csrf_token` cookie
        // (auth_middleware.rs's `require_csrf_for_session`) — mirroring
        // `apps/web/src/api/auth.ts`'s `withCsrf` helper, duplicated here
        // since this raw `fetch` runs inside `page.evaluate`'s browser
        // context, not this file's Node-side helpers.
        const csrfToken =
          document.cookie
            .split("; ")
            .find((row) => row.startsWith("csrf_token="))
            ?.split("=")[1] ?? "";
        const headers = {
          "Content-Type": "application/json",
          "x-csrf-token": csrfToken,
        };

        const worldMatch = /\/world\/([^/]+)\/play/.exec(window.location.pathname);
        const worldId = worldMatch?.[1];
        const scenesResponse = await fetch("/api/graphql", {
          method: "POST",
          headers,
          credentials: "same-origin",
          body: JSON.stringify({
            query: `query($worldId: UUID!) { scenes(worldId: $worldId) { sceneId } }`,
            variables: { worldId },
          }),
        });
        const scenesJson = await scenesResponse.json();
        const resolvedSceneId = sceneIdArg ?? scenesJson.data?.scenes?.[0]?.sceneId;
        const tokensResponse = await fetch("/api/graphql", {
          method: "POST",
          headers,
          credentials: "same-origin",
          body: JSON.stringify({
            query: `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { x y rotation scale } }`,
            variables: { sceneId: resolvedSceneId },
          }),
        });
        const tokensJson = await tokensResponse.json();
        return tokensJson.data?.tokens ?? [];
      }, sceneId);

    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens[0].scale).toBeCloseTo(3.0, 5); // 1.0 default + 2 increments
    // Two 30-degree steps = 60 degrees = PI/3 radians.
    expect(tokens[0].rotation).toBeCloseTo(Math.PI / 3, 2);
  });
});
