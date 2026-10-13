import { test, expect } from "./fixtures/test";
import {
  selectPersistedToken,
  selectedToken,
  canvasBox,
  waitForEngineReady,
  dragCanvas,
  createTokenViaPanel,
  worldToScreenOffset,
} from "./fixtures/tokenAuthoring";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  secondSessionSameLogin,
} from "./fixtures/tokenAuthoringSession";

/**
 * specs/004-token-canvas-authoring, User Story 1 (T008/T009): a GM can
 * reposition an existing token directly on the canvas (no TokenPanel
 * needed to move it), the move persists across reload, and TokenPanel's
 * own displayed position never disagrees with where the canvas last put
 * it (FR-005) — both read/write the same `tokens` row per T006's
 * backing-store unification (ADR-040).
 *
 * The helpers live in `fixtures/tokenAuthoring.ts` and
 * `fixtures/tokenAuthoringSession.ts`, shared by every
 * `token-authoring-*.spec.ts` file.
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

    // Not the world origin: `TokenPanel` creates at (0, 0) but grid
    // snapping moves the token before it settles, so the drag has to start
    // from where it actually is (see `selectPersistedToken`) and the
    // assertion below is start-plus-delta.
    // Every token in this scene is stacked on exactly one point: the panel
    // creates at (0, 0), grid snapping puts that at (-192.5, -62.5), and
    // the engine's demo token is already there. Dragging that pile blind
    // moves nothing — `selectPersistedToken` clears the demo token off the
    // top and leaves a real one held, which is the only way to know which
    // token the drag below is actually about to move.
    const box = await canvasBox(page);
    await selectPersistedToken(
      page,
      box.x + box.width / 2,
      box.y + box.height / 2,
    );
    const before = await selectedToken(page);
    const from = worldToScreenOffset(before);
    await dragCanvas(page, from, { dx: from.dx + 120, dy: from.dy - 80 });

    // Give the drop's `upsert_token` → mutation-bridge → `updateToken`
    // round-trip a moment to actually persist server-side before we
    // reload and re-query it.
    await page.waitForTimeout(1_000);

    await page.reload();
    await waitForEngineReady(page);

    await page.getByTestId("token-panel-toggle-button").click({ force: true });
    // By id, not `.first()`: the scene holds several tokens and list order
    // is not a statement about which one this test dragged.
    const tokenItem = page.getByTestId(`token-list-item-${before.id}`);
    await expect(tokenItem).toBeVisible({ timeout: 10_000 });
    // `force: true`: the token list item's bounding box appears to churn
    // frame-to-frame while the WorldLayout party-roster sidebar is also
    // live-rendering this same token's position, which starves
    // Playwright's default actionability "stable for 2 consecutive
    // frames" check — the element is genuinely visible/clickable
    // (confirmed via screenshots during debugging), so bypass that check
    // rather than fight it.
    await tokenItem.click({ force: true });

    const positionText = page.getByTestId(`token-position-${before.id}`);
    await expect(positionText).toBeVisible({ timeout: 10_000 });
    const text = await positionText.textContent();
    const match = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(text ?? "");
    expect(match).not.toBeNull();
    const [, xStr, yStr] = match!;
    // Dragged +120 on screen x / -80 on screen y from wherever the token
    // was; canvas y is screen-down while Bevy world-space y is screen-up
    // (matching canvas-authoring.spec.ts's own wall-coordinate
    // assertions), so the persisted world y moves by the *positive* of the
    // screen-space dy. Tolerance is a whole grid cell: the drop snaps.
    expect(Number(xStr)).toBeCloseTo(before.x + 120, -1);
    expect(Number(yStr)).toBeCloseTo(before.y + 80, -1);
  });

  test("a second session opening the scene fresh sees the GM's dragged token position (no live-subscription transport yet — spec 005)", async ({
    page,
    browser,
  }) => {
    await registerAndCreateWorld(
      page,
      `E2E Token Cross-Session ${uniqueSuffix()}`,
    );
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
    // Same stack as the sibling test: clear the demo token off the top so
    // the drag below has a known token in hand.
    const box = await canvasBox(page);
    await selectPersistedToken(
      page,
      box.x + box.width / 2,
      box.y + box.height / 2,
    );
    const before = await selectedToken(page);
    const from = worldToScreenOffset(before);
    await dragCanvas(page, from, { dx: from.dx + 60, dy: from.dy + 40 });
    await page.waitForTimeout(1_000);
    // Where it came to rest, snapping included — both sessions must agree
    // on this exact value, which is the point of the test.
    const after = await selectedToken(page);
    // Assert the drag moved it before asserting the two sessions agree:
    // without this the test passes when the drag does nothing at all,
    // because `after` is then just `before` and both sessions dutifully
    // report the same unmoved token. Tolerance is a whole grid cell (the
    // drop snaps); y moves by the negative of screen dy.
    expect(after.x).toBeCloseTo(before.x + 60, -1);
    expect(after.y).toBeCloseTo(before.y - 40, -1);

    // Confirm the drag actually persisted on the GM's own page before
    // involving a second session at all — isolates "did the drag work"
    // from "did the second session read stale/fresh data".
    await page.getByTestId("token-panel-toggle-button").click({ force: true });
    const gmTokenItem = page.getByTestId(`token-list-item-${before.id}`);
    await expect(gmTokenItem).toBeVisible({ timeout: 10_000 });
    await gmTokenItem.click({ force: true });
    const gmPositionText = page.getByTestId(`token-position-${before.id}`);
    await expect(gmPositionText).toBeVisible({ timeout: 10_000 });
    const gmText = await gmPositionText.textContent();
    const gmMatch = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(gmText ?? "");
    expect(gmMatch).not.toBeNull();
    expect(Number(gmMatch![1])).toBeCloseTo(after.x, 0);
    expect(Number(gmMatch![2])).toBeCloseTo(after.y, 0);
    await page.keyboard.press("Escape");
    await page.keyboard.press("Escape");

    const { context: secondContext, page: secondPage } =
      await secondSessionSameLogin(browser, page);
    try {
      await waitForEngineReady(secondPage);
      await secondPage
        .getByTestId("token-panel-toggle-button")
        .click({ force: true });
      const tokenItem = secondPage.getByTestId(`token-list-item-${before.id}`);
      await expect(tokenItem).toBeVisible({ timeout: 10_000 });
      // `force: true`: the token list item's bounding box appears to churn
      // frame-to-frame while the WorldLayout party-roster sidebar is also
      // live-rendering this same token's position, which starves
      // Playwright's default actionability "stable for 2 consecutive
      // frames" check — the element is genuinely visible/clickable
      // (confirmed via screenshots during debugging), so bypass that check
      // rather than fight it.
      await tokenItem.click({ force: true });

      const positionText = secondPage.getByTestId(
        `token-position-${before.id}`,
      );
      await expect(positionText).toBeVisible({ timeout: 10_000 });
      const text = await positionText.textContent();
      const match = /Position: \(([-\d.]+), ([-\d.]+)\)/.exec(text ?? "");
      expect(match).not.toBeNull();
      const [, xStr, yStr] = match!;
      expect(Number(xStr)).toBeCloseTo(after.x, 0);
      expect(Number(yStr)).toBeCloseTo(after.y, 0);
    } finally {
      await secondContext.close();
    }
  });
});
