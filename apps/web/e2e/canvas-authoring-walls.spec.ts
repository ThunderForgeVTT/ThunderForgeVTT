import { test, expect } from "./fixtures/test";
import { openGmTool, waitForWallsLoaded } from "./fixtures/helpers";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  waitForEngineReady,
  clickCanvasAt,
} from "./fixtures/canvasAuthoring";

/**
 * T011-T015 (specs/002-canvas-authoring-asset-storage, closing T067's
 * hand-drawn-wall gap): a GM places a multi-point wall chain directly on
 * the canvas, toggles a segment into a door, deletes a segment, and can
 * cancel an in-progress chain with nothing persisted. Drives the engine
 * through real mouse/keyboard input on the canvas element rather than
 * WallTool's "Draw wall" toggle button, which only sets local UI state
 * today and isn't observed by the engine (a separate, pre-existing gap
 * noted but not fixed here — wall input has always been "always-on" for
 * a GM session regardless of that toggle).
 */
test.describe("Hand-drawn wall authoring (US1)", () => {
  test("a 3-point click chain, ended with Enter, creates a 2-segment wall that survives a reload", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await registerAndCreateWorld(page, `E2E Wall Chain ${uniqueSuffix()}`);
    await createScene(page, "Wall Chain Scene");
    await waitForEngineReady(page, "walls");
    await openGmTool(page, "walls");
    await expect(page.getByTestId("wall-tool")).toBeVisible();

    // FR-001: click three distinct points, then end the chain.
    await clickCanvasAt(page, -150, -120);
    await clickCanvasAt(page, 0, -120);
    await clickCanvasAt(page, 150, -120);
    await page.keyboard.press("Enter");

    // Existence proxy: clicking a segment's body selects it, surfacing
    // WallTool's "Selected wall" panel (there is no wall-count testid).
    await clickCanvasAt(page, -75, -120);
    await expect(page.getByText("Selected wall")).toBeVisible({
      timeout: 10_000,
    });

    await page.reload();
    await waitForEngineReady(page, "walls");
    // A reload refetches the scene's walls over a separate round trip, and a
    // click lands before they arrive. See `waitForWallsLoaded`.
    await waitForWallsLoaded(page, 1);

    // Both segments persisted, not just the first.
    await clickCanvasAt(page, -75, -120);
    await expect(page.getByText("Selected wall")).toBeVisible({
      timeout: 10_000,
    });
    await clickCanvasAt(page, 75, -120);
    await expect(page.getByText("Selected wall")).toBeVisible({
      timeout: 10_000,
    });
  });

  test("Escape mid-chain cancels the wall with nothing persisted", async ({
    page,
  }) => {
    test.setTimeout(180_000);
    await registerAndCreateWorld(page, `E2E Wall Cancel ${uniqueSuffix()}`);
    await createScene(page, "Wall Cancel Scene");
    await waitForEngineReady(page, "walls");

    await clickCanvasAt(page, -100, 40);
    await clickCanvasAt(page, 0, 40);
    await page.keyboard.press("Escape");

    await page.reload();
    await waitForEngineReady(page, "walls");
    await clickCanvasAt(page, -50, 40);
    await expect(page.getByText("Selected wall")).toHaveCount(0);
  });

  test("toggling a wall to a door and deleting it both persist", async ({
    page,
  }) => {
    // Three engine loads and a world of its own: a passing run was measured at
    // 29.8 seconds against the 30-second default this test used to run under,
    // and it timed out the first time the machine was a little slower.
    test.setTimeout(180_000);
    await registerAndCreateWorld(
      page,
      `E2E Wall Door Delete ${uniqueSuffix()}`,
    );
    await createScene(page, "Wall Door Scene");
    await waitForEngineReady(page, "walls");

    await clickCanvasAt(page, -100, 0);
    await clickCanvasAt(page, 100, 0);
    await page.keyboard.press("Enter");

    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected wall")).toBeVisible({
      timeout: 10_000,
    });

    // 'O' cycles door state (systems/wall.rs's handle_wall_keyboard_toggles).
    await page.keyboard.press("o");
    await expect(page.getByTestId("wall-tool")).toContainText(
      /door \(closed\)/i,
      {
        timeout: 10_000,
      },
    );

    await page.reload();
    await waitForEngineReady(page, "walls");
    // A reload refetches the scene's walls over a separate round trip, and a
    // click lands before they arrive. See `waitForWallsLoaded`.
    await waitForWallsLoaded(page, 1);
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByTestId("wall-tool")).toContainText(
      /door \(closed\)/i,
      {
        timeout: 10_000,
      },
    );

    await page.getByRole("button", { name: "Delete wall" }).click();
    await expect(page.getByText("Selected wall")).toHaveCount(0);

    await page.reload();
    await waitForEngineReady(page, "walls");
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected wall")).toHaveCount(0);
  });
});

/**
 * T012 (specs/002-canvas-authoring-asset-storage): a wall created in one
 * browser session must sync to a second, independent session viewing the
 * same scene (quickstart.md Scenario 1 step 6's "cross-session" half).
 *
 * Scope note / known verification gap: `apply_vision_occlusion`
 * (systems/wall.rs) only ever toggles a Bevy `Visibility` component —
 * there is no DOM/store signal for "is this token currently hidden from
 * this session's viewpoint." The natural way to assert it is a canvas
 * pixel check, and both `page.screenshot()` and `canvas.toDataURL()` come
 * back blank in this environment: Bevy's WebGL context here doesn't
 * preserve its drawing buffer (`preserveDrawingBuffer: true` is a
 * renderer-init flag, not a test concern), so neither Playwright's own
 * screenshot compositor nor an in-page canvas read captures a real frame.
 * Changing that is a rendering-pipeline decision with real performance
 * tradeoffs, out of scope for this fix. What *is* independently verified
 * here is the half that's actually specific to "cross-session" and that
 * a screenshot-based check would depend on anyway: the wall itself
 * genuinely propagates to a second live session (selectable there, same
 * as the first), which is the precondition `apply_vision_occlusion`
 * needs — session two's local WallSet has to contain the wall before its
 * local vision computation could possibly react to it. The visual
 * occlusion effect itself should be spot-checked manually or covered by
 * a future visual-regression setup with `preserveDrawingBuffer` enabled.
 */
test.describe("Wall sync across sessions (US1, T012)", () => {
  test("a wall created in one session is selectable in a second, independent session viewing the same scene", async ({
    browser,
  }) => {
    test.setTimeout(180_000);
    const contextA = await browser.newContext();
    const pageA = await contextA.newPage();
    await registerAndCreateWorld(pageA, `E2E Wall Sync ${uniqueSuffix()}`);
    await createScene(pageA, "Sync Scene");
    await waitForEngineReady(pageA, "walls");

    await clickCanvasAt(pageA, -100, 0);
    await clickCanvasAt(pageA, 100, 0);
    await pageA.keyboard.press("Enter");

    await clickCanvasAt(pageA, 0, 0);
    await expect(pageA.getByText("Selected wall")).toBeVisible({
      timeout: 10_000,
    });

    // Second, independent session (own cookies/localStorage, own Bevy
    // WASM instance, own WebSocket connection) reusing session one's
    // login via storageState — "two browser contexts" per T012, without
    // the added complexity of a full separate-user invite flow, which
    // isn't what this scenario is actually exercising (US4 covers
    // cross-account isolation; this is purely about sync propagation).
    const storageState = await contextA.storageState();
    const contextB = await browser.newContext({ storageState });
    const pageB = await contextB.newPage();
    await pageB.goto(pageA.url());
    await waitForEngineReady(pageB, "walls");

    await clickCanvasAt(pageB, 0, 0);
    await expect(pageB.getByText("Selected wall")).toBeVisible({
      timeout: 10_000,
    });

    await contextA.close();
    await contextB.close();
  });

  /**
   * Spec 005 T007 (FR-007, SC-002, quickstart Scenario 1 step 5): the author's
   * own change must not double-apply, flicker or revert when its own event
   * comes back down the subscription.
   *
   * The test above proves an event *reaches* the other session. This proves
   * the other half, and it is the half that fails quietly: an echo that
   * spawns a second wall, or clears and re-adds the first, looks fine in a
   * screenshot and wrong to a person drawing. The observable form is that the
   * author's own selection survives — a re-spawn drops the selected entity
   * and the panel with it.
   */
  test("a session's own wall does not double-apply, flicker or revert when its event echoes back", async ({
    browser,
  }) => {
    test.setTimeout(180_000);
    const context = await browser.newContext();
    const page = await context.newPage();
    await registerAndCreateWorld(page, `E2E Echo ${uniqueSuffix()}`);
    await createScene(page, "Echo Scene");
    await waitForEngineReady(page, "walls");

    await clickCanvasAt(page, -100, 0);
    await clickCanvasAt(page, 100, 0);
    await page.keyboard.press("Enter");

    await clickCanvasAt(page, 0, 0);
    const selected = page.getByText("Selected wall");
    await expect(selected).toBeVisible({ timeout: 10_000 });

    // The author's own event round-trips through Postgres, the broadcast
    // channel and back down this session's own subscription. Well past any
    // plausible delivery time, so "it had not arrived yet" cannot be what
    // makes this pass.
    await page.waitForTimeout(5_000);

    // Still selected: an echo that re-spawned the wall would have dropped the
    // selection, and one that spawned a second would leave this one selected
    // but the scene holding two.
    await expect(
      selected,
      "the author's own selection must survive their event echoing back",
    ).toBeVisible();

    // And exactly one wall exists. Asked of the server, because the engine's
    // own count is what a duplicate would corrupt — this is the independent
    // check on it. Two hops: the scene id is not in the URL.
    const cookies = await context.cookies();
    const csrf = cookies.find((cookie) => cookie.name === "csrf_token")?.value;
    const post = async (query: string, variables: unknown) => {
      const response = await page.request.post("/api/graphql", {
        headers: csrf ? { "x-csrf-token": csrf } : {},
        data: { query, variables },
      });
      return (await response.json()) as {
        data?: Record<string, unknown>;
        errors?: { message: string }[];
      };
    };

    const worldId = new URL(page.url()).pathname.split("/")[2];
    const scenes = await post(
      `query S($worldId: UUID!) { scenes(worldId: $worldId) { sceneId } }`,
      { worldId },
    );
    expect(
      scenes.errors,
      `scene lookup failed: ${JSON.stringify(scenes.errors)}`,
    ).toBeUndefined();

    const sceneIds = (
      (scenes.data?.scenes as { sceneId: string }[] | undefined) ?? []
    ).map((scene) => scene.sceneId);
    expect(sceneIds.length, "the world should have scenes").toBeGreaterThan(0);

    let total = 0;
    for (const sceneId of sceneIds) {
      const walls = await post(
        `query W($sceneId: UUID!) { walls(sceneId: $sceneId) { wallId } }`,
        { sceneId },
      );
      expect(
        walls.errors,
        `wall lookup failed: ${JSON.stringify(walls.errors)}`,
      ).toBeUndefined();
      total += ((walls.data?.walls as { wallId: string }[] | undefined) ?? [])
        .length;
    }

    expect(total, "one drawn wall must be one stored wall, not two").toBe(1);

    await context.close();
  });
});
