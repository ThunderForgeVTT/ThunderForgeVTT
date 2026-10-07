import { expect, test, type Page } from "@playwright/test";
import { ask, enterPlay, openDemo, WORLD_ID } from "./support";

/**
 * Spec 081 US6: the demo's tabs share one world. Two tabs of one browser,
 * one switched to the player: a roll in either animates on both boards, a
 * GM only roll stays on the GM's, the second tab keeps working when the
 * first closes, and a reload of both reads one world.
 */

type Visibility = "EVERYONE" | "GM_ONLY";

function dicePlayed(page: Page): Promise<number[][]> {
  return page.evaluate(() =>
    (
      window as unknown as { __engineProbe: { dicePlayed: () => number[][] } }
    ).__engineProbe.dicePlayed(),
  );
}

async function boardReady(page: Page): Promise<void> {
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
  await expect
    .poll(
      () =>
        page.evaluate(
          () =>
            typeof (
              window as unknown as {
                __engineProbe?: { dicePlayed?: unknown };
              }
            ).__engineProbe?.dicePlayed === "function",
        ),
      { timeout: 60_000 },
    )
    .toBe(true);
}

/** Rolls from the dice roller, and answers the die the panel was shown. */
async function roll(
  page: Page,
  formula: string,
  visibility: Visibility,
): Promise<number> {
  const panel = page.getByTestId("dice-roller-panel");
  await panel.getByTestId("roll-visibility-picker").selectOption(visibility);
  await panel.getByTestId("dice-formula-input").fill(formula);
  await panel.getByTestId("dice-roll-button").click();
  const result = panel.getByTestId("dice-roll-result");
  await expect(result).toContainText(formula, { timeout: 15_000 });
  const records = await ask<{
    worldRolls: { resolution: { dice: { finalValue: number }[] } }[];
  }>(
    page,
    `query ($worldId: UUID!) {
      worldRolls(worldId: $worldId, limit: 1) {
        ... on WorldRoll { resolution { dice { finalValue } } }
      }
    }`,
    { worldId: WORLD_ID },
  );
  return records.body.data!.worldRolls[0].resolution.dice[0].finalValue;
}

async function rollIds(page: Page): Promise<string[]> {
  const answer = await ask<{ worldRolls: { id: string }[] }>(
    page,
    `query ($worldId: UUID!) {
      worldRolls(worldId: $worldId, limit: 100) {
        ... on WorldRoll { id }
        ... on MaskedRoll { id }
      }
    }`,
    { worldId: WORLD_ID },
  );
  return answer.body.data!.worldRolls.map((entry) => entry.id);
}

/** The world as this browser saved it: every tab reads the same store. */
function savedRollIds(page: Page): Promise<string[]> {
  return page.evaluate(() => {
    const raw = window.localStorage.getItem("thunderforge-demo:v4");
    const saved = raw ? (JSON.parse(raw) as { rolls?: { id: string }[] }) : {};
    return (saved.rolls ?? []).map((r) => r.id).reverse();
  });
}

test("two tabs share one world: rolls reach both boards, a GM only roll does not reach the player's", async ({
  browser,
  baseURL,
}) => {
  test.setTimeout(240_000);
  const { page: gm, outside } = await openDemo(browser, baseURL);
  const context = gm.context();
  try {
    await enterPlay(gm);
    await boardReady(gm);

    const player = await context.newPage();
    await player.goto(`/demo/world/${WORLD_ID}/play`);
    await expect(player.getByTestId("demo-viewer")).toBeVisible({
      timeout: 60_000,
    });
    await player.getByRole("button", { name: "View as player" }).click();
    await expect(player.getByTestId("demo-viewer")).toContainText(
      "Viewing as a player",
    );
    await boardReady(player);
    // The switch is the tab's own: the first tab still renders for the GM.
    await gm.reload();
    await boardReady(gm);
    await expect(gm.getByTestId("demo-viewer")).toContainText(
      "Viewing as the Game Master",
    );

    // A player's roll in the open, from the guest or the holder alike.
    const open = await roll(player, "1d20", "EVERYONE");
    for (const board of [gm, player]) {
      await expect
        .poll(() => dicePlayed(board), { timeout: 15_000 })
        .toEqual([[open]]);
    }

    // The GM's roll behind the screen reaches the GM's board only.
    const hidden = await roll(gm, "1d20 + 5", "GM_ONLY");
    await expect
      .poll(() => dicePlayed(gm), { timeout: 15_000 })
      .toEqual([[open], [hidden]]);
    await player.waitForTimeout(5_000);
    expect(await dicePlayed(player)).toEqual([[open]]);
    expect(await rollIds(player)).toHaveLength(1);

    // A reload of both reads one world, whichever tab holds it after.
    await gm.reload();
    await player.reload();
    await Promise.all([boardReady(gm), boardReady(player)]);
    const all = await rollIds(gm);
    expect(all).toHaveLength(2);
    expect(await savedRollIds(gm)).toEqual(all);
    expect(await savedRollIds(player)).toEqual(all);
    expect(await rollIds(player)).toEqual([all[1]]);

    // The first tab closes; the second keeps rolling onto its board.
    await gm.close();
    const after = await roll(player, "1d6", "EVERYONE");
    await expect
      .poll(() => dicePlayed(player), { timeout: 15_000 })
      .toEqual([[after]]);
    expect(await rollIds(player)).toHaveLength(2);

    expect(outside, "nothing leaves the demo's own static files").toEqual([]);
  } finally {
    await context.close();
  }
});
