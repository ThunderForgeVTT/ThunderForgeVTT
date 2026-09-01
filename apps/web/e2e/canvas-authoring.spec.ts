import { test, expect, type Page } from "@playwright/test";
import path from "node:path";
import {
  launchSceneByName,
  openDockTab,
  openGmTool,
  waitForShapesLoaded,
  waitForWallsLoaded,
  type GmToolId,
} from "./fixtures/helpers";

/**
 * End-to-end coverage for the native canvas authoring feature
 * (specs/001-bevy-canvas-authoring) plus the scene-switching work built
 * on top of it: a GM imports a Universal VTT map into one scene, creates
 * a second scene, imports a different map into it, and switches between
 * the two — proving map import and per-scene isolation both work through
 * the real UI, not just at the API/unit level.
 *
 * Fixtures: examples/maps/demo.dd2vtt (8 line_of_sight polygons -> 31
 * walls, 2 doors, 12 lights) and examples/maps/chamber-of-echoing-grief.dd2vtt
 * (1 polygon -> 4 walls, 0 doors, 0 lights) — counts verified directly
 * against the parser in src/server/src/map_import.rs's own tests, and
 * asserted here via the exact UI text MapImportTool renders after a
 * successful import (no backdoor API calls — this is what a GM sees).
 */

const DEMO_MAP = path.resolve(__dirname, "../../../examples/maps/demo.dd2vtt");
const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

async function registerAndCreateWorld(
  page: Page,
  worldName: string,
): Promise<void> {
  const suffix = uniqueSuffix();
  const username = `e2e${suffix}`;
  const email = `${username}@example.test`;
  const password = "Sup3r-Secret-Passphrase!";

  await page.goto("/register");
  await page.locator("#register-username").fill(username);
  await page.locator("#register-email").fill(email);
  await page.locator("#register-password").fill(password);
  await page.locator("#register-password-confirmation").fill(password);
  await page.getByRole("button", { name: "Create account" }).click();

  // Registration logs the user in and redirects away from /register.
  await page.waitForURL((url) => !url.pathname.startsWith("/register"), {
    timeout: 15_000,
  });

  await page.goto("/worlds/create");
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();

  // Spec 010: CreateWorldPage navigates to /world/{id}/staging (not the
  // canvas directly, and not the dashboard) — click "Play" to reach the
  // full-screen canvas at /world/{id}/play.
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  await page.getByTestId("play-button").click();
  await page.waitForURL(/\/world\/[^/]+\/play$/, { timeout: 15_000 });
}

/**
 * Create a scene and switch to it.
 *
 * # Why this navigates first
 *
 * `SceneSwitcher` — and with it the "New scene" button — is mounted in exactly
 * one place: the Settings section of the play view's right-hand dock. It is
 * not on the staging route at all. `registerAndCreateWorld` leaves the page on
 * `/staging`, so a spec calling this straight afterwards is on a page where
 * scene creation does not exist.
 *
 * The previous version checked whether the button happened to be visible and,
 * if not, clicked a dock tab. On staging neither exists, so it waited out the
 * full test timeout on a tab that was never going to appear — a hang that
 * reads like a broken app rather than a helper looking on the wrong page.
 */
async function createScene(page: Page, name: string): Promise<void> {
  const path = new URL(page.url()).pathname;
  const worldId = /\/world\/([^/]+)/.exec(path)?.[1];
  if (!worldId) {
    throw new Error(`createScene needs to be on a world route, not ${path}`);
  }
  if (!/\/play$/.test(path)) {
    await page.goto(`/world/${worldId}/play`);
  }

  await openDockTab(page, "settings");
  const newSceneButton = page.getByTestId("new-scene-button");
  await expect(newSceneButton).toBeVisible({ timeout: 15_000 });
  await newSceneButton.click();

  await page.locator('[data-testid="new-scene-name-input"]:visible').fill(name);
  await page.locator('[data-testid="create-scene-submit"]:visible').click();

  await expect(page.getByTestId("new-scene-name-input")).toBeHidden({
    timeout: 10_000,
  });
  await expect(
    page.locator('[data-testid="scene-switcher"]:visible'),
  ).toContainText(name);

  // Make it the world's *active* scene, not merely this client's selection.
  //
  // Which scene a reload lands on is server state (ADR-046, spec 022), and
  // creating one through the switcher does not launch it. Without this, a test
  // draws on the new scene, reloads, and is silently returned to the previous
  // one — where its walls do not exist. That reads exactly like "walls do not
  // survive a reload", and cost a long session to tell apart from it.
  await launchSceneByName(page, worldId, name);
  await page.goto(`/world/${worldId}/play`);
}

async function importMap(
  page: Page,
  filePath: string,
  expectedSummary: string,
): Promise<number> {
  // Map import lives in the Settings section of the right-hand dock. Setting
  // files on a hidden input succeeds, so skipping this produces the confusing
  // shape where the import runs and its success panel is never observable.
  await openDockTab(page, "settings");
  const tool = page.getByTestId("map-import-tool");
  const startedAt = Date.now();
  await tool.locator('input[type="file"]').setInputFiles(filePath);

  const success = page.getByTestId("map-import-success");
  // Deliberately far longer than SC-007's own 30-second budget, which is
  // asserted separately on the returned duration. If the import overruns, the
  // useful failure is "it took 34s" — not a locator timeout on a spinner that
  // was still spinning, which is what a 30-second wait here produced and which
  // reads like a hung upload rather than a missed budget.
  await expect(success).toBeVisible({ timeout: 120_000 });
  await expect(success).toContainText(expectedSummary);
  return Date.now() - startedAt;
}

async function switchToScene(page: Page, name: string): Promise<void> {
  // The switcher lives in the dock's Settings section, which is collapsed
  // again after every navigation `createScene`/`launchSceneByName` performs.
  // Without this the `:visible` locator below simply never resolves, and the
  // click sits there until the test times out.
  await openDockTab(page, "settings");
  // Spec 009: both the (hidden, if in full-screen mode) staging page and
  // the sidebar render their own scene switcher — scope to the visible one.
  const sceneSwitcher = page.locator('[data-testid="scene-switcher"]:visible');
  await sceneSwitcher.click();
  await page.getByRole("option", { name }).click();
  await expect(sceneSwitcher).toContainText(name);
}

type Box = { x: number; y: number; width: number; height: number };

/**
 * The Bevy canvas element, located directly rather than scoped under
 * `#game-canvas-container`: bevy_winit appends its `<canvas>` to
 * `document.body` rather than into the container div the `canvasSelector`
 * option names (a pre-existing, unrelated DOM-mounting gap — confirmed
 * present with all of this feature's wall/shape/engine changes reverted
 * too — which is also why `useCanvasEngine`'s own `container.querySelector
 * ('canvas')` polling always misses and logs "Canvas not found after 5
 * seconds"). There is exactly one `<canvas>` on this page at a time, so
 * `page.locator("canvas")` unambiguously finds it regardless of parentage.
 */
async function canvasBox(page: Page): Promise<Box> {
  const canvas = page.locator("canvas");
  // Being a `<body>` child in normal document flow (see the doc comment
  // above), the canvas often renders below the fold — `boundingBox()`
  // alone returns its true page position, which can fall outside the
  // viewport entirely and make raw `page.mouse` coordinates land nowhere.
  // Scroll it into view first so click math stays within the viewport.
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("Bevy canvas element not found");
  }
  return box;
}

/**
 * Wall/shape authoring input requires three things to land before any
 * click/keypress does anything real (specs/002-canvas-authoring-asset-
 * storage T014/T015):
 * 1. `set_is_game_master` (WorldPage.tsx -> engine bridge) — previously
 *    nothing ever set this and no GM could author anything at all.
 * 2. `bridgeReady` (WorldPage.tsx) — `bindWorldStore`'s subscription,
 *    which forwards confirmed server data (e.g. reloaded walls) into the
 *    engine, is reached through its own dynamic-import hop that
 *    `engineReady` doesn't wait on; without gating on it, scene data
 *    loaded on mount/reload can lose the race and never reach the
 *    engine's WallSet/ShapeSet even though it's genuinely persisted.
 * 3. Keyboard focus on the canvas element itself. Bevy's winit web
 *    backend only receives keydown events targeted at the canvas; a
 *    fresh page load never focuses it, so a keyboard-only interaction
 *    (e.g. a shape tool's `1`-`5` digit-key selection, tried before any
 *    mouse click has incidentally focused the canvas) is silently
 *    dropped — confirmed by instrumenting `handle_shape_tool_selection`:
 *    zero `just_pressed` events reached it pre-click, all of them
 *    post-click. Wall tests never hit this because every wall
 *    interaction starts with a mouse click anyway.
 * No dedicated "ready" testid exists for any of these, so wait for the
 * canvas element, click a neutral corner of it (focus + settle for 1-2),
 * plus an extra settle window.
 */
async function waitForEngineReady(
  page: Page,
  /**
   * Which GM tool to leave open, for tests that go on to assert its panel.
   *
   * Needed on every call that precedes a tool assertion, including after a
   * reload: the rail mounts only the open tool's content, and a reload closes
   * it. Passing it here rather than as a separate call keeps the two things
   * that must happen together in one place.
   */
  tool?: GmToolId,
): Promise<void> {
  const canvas = page.locator("canvas");
  // Spec 010: `/world/:id/play` is a real route now, not a client-state
  // toggle — a reload keeps the same URL, so the canvas is simply still
  // mounting (WASM engine startup) and no staging "Play" button will
  // ever appear here. Only click Play when we're actually still on the
  // staging route (a one-shot `isVisible()` check on the canvas used to
  // stand in for this, but it raced the canvas's own mount and could
  // misfire a click on a "play-button" that doesn't exist on /play,
  // hanging for the full timeout).
  if (/\/staging$/.test(new URL(page.url()).pathname)) {
    await page.getByTestId("play-button").click({ timeout: 15_000 });
  }
  await expect(canvas).toBeVisible({ timeout: 15_000 });
  await page.waitForTimeout(3_000);

  // Opened *before* the focusing click below, not after. Clicking a rail
  // button leaves keyboard focus on it, and this file drives the engine's
  // shape sub-tools with bare number keys — which never reach it if a DOM
  // button is holding focus. The corner click restores focus to the canvas.
  if (tool) {
    await openGmTool(page, tool);
  }

  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (box) {
    // Deliberately a corner far from where test geometry is placed
    // (offsets used elsewhere in this file stay within roughly ±220 of
    // center), so this focusing click can't accidentally select/move
    // something a test cares about. It still counts as a wall-chain
    // click-without-drag though (systems/wall.rs's FR-001 point-add), so
    // press Escape right after to discard that phantom single-point
    // "chain" — otherwise a test's real first chain click would silently
    // become its second point.
    await page.mouse.click(box.x + box.width - 40, box.y + box.height - 40);
    await page.keyboard.press("Escape");
    await page.waitForTimeout(200);
  }
}

/**
 * Clicks at a pixel offset from the canvas center. Deliberately not
 * `page.mouse.click()`: Bevy's per-frame `ButtonInput` only sets
 * `just_pressed`/`just_released` from window events it has actually
 * polled, and a zero-delay synthetic mousedown+mouseup pair can both land
 * within the same animation frame — collapsing to `just_pressed` only
 * (whose handlers all early-`return`, so `just_released`'s handling,
 * e.g. this feature's wall-chain point-add, never runs for that click). A
 * real human click is tens of milliseconds long, spanning multiple 16ms
 * frames, so this never happens outside automation. The explicit small
 * delay between down/up guarantees at least one frame boundary between
 * them, matching real click timing.
 */
async function clickCanvasAt(
  page: Page,
  dx: number,
  dy: number,
): Promise<void> {
  const box = await canvasBox(page);
  const x = box.x + box.width / 2 + dx;
  const y = box.y + box.height / 2 + dy;
  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.waitForTimeout(80);
  await page.mouse.up();
}

/**
 * Click-drags from one canvas-center-relative offset to another. See
 * `clickCanvasAt`'s doc comment for why the explicit delay before
 * `mouse.up()` matters — same frame-collapsing risk applies to a
 * zero-delay synthetic drag.
 */
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
  // Settle before moving off the start point, for the mirror image of the
  // reason `clickCanvasAt` settles before releasing. Bevy reads the drag's
  // origin as `window.cursor_position()` in whichever frame `just_pressed`
  // lands, and a zero-delay `down` followed immediately by a stepped move
  // pushes several CursorMoved events into that same frame — so the origin
  // recorded was the *first interpolation step*, not where the drag began.
  // A rectangle drawn from -60 to +60 in five steps came back 96 units wide
  // starting at -36 (exactly one 24-unit step in), putting its anchor at
  // (12, -12) instead of the origin — more than SHAPE_SELECT_DISTANCE's 8
  // units from where the test then clicked, so nothing was ever selected.
  // That reads as "shape selection is broken" and is really an off-by-one-
  // frame in the synthetic drag; real drags never begin this fast.
  await page.waitForTimeout(80);
  await page.mouse.move(cx + to.dx, cy + to.dy, { steps: 5 });
  await page.waitForTimeout(80);
  await page.mouse.up();
}

test.describe("Native canvas authoring: map import and scene switching", () => {
  test("importing a map into one scene, creating a second scene, importing a different map, and switching between them keeps each scene's content isolated", async ({
    page,
  }) => {
    // Two full map imports (each with a 30-second SC-007 budget of its own),
    // three scene launches and four scene switches do not fit in Playwright's
    // 30-second default — the test was dying mid-upload, which reads like a
    // broken import rather than a clock running out.
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E World ${uniqueSuffix()}`);

    // "New scene" lives in the Play dock's Settings section, collapsed by
    // default — open it before the rest of this test relies on it being
    // reachable via `createScene`/`switchToScene`.
    await page.getByTestId("world-dock-tab-settings").click();
    await expect(
      page.locator('[data-testid="new-scene-button"]:visible'),
    ).toBeVisible();

    // --- Scene One: import the rich demo map ---
    //
    // `createScene` finishes by launching the scene, which is a real
    // navigation — the dock comes back collapsed, so every assertion about
    // its Settings contents has to open it again first. Asserting the import
    // tool without doing so found nothing and read like the tool being gone.
    await createScene(page, "Scene One");
    await openDockTab(page, "settings");
    await expect(page.getByTestId("map-import-tool")).toBeVisible();
    // SC-007: importing demo.dd2vtt (background art + 8 wall polygons + 2
    // doors + 12 lights) end-to-end through the real UI must complete in
    // under 30 seconds.
    const importDurationMs = await importMap(
      page,
      DEMO_MAP,
      "31 walls, 2 doors, 12 lights",
    );
    // Measured live at 28-34s against the dev stack, i.e. right on the line
    // and failing about as often as it passes.
    // Nearly all of it is `save_background_image`/`save_scene_preview_image`
    // decoding and transcoding demo.dd2vtt's ~3MB embedded PNG in a `cargo
    // run` *debug* backend, where the `image`/`webp` codecs run unoptimised:
    // the 958KB chamber map, same code path, lands in ~2.9s. The product this
    // budget is about is the release build, so a `[profile.dev.package.image]
    // opt-level` bump would make this measurement representative rather than
    // marginal — a build-config change, out of scope here, recorded so a
    // borderline failure is read as the debug backend and not a regression.
    expect(importDurationMs).toBeLessThan(30_000);
    console.log(`SC-007: demo.dd2vtt import took ${importDurationMs}ms`);

    // --- Scene Two: import the simpler chamber map ---
    await createScene(page, "Scene Two");
    // A freshly created scene must not show the previous scene's import
    // result (per-scene isolation, not a global "last import" banner).
    // Open Settings first: with the dock collapsed the success panel is not
    // mounted at all, so this would pass without proving anything.
    await openDockTab(page, "settings");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);
    await importMap(page, CHAMBER_MAP, "4 walls, 0 doors, 0 lights");

    // --- Switch back to Scene One: its own import result, not Scene Two's ---
    await switchToScene(page, "Scene One");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);

    // --- And back to Scene Two once more, to rule out one-way state bleed ---
    await switchToScene(page, "Scene Two");
    await expect(page.getByTestId("map-import-success")).toHaveCount(0);
  });
});

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
});

/**
 * T016-T020 (specs/002-canvas-authoring-asset-storage, closing T067's
 * hand-drawn-shape gap): freehand/rectangle/ellipse/line/text shape
 * creation, scene-switch isolation, and delete. Uses the engine's own
 * `1`-`5` keyboard shortcuts (`handle_shape_tool_selection`,
 * systems/shape.rs) to pick a sub-tool rather than ShapeTool.tsx's
 * toolbar buttons: those buttons only set local React UI state today and
 * were never wired to the engine's `ActiveShapeTool` resource (the same
 * category of gap `WallTool.tsx`'s "Draw wall" toggle has, pre-existing,
 * not fixed here — out of this pass's file scope). The Text sub-tool is
 * the one exception with a genuinely working end-to-end UI path
 * (`ShapeTool.tsx`'s own click-to-place popover), so it's driven through
 * its real button instead.
 */
test.describe("Hand-drawn shape authoring (US2)", () => {
  test("freehand, rectangle, ellipse, line/arrow, and text shapes can each be created and selected", async ({
    page,
  }) => {
    // Registration, world creation, a scene launch (a full navigation) and
    // the engine's own multi-second WASM startup do not leave room inside
    // Playwright's 30-second default for the drawing this test is about — it
    // was dying mid-setup, which looks like a hang rather than a clock.
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E Shapes ${uniqueSuffix()}`);
    await createScene(page, "Shape Scene");
    await waitForEngineReady(page, "shapes");
    await expect(page.getByTestId("shape-tool")).toBeVisible();

    // Freehand (key 1): a short multi-point drag.
    await page.keyboard.press("1");
    await dragCanvas(page, { dx: -220, dy: -150 }, { dx: -180, dy: -110 });

    // Rectangle (key 2): bounding box (-60,-150)-(20,-100), anchor (center)
    // at (-20,-125).
    await page.keyboard.press("2");
    await dragCanvas(page, { dx: -60, dy: -150 }, { dx: 20, dy: -100 });

    // Ellipse (key 3): bounding box (100,-150)-(200,-100), anchor at (150,-125).
    await page.keyboard.press("3");
    await dragCanvas(page, { dx: 100, dy: -150 }, { dx: 200, dy: -100 });

    // Line/arrow (key 4): anchor is the segment midpoint, (-150,75).
    await page.keyboard.press("4");
    await dragCanvas(page, { dx: -200, dy: 50 }, { dx: -100, dy: 100 });

    // Confirm the drag-drawn shapes are each selectable (existence proxy,
    // same technique the wall tests use — there is no shape-count testid).
    // Escape first to leave the ellipse/line drag-tool mode and drop back
    // to plain select-by-click (systems/shape.rs's handle_shape_tool_selection).
    await page.keyboard.press("Escape");
    await clickCanvasAt(page, -20, -125); // rectangle center
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    await clickCanvasAt(page, 150, -125); // ellipse center
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    await clickCanvasAt(page, -150, 75); // line midpoint
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });

    // Text: the one sub-tool with a real, already-working UI path —
    // ShapeTool.tsx's own click-to-place popover, not engine tool state.
    await page.getByRole("button", { name: "Text" }).click();
    const box = await canvasBox(page);
    await page.mouse.click(
      box.x + box.width / 2 + 100,
      box.y + box.height / 2 + 100,
    );
    await expect(page.getByTestId("shape-text-popover")).toBeVisible();
    await page.getByLabel("Text").fill("Trap!");
    await page.getByRole("button", { name: "Add text" }).click();
    await expect(page.getByTestId("shape-text-popover")).toBeHidden();
  });

  test("shapes are isolated per scene across 3 scene switches, and deleting one removes it", async ({
    page,
  }) => {
    // Registration, world creation, a scene launch (a full navigation) and
    // the engine's own multi-second WASM startup do not leave room inside
    // Playwright's 30-second default for the drawing this test is about — it
    // was dying mid-setup, which looks like a hang rather than a clock.
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E Shape Isolation ${uniqueSuffix()}`);
    await createScene(page, "Shape Scene A");
    await waitForEngineReady(page, "shapes");

    await page.keyboard.press("2");
    await dragCanvas(page, { dx: -60, dy: -60 }, { dx: 60, dy: 60 });
    await page.keyboard.press("Escape");
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });

    await createScene(page, "Shape Scene B");
    await waitForEngineReady(page, "shapes");
    // A freshly created scene must not show Scene A's rectangle.
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toHaveCount(0);

    // Switch A -> B -> A -> B -> A (>= 3 switches, per SC-003) confirming
    // isolation holds each time, not just once.
    for (let i = 0; i < 3; i++) {
      await switchToScene(page, "Shape Scene A");
      await waitForEngineReady(page, "shapes");
      // A scene switch refetches that scene's shapes over their own round
      // trip; a click that lands first selects nothing. See
      // `waitForShapesLoaded`.
      await waitForShapesLoaded(page, 1);
      await clickCanvasAt(page, 0, 0);
      await expect(page.getByText("Selected shape")).toBeVisible({
        timeout: 10_000,
      });

      await switchToScene(page, "Shape Scene B");
      await waitForEngineReady(page, "shapes");
      await clickCanvasAt(page, 0, 0);
      await expect(page.getByText("Selected shape")).toHaveCount(0);
    }

    // Delete: back on Scene A, remove the rectangle and confirm it's gone.
    await switchToScene(page, "Shape Scene A");
    await waitForEngineReady(page, "shapes");
    await waitForShapesLoaded(page, 1);
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    await page.getByRole("button", { name: "Delete shape" }).click();
    await expect(page.getByText("Selected shape")).toHaveCount(0);

    await page.reload();
    await waitForEngineReady(page, "shapes");
    // No `waitForShapesLoaded` here, deliberately: the scene's only shape is
    // the one just deleted, so polling for one to arrive would wait out its
    // whole timeout on something that must never come back. The engine-ready
    // settle is what gives the refetch its chance to land.
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toHaveCount(0);
  });
});
