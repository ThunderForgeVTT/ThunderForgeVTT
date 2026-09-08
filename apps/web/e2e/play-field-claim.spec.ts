import { test, expect } from "@playwright/test";
import {
  clickPlay,
  freshCredentials,
  register,
  waitForEngineReady,
} from "./fixtures/helpers";
import { openAnotherClient } from "./fixtures/clients";

/**
 * Spec 036 US3a: one account, one table.
 *
 * An account may now be signed in several times (US1). This is the narrowing
 * that makes that useful rather than chaotic — exactly one of those clients is
 * the play field, and the others are companion surfaces. Two play fields for
 * one person would mean two engines, two event appliers and two peer
 * endpoints for one human being.
 *
 * Subscribing is claiming, so the claim lives exactly as long as the window
 * that made it: there is no timeout, and a window that is closed or crashes
 * releases the table without anything having to notice.
 */

async function openThePlayField(
  page: import("@playwright/test").Page,
  worldName: string,
  prefix: string,
) {
  const creds = freshCredentials(prefix);
  await register(page, creds);
  await page.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
  await page.locator("#world-name").fill(worldName);
  await page.getByRole("button", { name: /create world/i }).click();
  await page.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
  const worldId = /\/world\/([^/]+)\/staging$/.exec(
    new URL(page.url()).pathname,
  )?.[1];
  if (!worldId) {
    throw new Error(`could not read the world id from ${page.url()}`);
  }
  await clickPlay(page);
  await waitForEngineReady(page);
  return { creds, worldId };
}

test.describe("Spec 036 US3a: one account, one play field", () => {
  test("a second window takes the table, and the first is told it has become a companion", async ({
    page,
    browser,
  }) => {
    const { creds, worldId } = await openThePlayField(
      page,
      `Table ${Date.now().toString(36)}`,
      "e2eclaim",
    );

    // The window holding the table shows no notice — the assertion has to be
    // made before the takeover or it proves nothing about the takeover.
    await expect(page.getByTestId("play-field-taken-over-notice")).toHaveCount(
      0,
    );

    const second = await openAnotherClient(browser, creds, "context");
    await second.goto(`/world/${worldId}/play`);
    await waitForEngineReady(second);

    // The first window learns it was displaced on the same stream that made
    // it the holder, with no reload and no polling.
    await expect(page.getByTestId("play-field-taken-over-notice")).toBeVisible({
      timeout: 15_000,
    });

    // And the window that took it is not told it is a companion.
    await expect(
      second.getByTestId("play-field-taken-over-notice"),
    ).toHaveCount(0);

    await second.context().close();
  });

  test("closing the holder releases the table with no timeout to wait out", async ({
    page,
    browser,
  }) => {
    // Playwright's default budget is 30 seconds and this test declares three
    // waits of 30 seconds each on top of registering, creating a world and
    // loading the engine twice. A test whose own waits exceed its budget can
    // only ever fail at the budget, which reports the clock rather than the
    // claim — and says nothing about whether releasing works.
    test.setTimeout(3 * 60_000);

    const { creds, worldId } = await openThePlayField(
      page,
      `Released ${Date.now().toString(36)}`,
      "e2eclaimrel",
    );

    const second = await openAnotherClient(browser, creds, "context");
    await second.goto(`/world/${worldId}/play`);
    await waitForEngineReady(second);

    // Wait for the takeover to have actually happened before relying on it.
    // The banner is the person-facing half; this is the state behind it, and
    // asserting the state first means a slow claim reads as slow rather than
    // as a missing notice.
    await expect(second.getByTestId("play-field-status")).toHaveAttribute(
      "data-status",
      "holding",
      { timeout: 30_000 },
    );
    await expect(page.getByTestId("play-field-status")).toHaveAttribute(
      "data-status",
      "companion",
      { timeout: 30_000 },
    );

    // The holder goes away the way a real one does — the window closes.
    await second.context().close();

    // The first window can take the table straight back. If the claim were a
    // row with a heartbeat, this is where it would sit waiting for a reaper.
    //
    // `dispatchEvent` rather than `click`: the notice is a fixed banner over
    // the play view, and Playwright's actionability check does not settle
    // against the canvas beneath it — the same reason `ensureSidebarOpen`
    // dispatches rather than clicks the dock tab.
    await page.getByTestId("take-play-field-back").dispatchEvent("click");
    await expect(page.getByTestId("play-field-status")).toHaveAttribute(
      "data-status",
      "holding",
      { timeout: 30_000 },
    );
    await expect(page.getByTestId("play-field-taken-over-notice")).toHaveCount(
      0,
    );
  });
});
