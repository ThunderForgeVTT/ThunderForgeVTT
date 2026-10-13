import { test, expect } from "./fixtures/test";
import { inviteAndJoinAsPlayer } from "./fixtures/helpers";
import {
  canvasBox,
  waitForEngineReady,
  dragCanvas,
  resizeHandleScreenOffset,
  rotateHandleScreenOffset,
  dragTokenHandle,
  createTokenViaPanel,
  createTokenViaPanelCapturingId,
  dragTokenAtOriginTo,
  snapWorld,
  screenOffsetToWorld,
  worldToScreenOffset,
  readTokenPosition,
  assignTokenOwnership,
} from "./fixtures/tokenAuthoring";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  registerAndGetUserId,
  generateInviteCodeFromDashboard,
  joinWorldAndEnterPlay,
  createScene,
} from "./fixtures/tokenAuthoringSession";

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
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Token No-GM-Handles ${uniqueSuffix()}`,
    );
    await createScene(page, "No GM Handles Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);

    // Membership over GraphQL rather than through the invite UI: this test
    // is about what a player *sees on the canvas*, and the clicked invite
    // flow ("Generate Join Link" → read the code → "Join Campaign") was the
    // suite's single largest source of noise, intermittently never
    // surfacing one of its buttons under load and killing the test on its
    // full timeout. See `inviteAndJoinAsPlayer`'s own doc comment; the one
    // spec that is genuinely about the invite UI still clicks every button.
    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2enogm",
    );
    const playerContext = playerPage.context();
    try {
      await playerPage.goto(`/world/${worldId}/play`);
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

    // See the sibling test above for why membership is created over
    // GraphQL instead of through the invite UI.
    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      page,
      worldId,
      "e2enogmch",
    );
    const playerContext = playerPage.context();
    try {
      await playerPage.goto(`/world/${worldId}/play`);
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
    const tokens: { tokenId: string; rotation: number; scale: number }[] =
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
        const worldMatch = /\/world\/([^/]+)\/play/.exec(
          window.location.pathname,
        );
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
        const resolvedSceneId =
          sceneIdArg ?? scenesJson.data?.scenes?.[0]?.sceneId;
        const tokensResponse = await fetch("/api/graphql", {
          method: "POST",
          headers,
          credentials: "same-origin",
          body: JSON.stringify({
            query: `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId rotation scale } }`,
            variables: { sceneId: resolvedSceneId },
          }),
        });
        const tokensJson = await tokensResponse.json();
        return tokensJson.data?.tokens ?? [];
      }, sceneId);

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
    const worldId = await registerAndCreateWorld(
      page,
      `E2E Player Tokens ${uniqueSuffix()}`,
    );
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
    expect(await readTokenPosition(page, tokenAId)).toEqual(
      snapWorld(tokenAWorldPos),
    );
    expect(await readTokenPosition(page, tokenBId)).toEqual(
      snapWorld(tokenBWorldPos),
    );
    expect(await readTokenPosition(page, tokenCId)).toEqual({ x: 0, y: 0 });

    try {
      await joinWorldAndEnterPlay(playerPage, inviteCode, worldId);
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);

      // T023: no create-token control anywhere for a non-GM.
      await playerPage
        .getByTestId("token-panel-toggle-button")
        .click({ force: true });
      await expect(playerPage.getByTestId("token-create-trigger")).toHaveCount(
        0,
      );
      await playerPage.keyboard.press("Escape");

      // T021 (success half): the player drags their primary token
      // (tokenA, world (100, -100)) to a new world position — it moves
      // and persists.
      const tokenANewWorldPos = { x: 150, y: 50 };
      // Grabbed at the token's *resting* position. It snapped to a cell
      // centre when the GM dropped it, so aiming at the raw drag target
      // starts the drag beside the token — close enough to look right and
      // capable of catching a neighbour instead, which the server then
      // refuses with "not found or not controlled by you".
      await dragCanvas(
        playerPage,
        worldToScreenOffset(snapWorld(tokenAWorldPos)),
        worldToScreenOffset(tokenANewWorldPos),
      );
      await playerPage.waitForTimeout(1_000);
      await playerPage.reload();
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);
      expect(await readTokenPosition(playerPage, tokenAId)).toEqual(
        snapWorld(tokenANewWorldPos),
      );

      // T021 (rejection half): the player attempts to drag tokenC (at
      // the origin, unassigned) — the server rejects it with no
      // effect; position must be unchanged after reload.
      await dragCanvas(playerPage, { dx: 0, dy: 0 }, { dx: 40, dy: 40 });
      await playerPage.waitForTimeout(1_000);
      await playerPage.reload();
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);
      expect(await readTokenPosition(playerPage, tokenCId)).toEqual({
        x: 0,
        y: 0,
      });

      // T022: the player drags tokenB (additionally granted, not their
      // primary, world (-100, -100)) — succeeds identically to their
      // primary.
      const tokenBNewWorldPos = { x: -60, y: 60 };
      await dragCanvas(
        playerPage,
        worldToScreenOffset(snapWorld(tokenBWorldPos)),
        worldToScreenOffset(tokenBNewWorldPos),
      );
      await playerPage.waitForTimeout(1_000);
      await playerPage.reload();
      await waitForEngineReady(playerPage);
      await playerPage.waitForTimeout(1_500);
      expect(await readTokenPosition(playerPage, tokenBId)).toEqual(
        snapWorld(tokenBNewWorldPos),
      );

      // T023: the player edits their primary token's photo; visible to
      // the GM afterward.
      await playerPage
        .getByTestId("token-panel-toggle-button")
        .click({ force: true });
      const playerTokenAItem = playerPage.getByTestId(
        `token-list-item-${tokenAId}`,
      );
      await expect(playerTokenAItem).toBeVisible({ timeout: 10_000 });
      await playerTokenAItem.click({ force: true });
      const photoInput = playerPage.getByTestId(
        `token-photo-input-${tokenAId}`,
      );
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
