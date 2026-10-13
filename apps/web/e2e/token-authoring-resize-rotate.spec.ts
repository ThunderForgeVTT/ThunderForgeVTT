import { test, expect } from "./fixtures/test";
import {
  ONE_PIXEL_PNG,
  selectPersistedToken,
  persistedToken,
  anchoredAt,
  openTokenTool,
  canvasBox,
  waitForEngineReady,
  resizeHandleScreenOffset,
  rotateHandleScreenOffset,
  dragTokenHandle,
  createTokenViaPanel,
  worldToScreenOffset,
} from "./fixtures/tokenAuthoring";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
} from "./fixtures/tokenAuthoringSession";

/**
 * User Story 1 (spec 006, T001-T003): GM resizes a selected token by
 * dragging its real canvas-rendered resize handle in whole grid-cell
 * increments, and rotates its facing continuously by dragging a separate
 * rotate handle, independently. Replaces spec 004's keyboard-shortcut
 * stand-in (`]`/`[`/`,`/`.`, still available as a secondary path per
 * `handle_token_resize_rotate_keyboard`'s doc comment) with actual
 * draggable handle sprites (`TokenResizeHandle`/`TokenRotateHandle` in
 * `crates/thunderforge-engine/src/systems/token.rs`), closing the interaction-affordance
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

    // Select the token by clicking where it actually is — not the world
    // origin, which `TokenPanel` creates at but grid snapping moves it off.
    const box = await canvasBox(page);
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    // Handles render around the *selected* token, so the anchor has to be
    // that token and not merely the first persisted one in the store.
    const underTest = await selectPersistedToken(page, cx, cy);
    const anchor = worldToScreenOffset(underTest);

    // T001/FR-001: the resize handle is now rendered at the token's
    // corner (scale 1) — grab it and drag out to the distance
    // corresponding to scale 3 (whole grid-cell increments, +2 cells).
    const resizeStart = anchoredAt(anchor, resizeHandleScreenOffset(1));
    const resizeEnd = anchoredAt(anchor, resizeHandleScreenOffset(3));
    await dragTokenHandle(page, resizeStart, resizeEnd);

    // T001/FR-003: the rotate handle sits at a separate offset (scaled
    // with the token, per `rotate_handle_world_pos`) — grab it and drag
    // to the angle corresponding to a 60-degree (PI/3) rotation,
    // independent of the resize above.
    const rotateStart = anchoredAt(anchor, rotateHandleScreenOffset(3, 0));
    const rotateEnd = anchoredAt(
      anchor,
      rotateHandleScreenOffset(3, Math.PI / 3),
    );
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
    const tokens: {
      tokenId: string;
      x: number;
      y: number;
      rotation: number;
      scale: number;
    }[] = await page.evaluate(async (sceneIdArg: string | undefined) => {
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
          query: `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y rotation scale } }`,
          variables: { sceneId: resolvedSceneId },
        }),
      });
      const tokensJson = await tokensResponse.json();
      return tokensJson.data?.tokens ?? [];
    }, sceneId);

    expect(tokens.length).toBeGreaterThan(0);
    const resized = persistedToken(tokens, underTest.id);
    expect(resized.scale).toBeCloseTo(3.0, 5); // 1.0 default + 2 increments
    // Two 30-degree steps = 60 degrees = PI/3 radians.
    expect(resized.rotation).toBeCloseTo(Math.PI / 3, 2);
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
    // Anchored on the token actually selected — see the sibling test above.
    const underTest = await selectPersistedToken(page, cx, cy);
    const anchor = worldToScreenOffset(underTest);

    // Drag only the resize handle: scale changes, position and rotation
    // don't.
    await dragTokenHandle(
      page,
      anchoredAt(anchor, resizeHandleScreenOffset(1)),
      anchoredAt(anchor, resizeHandleScreenOffset(2)),
    );
    await page.waitForTimeout(500);

    // Drag only the rotate handle (now at scale 2's offset): rotation
    // changes, position and scale don't.
    await dragTokenHandle(
      page,
      anchoredAt(anchor, rotateHandleScreenOffset(2, 0)),
      anchoredAt(anchor, rotateHandleScreenOffset(2, Math.PI / 2)),
    );
    await page.waitForTimeout(500);

    await page.reload();
    await waitForEngineReady(page);

    const sceneId = /[?&]sceneId=([^&]+)/.exec(page.url())?.[1];
    const tokens: {
      tokenId: string;
      x: number;
      y: number;
      rotation: number;
      scale: number;
    }[] = await page.evaluate(async (sceneIdArg: string | undefined) => {
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
          query: `query($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y rotation scale } }`,
          variables: { sceneId: resolvedSceneId },
        }),
      });
      const tokensJson = await tokensResponse.json();
      return tokensJson.data?.tokens ?? [];
    }, sceneId);

    expect(tokens.length).toBeGreaterThan(0);
    const handled = persistedToken(tokens, underTest.id);
    // Position is asserted against where the token actually was, not (0,0):
    // it is grid-snapped on creation, and the point here is that *handle*
    // drags never move it.
    expect(handled.x).toBeCloseTo(underTest.x, 0);
    expect(handled.y).toBeCloseTo(underTest.y, 0);
    expect(handled.scale).toBeCloseTo(2.0, 5);
    expect(handled.rotation).toBeCloseTo(Math.PI / 2, 2);
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
    // Not a centre click: tokens do not spawn at the world origin, and the
    // engine's own demo token cannot persist a change. See
    // `selectPersistedToken`.
    await selectPersistedToken(page, cx, cy);

    // Selecting the token on the canvas must surface the panel — this is
    // the discoverability gap T020 closes: previously only `]`/`[`/`,`/`.`
    // worked, with nothing on screen telling a GM they existed.
    await openTokenTool(page);
    await expect(page.getByTestId("token-tool")).toBeVisible({
      timeout: 5_000,
    });
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
    await selectPersistedToken(page, cx, cy);
    // The rail resets to no open tool on reload.
    await openTokenTool(page);
    await expect(page.getByTestId("token-tool-scale")).toContainText("2x");
    await expect(page.getByTestId("token-tool-rotation")).toContainText("30°");

    // Deselecting (click empty canvas) must hide the panel again.
    //
    // Not the canvas's top-left corner: the GM tool rail is painted over
    // that edge, so a click there hits the rail and never reaches the
    // engine — the panel then stays open and looks like a deselect bug.
    // Somewhere clear of both the rail and the tokens instead.
    await page.mouse.move(cx + 400, cy + 300);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await expect(page.getByTestId("token-tool")).toHaveCount(0);
  });

  test("GM sets token art from TokenTool, it persists across reload, and can be removed again", async ({
    page,
  }) => {
    await registerAndCreateWorld(page, `E2E Token Art ${uniqueSuffix()}`);
    await createScene(page, "Token Art Scene");
    await waitForEngineReady(page);

    await createTokenViaPanel(page);
    await page.reload();
    await waitForEngineReady(page);
    await page.waitForTimeout(1_500);

    const box = await canvasBox(page);
    const cx = box.x + box.width / 2;
    const cy = box.y + box.height / 2;
    // Click where the token actually is, not where the canvas centre is.
    // Tokens do not spawn at the world origin — the probe reports this
    // scene's at (-192.5, -62.5) — so a centre click lands on empty canvas
    // and the engine emits a *deselect*, which looks exactly like "the
    // panel is broken" from the DOM. World units map 1:1 to pixels at the
    // default zoom, with the origin at the canvas centre and y pointing up.
    await selectPersistedToken(page, cx, cy);
    await openTokenTool(page);

    await expect(page.getByTestId("token-tool-art")).toBeVisible({
      timeout: 5_000,
    });

    // A token with no art has no preview and nothing to remove.
    await expect(page.getByTestId("token-tool-art-preview")).toHaveCount(0);
    await expect(page.getByTestId("token-tool-art-clear")).toHaveCount(0);

    // Upload drives the same `uploadCanvasImage` path AssetPasteTool uses,
    // so the art ends up a same-origin `/api/canvas-assets/<id>.webp` URL —
    // the only form the engine's asset loader can actually fetch.
    await page.getByTestId("token-tool-art-file").setInputFiles({
      name: "token-art.png",
      mimeType: "image/png",
      buffer: ONE_PIXEL_PNG,
    });

    const preview = page.getByTestId("token-tool-art-preview");
    await expect(preview).toBeVisible({ timeout: 20_000 });
    await expect(preview).toHaveAttribute(
      "src",
      /\/api\/canvas-assets\/[0-9a-f-]+\.webp$/,
    );
    const uploadedSrc = await preview.getAttribute("src");

    // Persisted, not just React state: the art has to survive the GraphQL
    // round trip, which is what `UpdateTokenInput.photoUrl` was added for.
    await page.reload();
    await waitForEngineReady(page);
    await selectPersistedToken(page, cx, cy);
    await openTokenTool(page);
    await expect(page.getByTestId("token-tool-art-preview")).toHaveAttribute(
      "src",
      uploadedSrc!,
      { timeout: 10_000 },
    );

    // Removing art sends an explicit null, which is a different request
    // from omitting the field — a plain `Option` on the server collapsed
    // both to "unchanged" and made this impossible.
    await page.getByTestId("token-tool-art-clear").click();
    await page.waitForTimeout(1_000);
    await expect(page.getByTestId("token-tool-art-preview")).toHaveCount(0);

    await page.reload();
    await waitForEngineReady(page);
    await selectPersistedToken(page, cx, cy);
    await openTokenTool(page);
    await expect(page.getByTestId("token-tool-art")).toBeVisible({
      timeout: 5_000,
    });
    await expect(page.getByTestId("token-tool-art-preview")).toHaveCount(0);
  });
});
