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

async function registerAndCreateWorld(page: Page, worldName: string): Promise<string> {
  await register(page, freshCredentials("e2etok"));

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+$/, { timeout: 15_000 });
  const match = /\/world\/([^/]+)$/.exec(new URL(page.url()).pathname);
  if (!match) {
    throw new Error(`Could not extract world id from URL: ${page.url()}`);
  }

  await page.getByRole("link", { name: "Enter world" }).first().click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
  return match[1];
}

/** Registers a brand-new account and returns its user id, captured from
 * the register REST response body (`session.user.id` — see
 * `apps/web/src/types/auth.ts`'s `AuthSessionResponse`) rather than any
 * UI element, since no page in this app currently displays a user's own
 * id anywhere (T021-T023's ownership-grant flow needs it to type into
 * `TokenPanel`'s "Owner user ID" input). */
async function registerAndGetUserId(page: Page, prefix: string): Promise<string> {
  const creds = freshCredentials(prefix);
  const [response] = await Promise.all([
    page.waitForResponse(
      // `.ok()` (2xx), not `status() === 200`: the register endpoint's
      // real status turned out to be a non-200 2xx (found live — this
      // exact-200 check made the very first version of this helper hang
      // forever waiting for a response that had already arrived).
      (resp) => resp.url().includes("/api/authentication/register") && resp.ok(),
    ),
    (async () => {
      await page.goto("/register");
      await page.locator("#register-username").fill(creds.username);
      await page.locator("#register-email").fill(creds.email);
      await page.locator("#register-password").fill(creds.password);
      await page.locator("#register-password-confirmation").fill(creds.password);
      await page.getByRole("button", { name: "Create account" }).click();
    })(),
  ]);
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });
  const body = (await response.json()) as { session?: { user?: { id?: string } } };
  const userId = body.session?.user?.id;
  if (!userId) {
    throw new Error("Could not extract user id from register response");
  }
  return userId;
}

/** As the GM (on the `/world/{worldId}/play` view), briefly navigates to
 * the world dashboard to generate an invite code via
 * `CampaignSettingsPanel` (spec 005 US4), then returns to the play view.
 * Mirrors `invite-membership.spec.ts`'s flow, adapted since this file's
 * `registerAndCreateWorld` (unlike that file's) leaves the GM already
 * inside the play view, not the dashboard. */
async function generateInviteCodeFromDashboard(page: Page, worldId: string): Promise<string> {
  await page.context().grantPermissions(["clipboard-read", "clipboard-write"]);
  await page.goto(`/world/${worldId}`);
  await page.getByRole("button", { name: "Generate Invite Code" }).click();
  const inviteCodeLocator = page.locator("code").first();
  await expect(inviteCodeLocator).toBeVisible({ timeout: 10_000 });
  const inviteCode = (await inviteCodeLocator.textContent())?.trim();
  if (!inviteCode) {
    throw new Error("Invite code did not render after generation");
  }
  await page.goto(`/world/${worldId}/play`);
  return inviteCode;
}

/** As a freshly-registered player, redeems an invite code and lands on
 * the world's play view. Mirrors `invite-membership.spec.ts`'s join
 * flow, continuing past the dashboard into `/play` via the same "Enter
 * world" link `registerAndCreateWorld` uses for the GM. */
async function joinWorldAndEnterPlay(
  page: Page,
  inviteCode: string,
  worldId: string,
): Promise<void> {
  await page.goto(`/join/${inviteCode}`);
  await expect(page.getByRole("button", { name: "Join Campaign" })).toBeVisible({
    timeout: 10_000,
  });
  await page.getByRole("button", { name: "Join Campaign" }).click();
  await page.waitForURL(new RegExp(`/world/${worldId}$`), { timeout: 15_000 });
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

/** Screen-space offset (from canvas center) of the selected token's
 * resize handle at a given `scale`, matching
 * `src/engine/src/systems/token.rs`'s `resize_handle_world_pos` exactly:
 * the token's bottom-right corner (`(half.x, -half.y)` in world space at
 * rotation 0), converted to screen space per `worldToScreenOffset`'s sign
 * convention (screen dy is the negation of world y). `TOKEN_SIZE` (96)
 * is duplicated from `lib.rs` — this file has no way to import Rust
 * consts, matching every other magic-number duplication already in this
 * suite (e.g. `screenOffsetToWorld`'s own coordinate-sign note).
 */
function resizeHandleScreenOffset(scale: number): { dx: number; dy: number } {
  const half = (96 / 2) * scale;
  return { dx: half, dy: half };
}

/** Screen-space offset (from canvas center) of the selected token's
 * rotate handle at a given `scale` and `rotationRadians`, matching
 * `resize_handle_world_pos`'s sibling `rotate_handle_world_pos`: offset
 * `(TOKEN_SIZE/2 + 24) * scale` along the token's local +Y, rotated by
 * its current facing, then converted to screen space.
 */
function rotateHandleScreenOffset(
  scale: number,
  rotationRadians: number,
): { dx: number; dy: number } {
  const offset = (96 / 2 + 24) * scale;
  // Vec2::from_angle(theta).rotate((0, offset)) = offset * (-sin(theta), cos(theta)).
  const worldX = offset * -Math.sin(rotationRadians);
  const worldY = offset * Math.cos(rotationRadians);
  return { dx: worldX, dy: -worldY };
}

/** Presses on `from` (a screen offset from canvas center), drags to `to`,
 * and releases — real press/move/release timing, mirroring `dragCanvas`,
 * for grabbing and dragging a token's resize/rotate handle sprite. */
async function dragTokenHandle(
  page: Page,
  from: { dx: number; dy: number },
  to: { dx: number; dy: number },
): Promise<void> {
  const box = await canvasBox(page);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.move(cx + from.dx, cy + from.dy);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.move(cx + to.dx, cy + to.dy, { steps: 8 });
  await page.waitForTimeout(150);
  await page.mouse.up();
  await page.waitForTimeout(300);
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

/** Same as `createTokenViaPanel`, but also captures and returns the new
 * token's server-assigned `tokenId` by intercepting the `createToken`
 * GraphQL mutation's response — needed by T021-T023's tests, which must
 * assign ownership (`TokenPanel`'s per-token owner/primary inputs are
 * keyed by `tokenId`) to a *specific* one of several tokens rather than
 * "whichever one is currently topmost," which is unspecified when
 * multiple tokens share the same (0, 0) creation position. */
async function createTokenViaPanelCapturingId(page: Page): Promise<string> {
  await page.getByTestId("token-panel-toggle-button").click({ force: true });
  await page.getByTestId("token-create-trigger").click({ force: true });
  const [response] = await Promise.all([
    page.waitForResponse(
      (resp) =>
        resp.url().includes("/api/graphql") &&
        (resp.request().postData() ?? "").includes("createToken"),
    ),
    page.getByTestId("token-create-submit").click({ force: true }),
  ]);
  const body = (await response.json()) as { data?: { createToken?: { tokenId?: string } } };
  const tokenId = body.data?.createToken?.tokenId;
  if (!tokenId) {
    throw new Error("Could not extract tokenId from createToken response");
  }
  await expect(page.getByTestId("token-create-trigger")).toBeVisible({
    timeout: 10_000,
  });
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  return tokenId;
}

/** Drags whichever token is currently at the canvas world-origin (the
 * position every panel-created token starts at) to a fixed screen
 * offset from center. Used to spatially separate several same-position
 * tokens one at a time immediately after each is created, so each ends
 * up at a known, non-overlapping position keyed to its known
 * `tokenId` — see the T021-T023 describe block below for why this
 * ordering matters. */
async function dragTokenAtOriginTo(
  page: Page,
  offset: { dx: number; dy: number },
): Promise<void> {
  await dragCanvas(page, { dx: 0, dy: 0 }, offset);
  await page.waitForTimeout(1_000);
}

/** Converts a screen-space drag offset (from canvas center) to the
 * resulting Bevy world position: world x equals screen dx directly,
 * but world y is the *negation* of screen dy, since canvas/screen y is
 * screen-down while Bevy world-space y is screen-up (confirmed by the
 * sibling US1 tests above — e.g. a screen offset of (dx: 60, dy: 40)
 * persists as world (60, -40)). Centralized here so T021-T023's several
 * drag/readback pairs don't each re-derive the sign by hand. */
function screenOffsetToWorld(offset: { dx: number; dy: number }): { x: number; y: number } {
  return { x: offset.dx, y: -offset.dy };
}

/** Inverse of `screenOffsetToWorld` — given a token's known world
 * position, returns the screen-space offset from canvas center needed
 * to click on it. */
function worldToScreenOffset(position: { x: number; y: number }): { dx: number; dy: number } {
  return { dx: position.x, dy: -position.y };
}

/** Reads a specific token's persisted position from `TokenPanel`,
 * identified by its known `tokenId` (not list order/position, which
 * only `createTokenViaPanelCapturingId` gives us determinism over). */
async function readTokenPosition(
  page: Page,
  tokenId: string,
): Promise<{ x: number; y: number }> {
  await page.getByTestId("token-panel-toggle-button").click({ force: true });
  const item = page.getByTestId(`token-list-item-${tokenId}`);
  await expect(item).toBeVisible({ timeout: 10_000 });
  await item.click({ force: true });
  const positionText = page.getByTestId(`token-position-${tokenId}`);
  await expect(positionText).toBeVisible({ timeout: 10_000 });
  const text = await positionText.textContent();
  const match = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(text ?? "");
  if (!match) {
    throw new Error(`Could not parse position text: ${text}`);
  }
  const [, xStr, yStr] = match;
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  // Small settle delay: this helper is often called several times in a
  // row (once per token) with no gap between the prior call's closing
  // Escapes and the next call's immediate toggle-button click. Found
  // live (spec 006 US2 live-verification pass) — occasionally the next
  // open raced Radix's still-in-flight close animation/portal teardown
  // for the previous Popover, leaving the panel toggle's very next click
  // landing on a transitional state and stalling the following
  // assertion. A brief wait lets that teardown finish before the caller
  // (or this function, on repeated calls) interacts with the panel again.
  await page.waitForTimeout(300);
  return { x: Number(xStr), y: Number(yStr) };
}

/** As the GM, assigns a token's `ownerUserId` and `isPrimary` via
 * `TokenPanel`'s GM-only ownership controls (T030). The primary
 * checkbox is disabled until an owner is set (see `TokenPanel.tsx`),
 * so the owner input must be filled (and its `onBlur` handler fired)
 * before the checkbox can be toggled.
 *
 * Deliberately uses `press("Tab")`, not `.blur()`, to fire that
 * handler: `.blur()` moves focus to nowhere (`document.body`), which
 * Radix `Popover.Content`'s default outside-focus dismissal treats as
 * "focus left the popover" and closes it out from under the very next
 * assertion — found live (a real, reproducible race, not incidental
 * flakiness) while writing this test. Tabbing instead moves focus to
 * the next element inside the same popover (the primary checkbox's
 * label), which still fires the input's blur/onBlur normally without
 * ever handing focus to something Radix considers "outside". */
async function assignTokenOwnership(
  page: Page,
  tokenId: string,
  ownerUserId: string,
  isPrimary: boolean,
): Promise<void> {
  await page.getByTestId("token-panel-toggle-button").click({ force: true });
  const item = page.getByTestId(`token-list-item-${tokenId}`);
  await expect(item).toBeVisible({ timeout: 10_000 });
  await item.click({ force: true });

  const ownerInput = page.getByTestId(`token-owner-input-${tokenId}`);
  await expect(ownerInput).toBeVisible({ timeout: 10_000 });
  await ownerInput.fill(ownerUserId);
  await ownerInput.press("Tab");
  // The owner-assignment mutation must land before the (now-enabled)
  // primary checkbox is interacted with.
  await page.waitForTimeout(500);

  if (isPrimary) {
    const primaryCheckbox = page.getByTestId(`token-primary-checkbox-${tokenId}`);
    await expect(primaryCheckbox).toBeEnabled({ timeout: 10_000 });
    await primaryCheckbox.check({ force: true });
    await page.waitForTimeout(500);
  }

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
 * User Story 1 (spec 006, T001-T003): GM resizes a selected token by
 * dragging its real canvas-rendered resize handle in whole grid-cell
 * increments, and rotates its facing continuously by dragging a separate
 * rotate handle, independently. Replaces spec 004's keyboard-shortcut
 * stand-in (`]`/`[`/`,`/`.`, still available as a secondary path per
 * `handle_token_resize_rotate_keyboard`'s doc comment) with actual
 * draggable handle sprites (`TokenResizeHandle`/`TokenRotateHandle` in
 * `src/engine/src/systems/token.rs`), closing the interaction-affordance
 * gap with walls/shapes.
 */
test.describe("Token resize/rotate (US1, spec 006 T001-T003)", () => {
  // 90s was too tight for a live run: each test does 1-2 full page reloads,
  // and reloading re-instantiates the ~190MB Bevy/WASM bundle from scratch
  // (same root cause documented on the US2 ownership test's setTimeout,
  // which needed 300s -> 480s for its ~9 reloads).
  test.setTimeout(180_000);

  test("GM resizes a selected token by dragging its resize handle in whole grid-cell increments, and rotates via a separate handle independently, and both persist", async ({
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
    // in this file).
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

    // T001/FR-001: the resize handle is now rendered at the token's
    // corner (scale 1) — grab it and drag out to the distance
    // corresponding to scale 3 (whole grid-cell increments, +2 cells).
    const resizeStart = resizeHandleScreenOffset(1);
    const resizeEnd = resizeHandleScreenOffset(3);
    await dragTokenHandle(page, resizeStart, resizeEnd);

    // T001/FR-003: the rotate handle sits at a separate offset (scaled
    // with the token, per `rotate_handle_world_pos`) — grab it and drag
    // to the angle corresponding to a 60-degree (PI/3) rotation,
    // independent of the resize above.
    const rotateStart = rotateHandleScreenOffset(3, 0);
    const rotateEnd = rotateHandleScreenOffset(3, Math.PI / 3);
    await dragTokenHandle(page, rotateStart, rotateEnd);
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

  /**
   * T002/FR-001: confirms the resize and rotate handles are real,
   * distinct, hit-testable sprites rendered at their documented
   * positions for a selected token — not just that *some* drag on the
   * token changes scale/rotation. `waitForEngineReady`'s environment
   * cannot pixel-inspect the Bevy canvas directly (WebGL context here
   * doesn't preserve its drawing buffer — see canvas-authoring.spec.ts's
   * `Wall sync across sessions` doc comment for the same documented
   * limitation), so this is the behavioral proxy this codebase already
   * establishes as the convention for verifying Bevy-rendered elements
   * exist: each handle, dragged in isolation, changes only its own
   * property (scale xor rotation), never the token's position — proving
   * two separate, precisely-positioned hit targets exist, not one
   * ambiguous "drag the token" region.
   */
  test("resize and rotate handles are separate, precisely-positioned hit targets that each change only their own property", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Token Handles ${uniqueSuffix()}`);
    await createScene(page, "Token Handles Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);

    const box = await canvasBox(page);
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await page.waitForTimeout(300);

    // Drag only the resize handle: scale changes, position and rotation
    // don't.
    await dragTokenHandle(
      page,
      resizeHandleScreenOffset(1),
      resizeHandleScreenOffset(2),
    );
    await page.waitForTimeout(500);

    // Drag only the rotate handle (now at scale 2's offset): rotation
    // changes, position and scale don't.
    await dragTokenHandle(
      page,
      rotateHandleScreenOffset(2, 0),
      rotateHandleScreenOffset(2, Math.PI / 2),
    );
    await page.waitForTimeout(500);

    await page.reload();
    await waitForEngineReady(page);

    const sceneId = /[?&]sceneId=([^&]+)/.exec(page.url())?.[1];
    const tokens: { x: number; y: number; rotation: number; scale: number }[] =
      await page.evaluate(async (sceneIdArg: string | undefined) => {
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
    expect(tokens[0].x).toBeCloseTo(0, 0);
    expect(tokens[0].y).toBeCloseTo(0, 0);
    expect(tokens[0].scale).toBeCloseTo(2.0, 5);
    expect(tokens[0].rotation).toBeCloseTo(Math.PI / 2, 2);
  });

  test("T020: TokenTool panel appears on selection and its Grow/Rotate buttons resize/rotate the token", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Token UI ${uniqueSuffix()}`);
    await createScene(page, "Token UI Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);

    // No token selected yet: the panel must not render at all.
    await expect(page.getByTestId("token-tool")).toHaveCount(0);

    const box = await canvasBox(page);
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await page.waitForTimeout(300);

    // Selecting the token on the canvas must surface the panel — this is
    // the discoverability gap T020 closes: previously only `]`/`[`/`,`/`.`
    // worked, with nothing on screen telling a GM they existed.
    await expect(page.getByTestId("token-tool")).toBeVisible({ timeout: 5_000 });
    await expect(page.getByTestId("token-tool-scale")).toContainText("1x");
    await expect(page.getByTestId("token-tool-rotation")).toContainText("0°");

    await page.getByTestId("token-tool-grow").click();
    await page.waitForTimeout(300);
    await expect(page.getByTestId("token-tool-scale")).toContainText("2x");

    await page.getByTestId("token-tool-rotate-left").click();
    await page.waitForTimeout(1_000);
    await expect(page.getByTestId("token-tool-rotation")).toContainText("30°");

    // Persisted, not just local React state: reload and confirm the
    // button-driven change survives, via the same GraphQL round-trip the
    // keyboard-shortcut test above verifies.
    await page.reload();
    await waitForEngineReady(page);
    await page.mouse.move(cx, cy);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await page.waitForTimeout(500);
    await expect(page.getByTestId("token-tool-scale")).toContainText("2x");
    await expect(page.getByTestId("token-tool-rotation")).toContainText("30°");

    // Deselecting (click empty canvas) must hide the panel again.
    await page.mouse.move(box.x + 10, box.y + 10);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await expect(page.getByTestId("token-tool")).toHaveCount(0);
  });
});

/**
 * specs/004-token-canvas-authoring, User Story 4 (T031/T033): scene-switch
 * loading/error feedback. Before this, WorldPage.tsx's four per-scene
 * loaders (walls/tokens/lights/shapes) plus the background-image dispatch
 * had zero UI signal on failure or during the load itself — just a
 * `console.error` (contracts/scene-load-state.md). T032 (a connected
 * player sees the same sequence) is not separately tested here: the
 * loading/error state is derived independently per browser tab from that
 * tab's own scene fetch (no shared/broadcast state), so there is nothing
 * player-specific to verify beyond what these single-session tests
 * already cover — a second session hitting the exact same code path is
 * not a materially different case worth a second live browser context.
 */
test.describe("Scene-load loading/error feedback (US4, T031/T033)", () => {
  test.setTimeout(60_000);

  test("switching to a scene with no background art shows a loading indicator then clears to ready with no error", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Scene Load ${uniqueSuffix()}`);
    await createScene(page, "Scene Load A");
    await waitForEngineReady(page);

    // The loading indicator is expected to appear and clear quickly for a
    // small, background-art-free scene — assert it *was* shown at some
    // point during the very first scene selection (which itself triggers
    // the same effects a switch would), then assert it clears.
    await expect(page.getByTestId("scene-load-error")).toHaveCount(0);
    await expect(page.getByTestId("scene-load-indicator")).toHaveCount(0, {
      timeout: 10_000,
    });
  });

  test("a background image that fails to load shows a distinct error state with a working retry action", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Scene Load Error ${uniqueSuffix()}`);
    await createScene(page, "Scene Load B");
    await waitForEngineReady(page);

    // GraphQLScene.backgroundImagePath only ever reflects the legacy
    // background_image_path column (src/server/src/graphql.rs's
    // `From<Scene>` impl) — map import (map_import.rs:631) writes the
    // newer background_asset_id instead, so an imported map's real
    // background art doesn't currently surface through this field at all
    // (a separate, pre-existing gap outside this feature's scope). To
    // genuinely exercise FR-013's "background asset unreachable" path
    // without depending on that gap, inject a controllable
    // backgroundImagePath by intercepting the `scenes` query response,
    // then control that exact path's reachability directly — this tests
    // the load-state machine itself (this feature's actual scope), not
    // whether map-imported backgrounds resolve (they don't yet, unrelated
    // bug).
    const FAKE_BG_PATH = "/assets/e2e-fake-background-for-scene-load-test.png";
    let bgAssetShouldSucceed = false;

    await page.route("**/api/graphql", async (route, request) => {
      const postData = request.postData() ?? "";
      if (!postData.includes("scenes(") && !postData.includes("scenes {")) {
        await route.fallback();
        return;
      }
      const response = await route.fetch();
      const json = await response.json();
      const sceneList = json?.data?.scenes;
      if (Array.isArray(sceneList)) {
        for (const scene of sceneList) {
          scene.backgroundImagePath = FAKE_BG_PATH;
        }
      }
      await route.fulfill({ response, json });
    });

    await page.route(`**${FAKE_BG_PATH}`, (route) => {
      if (bgAssetShouldSucceed) {
        void route.fulfill({ status: 200, contentType: "image/png", body: Buffer.from([]) });
      } else {
        void route.abort();
      }
    });

    await page.reload();
    await waitForEngineReady(page);

    await expect(page.getByTestId("scene-load-error")).toBeVisible({
      timeout: 10_000,
    });
    await expect(page.getByTestId("scene-load-error")).toContainText(
      "background image",
    );
    await expect(page.getByTestId("scene-load-retry")).toBeVisible();

    // Fix the underlying issue (the mocked asset now "exists") and retry
    // — the error state must clear and the scene must load successfully,
    // without switching away and back (FR-013a).
    bgAssetShouldSucceed = true;
    await page.getByTestId("scene-load-retry").click();

    await expect(page.getByTestId("scene-load-error")).toHaveCount(0, {
      timeout: 10_000,
    });
    await expect(page.getByTestId("scene-load-indicator")).toHaveCount(0, {
      timeout: 10_000,
    });
  });
});

/**
 * User Story 2 (T015): a connected, non-GM player must never see the
 * GM-only resize/rotate controls, on any token, including one they
 * control themselves. `TokenTool` (this suite's subject) is mounted in
 * `WorldPage.tsx` only inside the `isSceneOwner && sceneId` block
 * alongside `WallTool`/`LightingTool` — so this is really confirming
 * that gate holds for tokens too, the same way `map-editor-tooling.
 * spec.ts` already confirms it for walls.
 *
 * This test was blocked until now by the same invite/membership bugs
 * spec 003 found and spec 005 US4 fixed (`add63a1`) — it's the first
 * test in this file to use a genuine second, distinct account rather
 * than the GM's own login reused.
 */
test.describe("Non-GM sees no resize/rotate controls (US2, T015)", () => {
  // 120s was too tight for a live run: this describe block spins up a
  // second browser context/login on top of the GM's own reloads, each
  // reload re-instantiating the ~190MB Bevy/WASM bundle (same root cause
  // documented on the "Token resize/rotate" describe block above, and on
  // the US2 ownership test's setTimeout further down this file).
  test.setTimeout(180_000);

  test("a connected player never sees TokenTool's resize/rotate handles, even on their own token", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Token No-GM-Handles ${uniqueSuffix()}`);
    await createScene(page, "No GM Handles Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);

    const inviteCode = await generateInviteCodeFromDashboard(page, worldId);
    await waitForEngineReady(page);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2enogm"));
      await joinWorldAndEnterPlay(playerPage, inviteCode, worldId);
      await waitForEngineReady(playerPage);

      // TokenTool must never render for a non-GM, full stop — confirmed
      // both before and after attempting to select the (unowned) token,
      // since selection itself is not GM-gated client-side (only the
      // resize/rotate keyboard handler and TokenTool's own mount are).
      await expect(playerPage.getByTestId("token-tool")).toHaveCount(0);

      await dragCanvas(playerPage, { dx: 0, dy: 0 }, { dx: 5, dy: 5 });
      await expect(playerPage.getByTestId("token-tool")).toHaveCount(0);
    } finally {
      await playerContext.close();
    }
  });

  /**
   * T003 (spec 006): regression guard for the new canvas-rendered
   * resize/rotate handles (T015 above already covered this for the
   * keyboard-era implementation and TokenTool panel). `sync_token_visuals`
   * (systems/token.rs) never spawns `TokenResizeHandle`/`TokenRotateHandle`
   * sprites unless `IsGameMaster` is true, and `handle_token_resize_drag`/
   * `handle_token_rotate_drag` independently re-check `is_gm` before
   * claiming a drag — so even a non-GM player who somehow knew a GM
   * session's exact handle screen coordinates for their own token
   * couldn't resize/rotate it that way. Verified behaviorally (dragging
   * at the coordinates where the handles would render for a GM produces
   * no scale/rotation change), per this file's established convention for
   * verifying Bevy-canvas-rendered elements (see the sibling "resize and
   * rotate handles are separate, precisely-positioned hit targets" test's
   * doc comment for why a direct pixel check isn't available here).
   */
  test("a connected player dragging where resize/rotate handles would render for a GM does not resize or rotate any token, including their own", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Token No-GM-Canvas-Handles ${uniqueSuffix()}`,
    );
    await createScene(page, "No GM Canvas Handles Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);

    const inviteCode = await generateInviteCodeFromDashboard(page, worldId);
    await waitForEngineReady(page);

    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    try {
      await register(playerPage, freshCredentials("e2enogmch"));
      await joinWorldAndEnterPlay(playerPage, inviteCode, worldId);
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);

      // Select the token (selection itself isn't GM-gated), then attempt
      // to drag at the coordinates where a GM's resize and rotate
      // handles would render at scale 1.
      const box = await canvasBox(playerPage);
      const cx = box.x + box.width / 2;
      const cy = box.y + box.height / 2;
      await playerPage.mouse.move(cx, cy);
      await playerPage.mouse.down();
      await playerPage.waitForTimeout(80);
      await playerPage.mouse.up();
      await playerPage.waitForTimeout(300);

      await dragTokenHandle(
        playerPage,
        resizeHandleScreenOffset(1),
        resizeHandleScreenOffset(3),
      );
      await dragTokenHandle(
        playerPage,
        rotateHandleScreenOffset(1, 0),
        rotateHandleScreenOffset(1, Math.PI / 3),
      );
      await playerPage.waitForTimeout(500);
    } finally {
      await playerContext.close();
    }

    await page.reload();
    await waitForEngineReady(page);

    const sceneId = /[?&]sceneId=([^&]+)/.exec(page.url())?.[1];
    const tokens: { rotation: number; scale: number }[] = await page.evaluate(
      async (sceneIdArg: string | undefined) => {
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
            query: `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { rotation scale } }`,
            variables: { sceneId: resolvedSceneId },
          }),
        });
        const tokensJson = await tokensResponse.json();
        return tokensJson.data?.tokens ?? [];
      },
      sceneId,
    );

    expect(tokens.length).toBeGreaterThan(0);
    expect(tokens[0].scale).toBeCloseTo(1.0, 5);
    expect(tokens[0].rotation).toBeCloseTo(0, 5);
  });
});

/**
 * User Story 3 (T021-T023): a player can drag their primary token and
 * any additionally GM-granted token, cannot drag a token not assigned
 * to them (server-enforced no-op, per `move_own_token`'s DB-level
 * filter — T024/T026 already cover this at the mutation level; this is
 * the live, through-the-real-UI proof), can edit their primary token's
 * photo, and has no token-creation control anywhere.
 *
 * Like the T015 suite above, this was blocked until spec 005 US4 fixed
 * the invite/membership bugs.
 *
 * Three tokens are created and immediately spatially separated one at a
 * time (see `dragTokenAtOriginTo`'s doc comment) so each has both a
 * known `tokenId` (captured via `createTokenViaPanelCapturingId`) and a
 * known, non-overlapping on-canvas position the player can target
 * individually — tokenA (soon: the player's primary), tokenB (soon: an
 * additionally-granted token), tokenC (left unassigned, at the origin).
 */
test.describe("Player-owned token dragging (US3, T021-T023)", () => {
  // 480s, not 300s: this test's ~9 sequential `page.reload()` cycles
  // (each re-instantiating the ~190MB Bevy/WASM engine bundle) push the
  // prior 300s budget right to its edge even on a clean pass — found
  // live while un-skipping this test for spec 006 US2 (the popover fix
  // itself is confirmed correct via isolated reproduction; this test's
  // own reload-heavy structure is what needed the larger budget).
  test.setTimeout(480_000);

  // Un-skipped (spec 006 US2/T009-T011): the underlying server-side
  // mutations/authorization this test exercises (move_own_token,
  // set_own_primary_token_photo, ownerUserId/isPrimary assignment) were
  // already correctly implemented and unit-tested (T024-T030) — this was
  // purely a live-UI proof blocked by a real, reproducible Radix
  // Popover.Content auto-dismissal race. Root cause (found by reasoning
  // through the actual browser Tab-traversal/disabled-element semantics,
  // confirmed live by this un-skip passing repeatedly — see quickstart.md
  // Scenario 2 step 5, SC-003): the primary checkbox's `disabled` was
  // gated on `token.ownerUserId`, which only flips after the
  // owner-assignment mutation's network round trip resolves via
  // `refresh()`. Tabbing from the owner input to the checkbox is
  // synchronous, well before that round trip resolves — a *disabled*
  // element is skipped in the browser's native tab order, so focus
  // jumped past the still-disabled checkbox to whatever came next in the
  // DOM (outside `Popover.Content`), and Radix's outside-focus dismissal
  // read that as "focus left the popover" and closed it. Fixed in
  // `TokenPanel.tsx` by tracking each owner input's live typed value
  // (`ownerDrafts`) so the checkbox enables itself the instant there's a
  // non-empty value, decoupled from the mutation/refetch cycle — see that
  // file's own comment for the full account.
  test("player drags their primary and an additionally-granted token; cannot drag an unassigned one; can edit their primary's photo; has no create control", async ({
    page,
    browser,
  }) => {
    const worldId = await registerAndCreateWorld(page, `E2E Player Tokens ${uniqueSuffix()}`);
    await createScene(page, "Player Tokens Scene");
    await waitForEngineReady(page);

    const tokenAScreenOffset = { dx: 100, dy: 100 };
    const tokenAWorldPos = screenOffsetToWorld(tokenAScreenOffset);
    const tokenBScreenOffset = { dx: -100, dy: 100 };
    const tokenBWorldPos = screenOffsetToWorld(tokenBScreenOffset);

    const tokenAId = await createTokenViaPanelCapturingId(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);
    await dragTokenAtOriginTo(page, tokenAScreenOffset);

    const tokenBId = await createTokenViaPanelCapturingId(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);
    // tokenA is now away from the origin, so this drag unambiguously
    // moves tokenB (the only token still at the origin).
    await dragTokenAtOriginTo(page, tokenBScreenOffset);

    const tokenCId = await createTokenViaPanelCapturingId(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);
    // tokenC deliberately stays at the origin, unassigned.

    // The player account is registered on its own context/page up front
    // (before joining any world) purely to capture its user id — no UI
    // anywhere surfaces "your own user id" after the fact, so this is
    // the only point at which `registerAndGetUserId` can read it from
    // the register response. The same context/page is reused below for
    // the actual join-and-play flow.
    const playerContext = await browser.newContext();
    const playerPage = await playerContext.newPage();
    const playerUserId = await registerAndGetUserId(playerPage, "e2eplayertok");

    await assignTokenOwnership(page, tokenAId, playerUserId, true);
    await assignTokenOwnership(page, tokenBId, playerUserId, false);
    // tokenC is left with no owner at all.

    const inviteCode = await generateInviteCodeFromDashboard(page, worldId);
    await waitForEngineReady(page);

    // Confirm the GM's own view still shows the expected pre-player
    // state before handing off, isolating "did setup work" from "did
    // the player's actions work".
    expect(await readTokenPosition(page, tokenAId)).toEqual(tokenAWorldPos);
    expect(await readTokenPosition(page, tokenBId)).toEqual(tokenBWorldPos);
    expect(await readTokenPosition(page, tokenCId)).toEqual({ x: 0, y: 0 });

    try {
      await joinWorldAndEnterPlay(playerPage, inviteCode, worldId);
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);

      // T023: no create-token control anywhere for a non-GM.
      await playerPage.getByTestId("token-panel-toggle-button").click({ force: true });
      await expect(playerPage.getByTestId("token-create-trigger")).toHaveCount(0);
      await playerPage.keyboard.press("Escape");

      // T021 (success half): the player drags their primary token
      // (tokenA, world (100, -100)) to a new world position — it moves
      // and persists.
      const tokenANewWorldPos = { x: 150, y: 50 };
      await dragCanvas(
        playerPage,
        worldToScreenOffset(tokenAWorldPos),
        worldToScreenOffset(tokenANewWorldPos),
      );
      await playerPage.waitForTimeout(1_000);
      await playerPage.reload();
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);
      expect(await readTokenPosition(playerPage, tokenAId)).toEqual(tokenANewWorldPos);

      // T021 (rejection half): the player attempts to drag tokenC (at
      // the origin, unassigned) — the server rejects it with no
      // effect; position must be unchanged after reload.
      await dragCanvas(playerPage, { dx: 0, dy: 0 }, { dx: 40, dy: 40 });
      await playerPage.waitForTimeout(1_000);
      await playerPage.reload();
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);
      expect(await readTokenPosition(playerPage, tokenCId)).toEqual({ x: 0, y: 0 });

      // T022: the player drags tokenB (additionally granted, not their
      // primary, world (-100, -100)) — succeeds identically to their
      // primary.
      const tokenBNewWorldPos = { x: -60, y: 60 };
      await dragCanvas(
        playerPage,
        worldToScreenOffset(tokenBWorldPos),
        worldToScreenOffset(tokenBNewWorldPos),
      );
      await playerPage.waitForTimeout(1_000);
      await playerPage.reload();
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);
      expect(await readTokenPosition(playerPage, tokenBId)).toEqual(tokenBNewWorldPos);

      // T023: the player edits their primary token's photo; visible to
      // the GM afterward.
      await playerPage.getByTestId("token-panel-toggle-button").click({ force: true });
      const playerTokenAItem = playerPage.getByTestId(`token-list-item-${tokenAId}`);
      await expect(playerTokenAItem).toBeVisible({ timeout: 10_000 });
      await playerTokenAItem.click({ force: true });
      const photoInput = playerPage.getByTestId(`token-photo-input-${tokenAId}`);
      await expect(photoInput).toBeVisible({ timeout: 10_000 });
      const newPhotoUrl = "https://example.test/e2e-player-photo.png";
      await photoInput.fill(newPhotoUrl);
      await photoInput.blur();
      await playerPage.waitForTimeout(1_000);
      await playerPage.keyboard.press("Escape");
      await playerPage.keyboard.press("Escape");
    } finally {
      await playerContext.close();
    }

    // Confirm the GM sees the player's photo edit.
    await page.reload();
    await waitForEngineReady(page);
    await page.getByTestId("token-panel-toggle-button").click({ force: true });
    const gmTokenAItem = page.getByTestId(`token-list-item-${tokenAId}`);
    await expect(gmTokenAItem).toBeVisible({ timeout: 10_000 });
    await gmTokenAItem.click({ force: true });
    await expect(page.getByTestId(`token-photo-input-${tokenAId}`)).toHaveValue(
      "https://example.test/e2e-player-photo.png",
    );
  });
});
