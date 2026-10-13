import { expect, type Page } from "./test";
import type { WorldProbe } from "../../src/engine/world/probe";
import path from "node:path";
import { readFileSync } from "node:fs";
import {
  launchSceneByName,
  openDockTab,
  openGmTool,
  type GmToolId,
} from "./helpers";

/**
 * Helpers shared by the `canvas-authoring-*.spec.ts` files
 * (specs/001-bevy-canvas-authoring, specs/002-canvas-authoring-asset-storage).
 * Moved here verbatim when `canvas-authoring.spec.ts` was split by feature;
 * only the relative paths changed, since this file sits one directory deeper.
 */

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

export const DEMO_MAP = path.resolve(
  __dirname,
  "../../../../examples/maps/demo.dd2vtt",
);
export const CHAMBER_MAP = path.resolve(
  __dirname,
  "../../../../examples/maps/chamber-of-echoing-grief.dd2vtt",
);

export function uniqueSuffix(): string {
  return `${Date.now().toString(36)}${Math.random().toString(36).slice(2, 8)}`;
}

export async function registerAndCreateWorld(
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
export async function createScene(page: Page, name: string): Promise<void> {
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

export async function importMap(
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

export async function switchToScene(page: Page, name: string): Promise<void> {
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

export type Box = { x: number; y: number; width: number; height: number };

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
export async function canvasBox(page: Page): Promise<Box> {
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
export async function waitForEngineReady(
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

  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (box) {
    // Deliberately a corner far from where test geometry is placed
    // (offsets used elsewhere in this file stay within roughly ±220 of
    // center), so this focusing click can't accidentally select/move
    // something a test cares about. It happens under the select tool,
    // before any tool opens, so it adds no wall-chain point; it used to
    // come after, with an Escape to discard that point, and Escape now
    // returns to the select tool (owner request 2026-10-06). Keys reach the
    // engine from the window (`engine/canvasKeyboard.ts`), so the rail
    // button that opens the tool may keep focus.
    await page.mouse.click(box.x + box.width - 40, box.y + box.height - 40);
    await page.waitForTimeout(200);
  }

  if (tool) {
    await openGmTool(page, tool);
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
export async function clickCanvasAt(
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
export async function dragCanvas(
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

/**
 * Assert the launched scene's grid and background agree with the source file.
 *
 * Reads the scene back through the app's own GraphQL rather than inspecting
 * the canvas: the claim is about what was *stored*, and a pixel-level check on
 * a WebGL canvas would be measuring the renderer instead.
 */
/**
 * Playtest 2026-09-10 P9: imported walls are on the map.
 *
 * A UVTT file counts from the image's top-left corner with y down; the engine
 * draws the background centred on the origin with y up. The importer only
 * scaled, so every imported wall, door and light sat half a map away from the
 * art, mirrored — and shaded the wrong part of it. On the map means inside
 * the background's rectangle, and on both sides of its middle.
 */
export async function expectImportedWallsOnTheMap(page: Page): Promise<void> {
  const worldId = /\/world\/([^/]+)\//.exec(new URL(page.url()).pathname)?.[1];
  if (!worldId) throw new Error(`No world id in ${page.url()}`);

  const { walls, width, height } = await page.evaluate(async (worldId) => {
    const engine = (await import(
      /* @vite-ignore */ "/src/engine/bevy/index.ts"
    )) as typeof import("../../src/engine/bevy/index");
    const state = engine.getBoundWorldStore()?.getState();
    const token = document.cookie
      .split("; ")
      .find((c) => c.startsWith("csrf_token="))
      ?.split("=")[1];
    // The scene the map was imported into: the one with a background, as
    // `expectSceneGridMatchesMap` finds it.
    const response = await fetch("/api/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...(token ? { "x-csrf-token": token } : {}),
      },
      body: JSON.stringify({
        query: `query S($worldId: UUID!) { scenes(worldId: $worldId) { width height backgroundAssetId } }`,
        variables: { worldId },
      }),
    });
    const body = await response.json();
    const scenes = (body?.data?.scenes ?? []) as {
      width?: number;
      height?: number;
      backgroundAssetId?: string | null;
    }[];
    const imported = scenes.find((s) => s.backgroundAssetId);
    return {
      walls: Object.values(state?.walls ?? {}).map((w) => [
        w.x1,
        w.y1,
        w.x2,
        w.y2,
      ]),
      width: imported?.width ?? 0,
      height: imported?.height ?? 0,
    };
  }, worldId);

  expect(walls.length, "the import created walls").toBeGreaterThan(0);
  expect(width).toBeGreaterThan(0);
  const xs = walls.flatMap(([x1, , x2]) => [x1, x2]);
  const ys = walls.flatMap(([, y1, , y2]) => [y1, y2]);
  for (const x of xs) expect(Math.abs(x)).toBeLessThanOrEqual(width / 2 + 1);
  for (const y of ys) expect(Math.abs(y)).toBeLessThanOrEqual(height / 2 + 1);
  expect(
    Math.min(...xs) < 0 && Math.max(...xs) > 0,
    "walls on both sides of the map's middle, left to right",
  ).toBe(true);
  expect(
    Math.min(...ys) < 0 && Math.max(...ys) > 0,
    "walls on both sides of the map's middle, top to bottom",
  ).toBe(true);
}

export async function expectSceneGridMatchesMap(
  page: Page,
  fixturePath: string,
): Promise<void> {
  const file = JSON.parse(readFileSync(fixturePath, "utf8")) as {
    resolution: { map_size: { x: number; y: number } };
  };

  const worldId = /\/world\/([^/]+)\//.exec(new URL(page.url()).pathname)?.[1];
  if (!worldId) throw new Error(`No world id in ${page.url()}`);

  const scene = await page.evaluate(async (worldId) => {
    const token = document.cookie
      .split("; ")
      .find((c) => c.startsWith("csrf_token="))
      ?.split("=")[1];
    const response = await fetch("/api/graphql", {
      method: "POST",
      credentials: "same-origin",
      headers: {
        "Content-Type": "application/json",
        ...(token ? { "x-csrf-token": token } : {}),
      },
      body: JSON.stringify({
        query: `query S($worldId: UUID!) { scenes(worldId: $worldId) { gridSize width height backgroundAssetId backgroundGridMismatch } }`,
        variables: { worldId },
      }),
    });
    const body = await response.json();
    const scenes = (body?.data?.scenes ?? []) as {
      gridSize?: number;
      width?: number;
      height?: number;
      backgroundAssetId?: string | null;
      backgroundGridMismatch?: string | null;
    }[];
    return scenes.find((s) => s.backgroundAssetId) ?? null;
  }, worldId);

  expect(
    scene,
    "a scene with an imported background should exist",
  ).not.toBeNull();
  const { gridSize, width, height } = scene!;
  expect(gridSize).toBeGreaterThan(0);

  const cellsX = width! / gridSize!;
  const cellsY = height! / gridSize!;
  expect(
    Math.abs(cellsX - file.resolution.map_size.x),
    `scene is ${cellsX.toFixed(2)} cells wide (${width}px / ${gridSize}px) but the file says ${file.resolution.map_size.x}`,
  ).toBeLessThan(0.5);
  expect(
    Math.abs(cellsY - file.resolution.map_size.y),
    `scene is ${cellsY.toFixed(2)} cells tall (${height}px / ${gridSize}px) but the file says ${file.resolution.map_size.y}`,
  ).toBeLessThan(0.5);

  // And the server agrees, through the field the GM's notice is drawn from.
  // Asserting the ratio alone would leave the detector free to be broken in
  // the opposite direction — silent about a scene that really is misaligned,
  // or crying wolf over one that is not.
  expect(
    scene!.backgroundGridMismatch ?? null,
    "a correctly imported map must not be reported as misaligned",
  ).toBeNull();
  await expect(page.getByTestId("map-import-grid-mismatch")).toHaveCount(0);
}
