import { expect, test } from "./fixtures/test";
import {
  dicePlayed,
  openChat,
  rollFromRoller,
  seatRollTable,
  waitForPlayView,
} from "./fixtures/rolls";

/**
 * Spec 081 US1: a roll in the open reaches every board at the table.
 *
 * A GM and two players sit at one world's play view, each in their own
 * browser context. One player rolls `1d20` from the dice roller. Every board
 * animates it — the roller's own included, since a board animates a roll
 * from its world event and never from the panel that rolled it (FR-013) —
 * and every chat panel lists it with the same total. A reload lists it once
 * and does not roll the dice again.
 */
test.describe("Rolls at the table", () => {
  test("a player's open roll animates on every board and lists once in every chat", async ({
    page,
    browser,
  }) => {
    test.setTimeout(300_000);
    const table = await seatRollTable(page, browser);
    const [roller, watcher] = table.players;
    const everyone = [table.gm, roller, watcher];

    try {
      const rolledAt = Date.now();
      const total = await rollFromRoller(roller, "1d20");

      // Every board, the roller's own included, played exactly this die.
      const arrivals: number[] = [];
      for (const board of everyone) {
        await expect
          .poll(() => dicePlayed(board), { timeout: 15_000 })
          .toEqual([[total]]);
        arrivals.push(Date.now() - rolledAt);
      }
      test.info().annotations.push({
        type: "SC-001",
        description: `boards animated by ${Math.max(...arrivals)} ms after the roll was asked for`,
      });

      for (const board of everyone) {
        const entry = board.getByTestId("roll-entry");
        await expect(entry).toHaveCount(1, { timeout: 15_000 });
        await expect(entry.getByTestId("roll-total")).toHaveText(String(total));
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
      await table.close();
    }
  });
});
