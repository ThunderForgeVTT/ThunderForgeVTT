import type { CDPSession } from "@playwright/test";
import { expect, test, type Page } from "./fixtures/test";
import { boardToScreen } from "./fixtures/boardPointer";
import { camera, canvasBox } from "./fixtures/lightingProbe";
import {
  createToken,
  createWorldAndPlay,
  register,
  tokenPosition,
  waitForEngineReady,
  waitForTokenTrafficToSettle,
} from "./fixtures/offline";

/**
 * The board under fingers (spec 069).
 *
 * The engine read a mouse and nothing else, and a browser does not turn a
 * finger into one. These are real touches — dispatched to the page the way a
 * touchscreen's are, arriving at the canvas as touch pointer events — and what
 * is asserted is what a person at a tablet would see: the token went where
 * the finger took it, the map moved and zoomed under two, and holding still
 * opened the menu.
 */

type Point = { x: number; y: number };

/** One frame of fingers on the glass. An empty list lifts them all. */
async function touches(cdp: CDPSession, points: Point[]): Promise<void> {
  await cdp.send("Input.dispatchTouchEvent", {
    type: points.length === 0 ? "touchEnd" : "touchMove",
    touchPoints: points.map((point, id) => ({ ...point, id })),
  });
}

async function touchDown(cdp: CDPSession, points: Point[]): Promise<void> {
  await cdp.send("Input.dispatchTouchEvent", {
    type: "touchStart",
    touchPoints: points.map((point, id) => ({ ...point, id })),
  });
}

/** Slide every finger by the same amount, a frame at a time. */
async function slide(
  page: Page,
  cdp: CDPSession,
  from: Point[],
  to: Point[],
  steps = 8,
): Promise<void> {
  for (let i = 1; i <= steps; i += 1) {
    const t = i / steps;
    await touches(
      cdp,
      from.map((start, finger) => ({
        x: start.x + (to[finger].x - start.x) * t,
        y: start.y + (to[finger].y - start.y) * t,
      })),
    );
    // A frame each: moves coalesced into one are one move to the engine.
    await page.waitForTimeout(40);
  }
}

test("a board is played with fingers: drag a token, pan and pinch the map, hold for the menu", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);

  await register(page, "touch");
  await createWorldAndPlay(page, `E2E Touch ${Date.now()}`);
  await waitForEngineReady(page);
  const tokenId = await createToken(page);
  await waitForTokenTrafficToSettle(page);

  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setTouchEmulationEnabled", {
    enabled: true,
    maxTouchPoints: 5,
  });

  await test.step("one finger drags a token", async () => {
    const before = await tokenPosition(page, tokenId);
    expect(before, "the token is on the board").not.toBeNull();
    const from = await boardToScreen(page, before!);
    const to = { x: from.x + 192, y: from.y };

    await touchDown(cdp, [from]);
    // Held across a frame before it moves, as a mouse press must be: the
    // engine takes its grab offset on the frame the press lands.
    await page.waitForTimeout(250);
    await expect
      .poll(() =>
        page.evaluate(() => window.__worldProbe?.state().selectedTokenId),
      )
      .toBe(tokenId);
    await slide(page, cdp, [from], [to]);
    await page.waitForTimeout(200);
    await touches(cdp, []);

    await expect
      .poll(async () => (await tokenPosition(page, tokenId))?.x, {
        timeout: 10_000,
        message: "the token follows the finger east",
      })
      .toBeGreaterThan(before!.x + 100);
  });

  const box = await canvasBox(page);
  // Clear of the dock and the token, on bare board.
  const left = { x: box.x + box.width / 2 - 60, y: box.y + box.height - 220 };
  const right = { x: box.x + box.width / 2 + 60, y: box.y + box.height - 220 };

  await test.step("two fingers pan the map", async () => {
    const before = (await camera(page))!;
    const by = 150;
    await touchDown(cdp, [left, right]);
    await page.waitForTimeout(120);
    await slide(
      page,
      cdp,
      [left, right],
      [
        { x: left.x + by, y: left.y },
        { x: right.x + by, y: right.y },
      ],
    );
    await touches(cdp, []);

    // Fingers right, map right, so the camera looks further left — by the
    // distance the fingers went, at this zoom.
    await expect
      .poll(async () => (await camera(page))!.x, { timeout: 10_000 })
      .toBeCloseTo(before.x - by * before.scale, 0);
    expect((await camera(page))!.scale).toBeCloseTo(before.scale, 3);
  });

  await test.step("two fingers spreading zoom in", async () => {
    const before = (await camera(page))!;
    await touchDown(cdp, [left, right]);
    await page.waitForTimeout(120);
    await slide(
      page,
      cdp,
      [left, right],
      [
        { x: left.x - 60, y: left.y },
        { x: right.x + 60, y: right.y },
      ],
    );
    await touches(cdp, []);

    // Twice as far apart is twice as close: scale is world units per pixel.
    await expect
      .poll(async () => (await camera(page))!.scale, { timeout: 10_000 })
      .toBeCloseTo(before.scale / 2, 1);
  });

  await test.step("a finger held still on a token opens its menu", async () => {
    const at = await tokenPosition(page, tokenId);
    const on = await boardToScreen(page, at!);
    await touchDown(cdp, [on]);
    await page.waitForTimeout(900);
    await touches(cdp, []);

    await expect(page.getByTestId("canvas-menu")).toBeVisible({
      timeout: 10_000,
    });
    // And holding did not drag it anywhere.
    expect(await tokenPosition(page, tokenId)).toEqual(at);
  });
});
