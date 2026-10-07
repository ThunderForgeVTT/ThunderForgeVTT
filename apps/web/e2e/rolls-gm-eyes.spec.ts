import { expect, test } from "./fixtures/test";
import {
  captureTraffic,
  dicePlayed,
  rollFromRoller,
  seatRollTable,
} from "./fixtures/rolls";

/**
 * Spec 081 US3: a player rolls for the GM's eyes.
 *
 * Player A rolls "GM's eyes". A and the GM see the whole roll and their
 * boards animate it. Player B is told only that A rolled for the GM — `****`
 * — and B's board plays nothing. What B's browser received over the whole
 * test, every WebSocket frame and every GraphQL response, holds nothing of
 * the roll: no formula, no dice, no total (SC-003). The mask is the
 * server's, not the page's.
 */
const FORMULA = "1d20+17";

test("a roll for the GM's eyes is whole for the roller and the GM, and **** for everyone else", async ({
  page,
  browser,
}) => {
  test.setTimeout(300_000);
  const table = await seatRollTable(page, browser);
  const [a, b] = table.players;
  const heardByB = captureTraffic(b);

  try {
    const total = await rollFromRoller(a, FORMULA, "GM_EYES");
    const die = total - 17;

    for (const board of [table.gm, a]) {
      await expect
        .poll(() => dicePlayed(board), { timeout: 15_000 })
        .toEqual([[die]]);
      const entry = board.getByTestId("roll-entry");
      await expect(entry).toHaveCount(1, { timeout: 15_000 });
      await expect(entry.getByTestId("roll-total")).toHaveText(String(total));
      await expect(entry.getByTestId("roll-visibility")).toHaveText(
        "GM's eyes",
      );
    }
    // Only the GM may reveal it.
    await expect(table.gm.getByTestId("roll-reveal-button")).toBeVisible();
    await expect(a.getByTestId("roll-reveal-button")).toHaveCount(0);

    const masked = b.getByTestId("roll-entry");
    await expect(masked).toHaveCount(1, { timeout: 15_000 });
    await expect(masked).toHaveAttribute("data-masked", "true");
    await expect(masked.getByTestId("roll-total")).toHaveText("****");
    await expect(masked).toContainText("rolled for the GM");

    // Well past the window a live roll has to animate in.
    await b.waitForTimeout(5_000);
    expect(await dicePlayed(b)).toEqual([]);

    const heard = heardByB();
    expect(heard.length).toBeGreaterThan(0);
    for (const text of heard) {
      expect(text).not.toContain(FORMULA);
      expect(text).not.toContain("resultValue");
      expect(text).not.toContain("finalValue");
    }
  } finally {
    await table.close();
  }
});
