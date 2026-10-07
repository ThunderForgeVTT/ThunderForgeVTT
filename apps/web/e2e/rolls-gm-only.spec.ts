import { expect, test } from "./fixtures/test";
import {
  captureTraffic,
  dicePlayed,
  openChat,
  rollFromRoller,
  seatRollTable,
  waitForPlayView,
} from "./fixtures/rolls";

/**
 * Spec 081 US4: the GM rolls behind the screen.
 *
 * A "GM only" roll animates on the GM's board and is listed, marked, in the
 * GM's chat. The players get nothing at all — no entry, no `****`, no dice —
 * and their browsers never receive the roll's event (FR-005a): no code-36
 * frame, no `MaskedRoll`, live or in a reload's catch-up (SC-003).
 */
const FORMULA = "1d20+23";

test("a GM only roll reaches the GM and nobody else, live or after a reload", async ({
  page,
  browser,
}) => {
  test.setTimeout(300_000);
  const table = await seatRollTable(page, browser);
  const [a, b] = table.players;
  const heardByA = captureTraffic(a);

  try {
    const total = await rollFromRoller(table.gm, FORMULA, "GM_ONLY");

    await expect
      .poll(() => dicePlayed(table.gm), { timeout: 15_000 })
      .toEqual([[total - 23]]);
    const entry = table.gm.getByTestId("roll-entry");
    await expect(entry).toHaveCount(1, { timeout: 15_000 });
    await expect(entry.getByTestId("roll-total")).toHaveText(String(total));
    await expect(entry.getByTestId("roll-visibility")).toHaveText("GM only");

    await a.waitForTimeout(5_000);
    for (const player of [a, b]) {
      await expect(player.getByTestId("roll-entry")).toHaveCount(0);
      expect(await dicePlayed(player)).toEqual([]);
    }

    // A reload's catch-up holds nothing of it either.
    await a.reload();
    await waitForPlayView(a);
    await openChat(a);
    await a.waitForTimeout(5_000);
    await expect(a.getByTestId("roll-entry")).toHaveCount(0);
    expect(await dicePlayed(a)).toEqual([]);

    const heard = heardByA();
    expect(heard.length).toBeGreaterThan(0);
    for (const text of heard) {
      expect(text).not.toContain(FORMULA);
      expect(text).not.toContain("MaskedRoll");
      expect(text).not.toMatch(/"eventCode"\s*:\s*36\b/);
      expect(text).not.toMatch(/"event_code"\s*:\s*36\b/);
    }
  } finally {
    await table.close();
  }
});
