import { expect, test, type Page } from "./fixtures/test";
import {
  createToken,
  createWorldAndPlay,
  register,
  tokenPosition,
  waitForEngineReady,
  waitForTokenTrafficToSettle,
} from "./fixtures/offline";

/**
 * A board that dies says so (spec 070).
 *
 * The canvas keeps its last frame when the engine stops, so a dead board
 * looked exactly like a live one that ignored you. Both ways it can die are
 * made to happen for real here — the browser takes the graphics context, and
 * the engine panics — and what is asserted is what a person at the table is
 * told, and that reloading gives them their table back.
 */

async function openBoard(page: Page, name: string): Promise<void> {
  await register(page, name);
  await createWorldAndPlay(page, `E2E Stopped ${name} ${Date.now()}`);
  await waitForEngineReady(page);
}

test("losing the graphics context says the board stopped, and a reload brings the table back", async ({
  page,
}) => {
  test.setTimeout(4 * 60_000);
  await openBoard(page, "ctxlost");
  const tokenId = await createToken(page);
  await waitForTokenTrafficToSettle(page);
  const before = await tokenPosition(page, tokenId);
  expect(before, "the token is on the board").not.toBeNull();

  const notice = page.getByTestId("engine-stopped");
  await expect(notice).toBeHidden();

  // What a driver reset does, asked for through the extension that exists
  // to ask for it: the engine's own context, really lost.
  const lost = await page.evaluate(() => {
    const canvas = document.querySelector("canvas");
    const gl = canvas?.getContext("webgl2");
    const extension = gl?.getExtension("WEBGL_lose_context");
    extension?.loseContext();
    return Boolean(extension);
  });
  expect(lost, "the test must be able to reach the engine's context").toBe(
    true,
  );

  await expect(notice).toBeVisible({ timeout: 10_000 });
  await expect(notice).toHaveAttribute("data-reason", "context-lost");
  await expect(notice).toContainText("Nothing is lost");

  await page.getByTestId("engine-stopped-reload").click();
  await waitForEngineReady(page);
  await expect(notice).toBeHidden();
  await expect
    .poll(() => tokenPosition(page, tokenId), { timeout: 20_000 })
    .toEqual(before);
});

test("an engine panic says the board stopped", async ({ page }) => {
  test.setTimeout(4 * 60_000);
  await openBoard(page, "panic");

  const notice = page.getByTestId("engine-stopped");
  await expect(notice).toBeHidden();

  // The running engine, through the module the app loaded.
  const asked = await page.evaluate(async () => {
    const probe = (await import(
      /* @vite-ignore */ "/src/engine/bevy/sdkFaultProbe.ts"
    )) as typeof import("../src/engine/bevy/sdkFaultProbe");
    return probe.crashEngine();
  });
  // A release engine cannot be asked to crash, by design. Said out loud
  // rather than passed: the context-loss test above still ran.
  test.skip(!asked, "this engine build exports no debug_panic (release)");

  await expect(notice).toBeVisible({ timeout: 10_000 });
  await expect(notice).toHaveAttribute("data-reason", "crashed");
  await expect(page.getByTestId("engine-stopped-reload")).toBeVisible();
});
