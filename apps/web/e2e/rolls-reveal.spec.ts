import type { Page } from "@playwright/test";

import { graphql } from "./fixtures/helpers";
import { expect, test } from "./fixtures/test";
import {
  captureTraffic,
  dicePlayed,
  rollFromRoller,
  seatRollTable,
} from "./fixtures/rolls";

/**
 * Spec 081 US5: the GM reveals.
 *
 * A player's GM's eyes roll and the GM's own GM only roll are each revealed
 * by the GM. Every member's chat then shows the whole roll, at the time it
 * was made, marked "revealed by" the GM, and every board animates it
 * (SC-004). A reveal is said once: revealing again sends the other members
 * nothing.
 */
async function entryFor(page: Page, rollId: string) {
  const entry = page.locator(
    `[data-testid="roll-entry"][data-roll-id="${rollId}"]`,
  );
  await expect(entry).toHaveCount(1, { timeout: 15_000 });
  return entry;
}

test("a revealed roll is whole for everyone and animates on every board", async ({
  page,
  browser,
}) => {
  test.setTimeout(300_000);
  const table = await seatRollTable(page, browser);
  const { gm } = table;
  const [a, b] = table.players;
  const everyone = [gm, a, b];

  try {
    const eyes = await rollFromRoller(a, "1d20+31", "GM_EYES");
    const hidden = await rollFromRoller(gm, "1d20+41", "GM_ONLY");
    const gmEntries = gm.getByTestId("roll-entry");
    await expect(gmEntries).toHaveCount(2, { timeout: 15_000 });
    const [eyesId, hiddenId] = await gmEntries.evaluateAll((nodes) =>
      nodes.map((node) => node.getAttribute("data-roll-id") ?? ""),
    );
    await expect(b.getByTestId("roll-entry")).toHaveCount(1, {
      timeout: 15_000,
    });
    // Past the window the rolls themselves had to animate in.
    await b.waitForTimeout(5_000);
    const before = await Promise.all(everyone.map(dicePlayed));

    for (const [rollId, total] of [
      [eyesId, eyes],
      [hiddenId, hidden],
    ] as const) {
      const revealedAt = Date.now();
      await (await entryFor(gm, rollId))
        .getByTestId("roll-reveal-button")
        .click();
      for (const board of everyone) {
        const entry = await entryFor(board, rollId);
        await expect(entry.getByTestId("roll-total")).toHaveText(
          String(total),
          { timeout: 15_000 },
        );
        await expect(entry.getByTestId("roll-revealed")).toContainText(
          "revealed by",
        );
        await expect(entry.getByTestId("roll-visibility")).toHaveCount(0);
        await expect(entry.getByTestId("roll-reveal-button")).toHaveCount(0);
      }
      test.info().annotations.push({
        type: "SC-004",
        description: `${rollId} whole for everyone ${Date.now() - revealedAt} ms after the reveal`,
      });
    }

    // Each board played each reveal once, after what it had before.
    const dieOf = (total: number, plus: number) => [total - plus];
    for (const [at, board] of everyone.entries()) {
      await expect
        .poll(() => dicePlayed(board), { timeout: 15_000 })
        .toEqual([...before[at], dieOf(eyes, 31), dieOf(hidden, 41)]);
    }
    // Both rolls keep their places, oldest first.
    for (const board of everyone) {
      await expect(board.getByTestId("roll-entry")).toHaveCount(2);
      const ids = await board
        .getByTestId("roll-entry")
        .evaluateAll((nodes) =>
          nodes.map((node) => node.getAttribute("data-roll-id")),
        );
      expect(ids).toEqual([eyesId, hiddenId]);
    }

    // A second reveal is the same answer and tells nobody anything.
    const heardByB = captureTraffic(b);
    const again = await graphql<{ errors?: unknown[] }>(
      gm,
      `
        mutation ($worldId: UUID!, $rollId: UUID!) {
          revealRoll(worldId: $worldId, rollId: $rollId) {
            id
          }
        }
      `,
      { worldId: table.worldId, rollId: eyesId },
    );
    expect(again.errors).toBeUndefined();
    await b.waitForTimeout(3_000);
    for (const text of heardByB()) {
      expect(text).not.toContain(eyesId);
    }
  } finally {
    await table.close();
  }
});
