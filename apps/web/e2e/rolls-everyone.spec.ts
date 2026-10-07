import { expect, test, type Page } from "./fixtures/test";
import {
  clickPlay,
  inviteAndJoinAsPlayer,
  openDockTab,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Spec 081 US1: a roll in the open reaches every board at the table.
 *
 * A GM and two players sit at one world's play view, each in their own
 * browser context. One player rolls `1d20` from the dice roller. Every board
 * animates it — the roller's own included, since a board animates a roll
 * from its world event and never from the panel that rolled it (FR-013) —
 * and every chat panel lists it with the same total. A reload lists it once
 * and does not roll the dice again.
 *
 * "Animates" is read from `__engineProbe.dicePlayed`, the dice each board
 * handed to the engine. It is recorded only after the engine took the
 * command, so it is the board's own account, not the panel's.
 */

async function waitForPlayView(page: Page): Promise<void> {
  await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });
  await expect(page.getByTestId("live-sync-reconnecting-indicator")).toBeHidden(
    { timeout: 30_000 },
  );
  await expect
    .poll(
      () =>
        page.evaluate(
          () =>
            typeof (window as unknown as { __engineProbe?: unknown })
              .__engineProbe === "object",
        ),
      { timeout: 60_000 },
    )
    .toBe(true);
}

async function openChat(page: Page): Promise<void> {
  await openDockTab(page, "chat");
  await expect(page.getByTestId("chat-panel")).toBeVisible({ timeout: 15_000 });
}

function dicePlayed(page: Page): Promise<number[][]> {
  return page.evaluate(() =>
    (
      window as unknown as { __engineProbe: { dicePlayed: () => number[][] } }
    ).__engineProbe.dicePlayed(),
  );
}

test.describe("Rolls at the table", () => {
  test("a player's open roll animates on every board and lists once in every chat", async ({
    page: gm,
    browser,
  }) => {
    test.setTimeout(300_000);
    const suffix = uniqueSuffix();
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Rolls ${suffix}`,
      "e2erollgm",
    );
    const roller = await inviteAndJoinAsPlayer(
      browser,
      gm,
      worldId,
      "e2erollera",
    );
    const watcher = await inviteAndJoinAsPlayer(
      browser,
      gm,
      worldId,
      "e2erollerb",
    );
    const table = [gm, roller, watcher];

    try {
      await clickPlay(gm);
      for (const page of [roller, watcher]) {
        await page.goto(`/world/${worldId}/play`);
      }
      for (const page of table) {
        await waitForPlayView(page);
        await openChat(page);
      }
      for (const page of table) {
        expect(await dicePlayed(page)).toEqual([]);
      }
      // The socket being live is not every subscription being open; the
      // play view opens several as it renders. Same measured settle as
      // chat-panel.spec.ts.
      await watcher.waitForTimeout(6_000);

      await roller.getByTestId("dice-formula-input").fill("1d20");
      await roller.getByTestId("dice-roll-button").click();
      await expect(roller.getByTestId("dice-roll-result")).toBeVisible({
        timeout: 15_000,
      });
      const rolledAt = Date.now();
      const shown = await roller.getByTestId("dice-roll-result").innerText();
      const total = Number(/1d20:\s*(-?\d+)/.exec(shown)?.[1]);
      expect(Number.isFinite(total)).toBe(true);

      // Every board, the roller's own included, played exactly this die.
      const arrivals: number[] = [];
      for (const page of table) {
        await expect
          .poll(() => dicePlayed(page), { timeout: 15_000 })
          .toEqual([[total]]);
        arrivals.push(Date.now() - rolledAt);
      }
      test.info().annotations.push({
        type: "SC-001",
        description: `boards animated by ${Math.max(...arrivals)} ms after the roller's result`,
      });

      for (const page of table) {
        const entry = page.getByTestId("roll-entry");
        await expect(entry).toHaveCount(1, { timeout: 15_000 });
        await expect(entry.getByTestId("roll-total")).toHaveText(
          String(total),
        );
        await expect(entry).toContainText("1d20");
        await expect(entry).not.toHaveAttribute("data-masked", "true");
      }

      // A reload is a catch-up, not a new roll: listed once, never replayed.
      await watcher.reload();
      await waitForPlayView(watcher);
      await openChat(watcher);
      const entry = watcher.getByTestId("roll-entry");
      await expect(entry).toHaveCount(1, { timeout: 15_000 });
      await expect(entry.getByTestId("roll-total")).toHaveText(String(total));
      // Given the window a live roll has to arrive in, and then some.
      await watcher.waitForTimeout(5_000);
      expect(await dicePlayed(watcher)).toEqual([]);
      await expect(entry).toHaveCount(1);
    } finally {
      await roller.context().close();
      await watcher.context().close();
    }
  });
});
