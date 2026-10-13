import { expect, type Page } from "./test";
import type { WorldProbe } from "../../src/engine/world/probe";

/**
 * Canvas and token helpers shared by the `token-authoring-*.spec.ts` files
 * (specs/004-token-canvas-authoring, specs/006-token-authoring-polish).
 *
 * Moved here verbatim when `token-authoring.spec.ts` was split by feature, so
 * every one of its files drives the canvas exactly as the single file did.
 * Several of these resemble `canvasAuthoring.ts`'s helpers but are *not* the
 * same — `dragCanvas` settles 250 ms, not 80, and `waitForEngineReady` focuses
 * a different spot — so they stay separate.
 */

declare global {
  interface Window {
    __worldProbe?: WorldProbe;
  }
}

/**
 * A 1x1 PNG, as bytes. Small on purpose: this test is about the art
 * round-tripping through upload → storage → `photoUrl` → the panel, not
 * about image content, and the server transcodes whatever it is given to
 * WebP anyway.
 */
export const ONE_PIXEL_PNG = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
  "base64",
);

/**
 * Opens the GM tool rail's Tokens tool, where `TokenTool` now lives.
 *
 * It used to be an always-mounted floating panel, so selecting a token was
 * enough to reveal it. The rail renders one tool at a time, so the tab has
 * to be open before `token-tool` exists in the DOM at all.
 */
/** True for a server-backed token id (a UUID), as opposed to the engine's
 *  own demo tokens, which have no row to persist changes to. */
export const isPersistedTokenId = (id: string | null) =>
  id !== null && /^[0-9a-f-]{36}$/.test(id);

/** The world store's own view of itself; see `engine/world/probe.ts`. */
export async function probeState(page: Page) {
  return page.evaluate(() => {
    const state = window.__worldProbe?.state();
    if (!state) throw new Error("world probe unavailable (dev build only)");
    return state;
  });
}

/**
 * Selects a *persisted* token by clicking where it actually is.
 *
 * Two things make the obvious "click the canvas centre" wrong. Tokens do
 * not spawn at the world origin, so a centre click lands on empty canvas
 * and the engine emits a deselect — which from the DOM is indistinguishable
 * from a broken panel. And every token in a fresh scene spawns on the same
 * spot, one of them being the engine's demo token (`"player"`), which has
 * no server row: changes to it cannot survive a reload. So click where the
 * probe says the tokens are, and drag off the pile until a UUID-identified
 * one is selected.
 *
 * World units map 1:1 to pixels at the default zoom, origin at the canvas
 * centre, y pointing up.
 */
export async function selectPersistedToken(
  page: Page,
  cx: number,
  cy: number,
): Promise<{ id: string; x: number; y: number }> {
  const { tokens } = await probeState(page);
  const target = tokens[0];
  if (!target) throw new Error("no tokens in the world store to select");

  const clickAt = async (x: number, y: number) => {
    await page.mouse.move(cx + x, cy - y);
    await page.mouse.down();
    await page.waitForTimeout(80);
    await page.mouse.up();
    await page.waitForTimeout(500);
  };

  for (let attempt = 0; attempt < 4; attempt += 1) {
    await clickAt(target.x, target.y);
    const { selectedTokenId, tokens } = await probeState(page);
    if (isPersistedTokenId(selectedTokenId)) {
      // Reported rather than left for the caller to look up again. Callers
      // used to follow this with a "first persisted token in the store"
      // lookup, which is a different question — the first such token is only
      // the one just selected while the scene holds exactly one. It does not:
      // this
      // helper drags the engine's demo tokens off the pile to reach a
      // persisted one, and dragging them is what gives them server rows, so
      // by the time it succeeds there are several and the first is whichever
      // the store happens to list first. Anchoring a handle drag on that
      // token while a *different* one is selected grabs empty canvas, and
      // the resize silently does nothing.
      const selected = tokens.find((token) => token.id === selectedTokenId);
      if (!selected) {
        throw new Error(`selected token ${selectedTokenId} not in the store`);
      }
      return { id: selected.id, x: selected.x, y: selected.y };
    }
    if (selectedTokenId === null) continue;

    // Selected the demo token: drag it clear so the next click reaches
    // whatever is underneath it.
    await page.mouse.move(cx + target.x, cy - target.y);
    await page.mouse.down();
    await page.mouse.move(cx + target.x + 260, cy - target.y - 160, {
      steps: 12,
    });
    await page.mouse.up();
    await page.waitForTimeout(600);
  }
  throw new Error("could not select a persisted token");
}

/**
 * The persisted row for `tokenId`.
 *
 * Assertions used to read `tokens[0]`, which was fine when a scene held
 * exactly one token. It no longer does: clicking a stack emits an
 * `upsert_token` for every member, and the engine's own demo tokens
 * (`"player"`, `"token-1"`) are in that pile, so the mutation bridge
 * creates server rows for them the first time a test clicks. `tokens[0]`
 * is then whichever the server returns first, not the one under test.
 */
export function persistedToken<T extends { tokenId: string }>(
  tokens: T[],
  tokenId: string,
): T {
  const row = tokens.find((token) => token.tokenId === tokenId);
  if (!row) {
    throw new Error(
      `token ${tokenId} not among persisted rows: ${tokens.map((t) => t.tokenId).join(", ")}`,
    );
  }
  return row;
}

/**
 * Shifts a handle offset (which `resizeHandleScreenOffset` and
 * `rotateHandleScreenOffset` express relative to the token) onto the
 * token's actual screen position.
 *
 * Those helpers, and `dragTokenHandle`, are all written against the canvas
 * centre because tokens were assumed to sit at the world origin. They do
 * not, so every handle grab has to be offset by where the token really is
 * or it grabs empty canvas.
 */
export function anchoredAt(
  anchor: { dx: number; dy: number },
  offset: { dx: number; dy: number },
): { dx: number; dy: number } {
  return { dx: anchor.dx + offset.dx, dy: anchor.dy + offset.dy };
}

/**
 * The token the engine currently reports as selected.
 *
 * `firstPersistedToken` is not enough on its own once a scene has a stack:
 * `TokenPanel` creates at (0, 0) and grid snapping lands every new token
 * on exactly (-192.5, -62.5), which is also where the engine's own demo
 * token sits. Probe order does not say which of that pile a click or drag
 * will actually reach — only the selection does.
 */
export async function selectedToken(
  page: Page,
): Promise<{ id: string; x: number; y: number }> {
  const { tokens, selectedTokenId } = await probeState(page);
  const token = tokens.find((candidate) => candidate.id === selectedTokenId);
  if (!token)
    throw new Error(`selected token ${selectedTokenId} not in the world store`);
  return { id: token.id, x: token.x, y: token.y };
}

export async function openTokenTool(page: Page): Promise<void> {
  const tokenTool = page.getByTestId("token-tool");
  if (await tokenTool.isVisible().catch(() => false)) {
    return;
  }
  await page.getByTestId("gm-tool-tokens").click();
}

export type Box = { x: number; y: number; width: number; height: number };

/** See canvasAuthoring.ts's similar helper for the full
 * rationale (Bevy mounts its canvas to `<body>`, not the named
 * container). */
export async function canvasBox(page: Page): Promise<Box> {
  const canvas = page.locator("canvas");
  await canvas.scrollIntoViewIfNeeded();
  const box = await canvas.boundingBox();
  if (!box) {
    throw new Error("Bevy canvas element not found");
  }
  return box;
}

/** See canvasAuthoring.ts's similar helper for the full
 * rationale (GM flag / bridge-ready / canvas-focus race). */
export async function waitForEngineReady(page: Page): Promise<void> {
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
    // Was the canvas's bottom-right corner, which the play dock's icon
    // rail is now painted over — the click landed on the dock and never
    // reached the engine, so the canvas never got focus and every
    // keyboard-driven assertion downstream silently did nothing. Inset
    // far enough to clear the rail (right) and the dice bar (bottom),
    // while staying clear of where tokens sit. A focusing click only: on
    // the select tool an Escape here would open the Settings pane over the
    // token panel's toggle (owner request 2026-10-06).
    await page.mouse.click(box.x + box.width - 200, box.y + 120);
    await page.waitForTimeout(200);
  }
}

/** See canvasAuthoring.ts's similar helper for the full
 * rationale (real click timing vs. synthetic same-frame down/up). */
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
  // The engine must get at least one frame with the cursor still at the
  // grab point. It computes the drag offset on `just_pressed` as
  // `tokenPos - cursorWorld`; if the press and the moves coalesce into one
  // frame, it first sees the cursor already at the destination, takes
  // `offset = tokenPos - destination`, and then every later frame places
  // the token at `destination + offset` — exactly where it started. The
  // drag silently does nothing, with no error anywhere to say so.
  await page.waitForTimeout(250);
  await page.mouse.move(cx + to.dx, cy + to.dy, { steps: 5 });
  await page.waitForTimeout(250);
  await page.mouse.up();
}

/** Screen-space offset (from canvas center) of the selected token's
 * resize handle at a given `scale`, matching
 * `crates/thunderforge-engine/src/systems/token.rs`'s `resize_handle_world_pos` exactly:
 * the token's bottom-right corner (`(half.x, -half.y)` in world space at
 * rotation 0), converted to screen space per `worldToScreenOffset`'s sign
 * convention (screen dy is the negation of world y). `TOKEN_SIZE` (96)
 * is duplicated from `lib.rs` — this file has no way to import Rust
 * consts, matching every other magic-number duplication already in this
 * suite (e.g. `screenOffsetToWorld`'s own coordinate-sign note).
 */
export function resizeHandleScreenOffset(scale: number): {
  dx: number;
  dy: number;
} {
  const half = (96 / 2) * scale;
  return { dx: half, dy: half };
}

/** Screen-space offset (from canvas center) of the selected token's
 * rotate handle at a given `scale` and `rotationRadians`, matching
 * `resize_handle_world_pos`'s sibling `rotate_handle_world_pos`: offset
 * `(TOKEN_SIZE/2 + 24) * scale` along the token's local +Y, rotated by
 * its current facing, then converted to screen space.
 */
export function rotateHandleScreenOffset(
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
export async function dragTokenHandle(
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

/** Creates a token via TokenPanel (position defaults to (0,0) — the panel's
 * create dialog has no x/y input, per TokenPanel.tsx's `handleCreateToken`)
 * and leaves the panel closed afterward so the canvas is unobstructed. */
export async function createTokenViaPanel(page: Page): Promise<void> {
  // The dock panel `createScene` opened stays open, and it covers the
  // bottom-right corner where the "Tokens" button and TokenPanel live —
  // the click still lands (it is forced) but the panel opens underneath
  // the dock and never becomes interactable. Collapse the dock first.
  const collapse = page.getByTestId("world-dock-collapse");
  if (await collapse.isVisible().catch(() => false)) {
    await collapse.click();
  }

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
export async function createTokenViaPanelCapturingId(
  page: Page,
): Promise<string> {
  // Same reason `createTokenViaPanel` does this, and the reason this
  // helper hung for its whole timeout without it: `createScene` leaves the
  // dock's Settings section open, and it covers the corner the "Tokens"
  // toggle sits in. The forced click then lands on the dock instead, the
  // popover never opens, and the wait below is for a `token-create-trigger`
  // that was never mounted — which reads as "creating a token is broken".
  const collapse = page.getByTestId("world-dock-collapse");
  if (await collapse.isVisible().catch(() => false)) {
    await collapse.click();
  }

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
  const body = (await response.json()) as {
    data?: { createToken?: { tokenId?: string } };
  };
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
export async function dragTokenAtOriginTo(
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
/**
 * The `grid_size` a scene gets when nothing else sets one
 * (`create_scenes_table/up.sql` — `grid_size INT NOT NULL DEFAULT 5`). Every
 * scene in this file is created through the UI with no map imported, so this
 * is the lattice its tokens land on.
 */
export const DEFAULT_SCENE_GRID_SIZE = 5;

/**
 * Where a token dropped at `world` actually comes to rest.
 *
 * Dragging a token snaps it to a cell *centre* (`GridSpec::snap` ->
 * `cell_center(world_to_cell(..))` in `thunderforge-canvas-core`), so a drag
 * aimed at (100, -100) persists as (102.5, -97.5) on the default 5-unit
 * grid. That is the product working: a VTT that let tokens rest between
 * cells would be the bug.
 *
 * The expectations here used to be the raw drag target, which held only
 * because the assertions they fed were reading a *different* scene (see
 * `createScene`) and so never compared these numbers to a real dragged
 * token. Snapping is applied to dragged positions only — a token that has
 * never been dragged keeps the (0, 0) `TokenPanel` created it at.
 */
export function snapWorld(world: { x: number; y: number }): {
  x: number;
  y: number;
} {
  const cell = DEFAULT_SCENE_GRID_SIZE;
  const center = (value: number) => Math.floor(value / cell) * cell + cell / 2;
  return { x: center(world.x), y: center(world.y) };
}

export function screenOffsetToWorld(offset: { dx: number; dy: number }): {
  x: number;
  y: number;
} {
  return { x: offset.dx, y: -offset.dy };
}

/** Inverse of `screenOffsetToWorld` — given a token's known world
 * position, returns the screen-space offset from canvas center needed
 * to click on it. */
export function worldToScreenOffset(position: { x: number; y: number }): {
  dx: number;
  dy: number;
} {
  return { dx: position.x, dy: -position.y };
}

/** Reads a specific token's persisted position from `TokenPanel`,
 * identified by its known `tokenId` (not list order/position, which
 * only `createTokenViaPanelCapturingId` gives us determinism over). */
export async function readTokenPosition(
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
export async function assignTokenOwnership(
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
    const primaryCheckbox = page.getByTestId(
      `token-primary-checkbox-${tokenId}`,
    );
    await expect(primaryCheckbox).toBeEnabled({ timeout: 10_000 });
    await primaryCheckbox.check({ force: true });
    await page.waitForTimeout(500);
  }

  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
}
