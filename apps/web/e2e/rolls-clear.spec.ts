import { expect, test, type Page } from "./fixtures/test";
import { graphql } from "./fixtures/helpers";
import {
  dicePlayed,
  openChat,
  rollFromRoller,
  seatRollTable,
  waitForPlayView,
} from "./fixtures/rolls";

/**
 * Spec 088 US5 (FR-040 to FR-045): the GM clears the roll feed for everyone.
 *
 * At the first session a GM pressed something expecting it to clear the
 * feed, and one player's feed kept the rolls. So the proof is every screen:
 * a GM and two players roll (one for the GM's eyes, one GM only); the GM
 * clears; every feed empties without a reload, the one that could not hear
 * the clear included, once it is back; and neither a reload nor a reconnect
 * brings a cleared roll back. A player is offered no **Clear rolls**, and
 * the server refuses one from them.
 */

const CONFIRMATION =
  "Clear every roll from the feed for everyone? The rolls are kept in the world's record.";

/** Record the page's `/api/ws` sockets, so they can be cut. */
async function recordSockets(page: Page): Promise<void> {
  await page.addInitScript(() => {
    const Native = window.WebSocket;
    const sockets: WebSocket[] = [];
    (window as unknown as { __e2eSockets: WebSocket[] }).__e2eSockets = sockets;
    class RecordingWebSocket extends Native {
      constructor(url: string | URL, protocols?: string | string[]) {
        super(url, protocols);
        if (String(url).includes("/api/ws")) sockets.push(this);
      }
    }
    window.WebSocket = RecordingWebSocket as unknown as typeof WebSocket;
  });
}

/**
 * Cut the page off from the server's events and keep it off, as
 * `world-event-catchup.spec.ts` does; answers a function that lets it back.
 */
async function cutOff(page: Page): Promise<() => Promise<void>> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Network.enable");
  await cdp.send("Network.setBlockedURLs", { urls: ["*/api/ws*"] });
  const open = await page.evaluate(() => {
    const sockets = (window as unknown as { __e2eSockets: WebSocket[] })
      .__e2eSockets;
    const live = sockets.filter((s) => s.readyState === WebSocket.OPEN);
    // 4499: graphql-ws retries it, as a dropped connection is retried.
    for (const socket of live) socket.close(4499, "e2e sever");
    return live.length;
  });
  expect(open, "the page should have a live /api/ws socket").toBeGreaterThan(0);
  await expect(
    page.getByTestId("live-sync-reconnecting-indicator"),
  ).toBeVisible({ timeout: 20_000 });
  return async () => {
    await cdp.send("Network.setBlockedURLs", { urls: [] });
    await expect(
      page.getByTestId("live-sync-reconnecting-indicator"),
    ).toBeHidden({ timeout: 60_000 });
  };
}

const entries = (page: Page) => page.getByTestId("roll-entry");

test.describe("Clearing the rolls", () => {
  test("the GM clears the feed on every screen, and nothing brings a cleared roll back", async ({
    page,
    browser,
  }) => {
    test.setTimeout(360_000);
    const table = await seatRollTable(page, browser);
    const { gm, worldId } = table;
    const [a, b] = table.players;

    try {
      // Player B is the one who will miss the clear: its sockets recorded
      // from a fresh load, so they can be cut.
      await recordSockets(b);
      await b.reload();
      await waitForPlayView(b);
      await openChat(b);
      await b.waitForTimeout(6_000);

      await rollFromRoller(a, "1d20");
      await rollFromRoller(b, "1d8", "GM_EYES");
      await rollFromRoller(gm, "1d6", "GM_ONLY");
      // The GM sees all three; each player their own and the other's (the
      // GM's eyes roll masked to A), never the GM only one.
      await expect(entries(gm)).toHaveCount(3, { timeout: 15_000 });
      await expect(entries(a)).toHaveCount(2, { timeout: 15_000 });
      await expect(entries(b)).toHaveCount(2, { timeout: 15_000 });

      // --- A player is offered no clear, and is refused one -------------
      for (const player of [a, b]) {
        await expect(player.getByTestId("chat-clear-rolls")).toHaveCount(0);
      }
      const refused = await graphql<{ errors?: { message: string }[] }>(
        a,
        `
          mutation ($worldId: UUID!) {
            clearWorldRolls(worldId: $worldId) {
              clearedAt
            }
          }
        `,
        { worldId },
      );
      expect(refused.errors?.map((e) => e.message)).toEqual([
        "Only the GM can clear the rolls.",
      ]);
      await expect(entries(gm)).toHaveCount(3);

      // --- B drops off; the GM clears ------------------------------------
      const letBackIn = await cutOff(b);
      const playedByB = (await dicePlayed(b)).length;

      let asked = "";
      gm.once("dialog", (dialog) => {
        asked = dialog.message();
        void dialog.accept();
      });
      await gm.getByTestId("chat-clear-rolls").click();
      await expect.poll(() => asked).toBe(CONFIRMATION);

      // Live, with no reload, on the screens that could hear it.
      await expect(entries(gm)).toHaveCount(0, { timeout: 15_000 });
      await expect(entries(a)).toHaveCount(0, { timeout: 15_000 });

      // B was not listening; its catch-up brings the clear, not the rolls.
      await letBackIn();
      await expect(entries(b)).toHaveCount(0, { timeout: 30_000 });
      await b.waitForTimeout(5_000);
      expect(
        (await dicePlayed(b)).length,
        "no cleared roll is thrown again",
      ).toBe(playedByB);

      // A reload reads the server's feed: nothing cleared comes back.
      for (const viewer of [gm, a, b]) {
        await viewer.reload();
        await waitForPlayView(viewer);
        await openChat(viewer);
      }
      await gm.waitForTimeout(5_000);
      for (const viewer of [gm, a, b]) {
        await expect(entries(viewer)).toHaveCount(0);
      }

      // --- A roll after the clear is the only one -------------------------
      const total = await rollFromRoller(a, "1d20");
      for (const viewer of [gm, a, b]) {
        await expect(entries(viewer)).toHaveCount(1, { timeout: 15_000 });
        await expect(entries(viewer).getByTestId("roll-total")).toHaveText(
          String(total),
        );
      }
    } finally {
      await table.close();
    }
  });
});
