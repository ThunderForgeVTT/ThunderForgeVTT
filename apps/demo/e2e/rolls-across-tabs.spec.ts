import { expect, test, type Page } from "@playwright/test";
import { ask, enterPlay, openDemo, WORLD_ID } from "./support";

/**
 * Spec 081 US6: the demo's tabs share one world. Two tabs of one browser,
 * one switched to the player: a roll in either animates on both boards, a
 * GM only roll stays on the GM's, the second tab keeps working when the
 * first closes, and a reload of both reads one world. Spec 088 US5: a GM
 * who clears the rolls in one tab empties the other tab's feed too.
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

interface Landed {
  rollId: string;
  skipped: boolean;
  dice: { face: number; restingPlace: [number, number] }[];
}

/** Spec 083: every throw this board's engine landed, oldest first. */
function diceLanded(page: Page): Promise<Landed[]> {
  return page.evaluate(() =>
    (
      window as unknown as { __engineProbe: { diceLanded: () => Landed[] } }
    ).__engineProbe.diceLanded(),
  );
}

test("two tabs throw the same dice: the same faces in the same places", async ({
  browser,
  baseURL,
}) => {
  test.setTimeout(240_000);
  const { page: first, outside } = await openDemo(browser, baseURL);
  const context = first.context();
  try {
    await enterPlay(first);
    await boardReady(first);
    const second = await context.newPage();
    await second.goto(`/demo/world/${WORLD_ID}/play`);
    await boardReady(second);

    const open = await roll(second, "1d20", "EVERYONE");
    for (const board of [first, second]) {
      await expect
        .poll(() => dicePlayed(board), { timeout: 15_000 })
        .toEqual([[open]]);
    }
    const [rollId] = await rollIds(second);
    const throws: Landed[] = [];
    for (const board of [first, second]) {
      await expect
        .poll(async () => (await diceLanded(board)).length, {
          timeout: 15_000,
        })
        .toBe(1);
      const [landed] = await diceLanded(board);
      expect(landed.rollId).toBe(rollId);
      expect(landed.skipped).toBe(false);
      expect(landed.dice.map((d) => d.face)).toEqual([open]);
      throws.push(landed);
    }
    expect(throws[1].dice).toEqual(throws[0].dice);

    expect(outside, "nothing leaves the demo's own static files").toEqual([]);
  } finally {
    await context.close();
  }
});

const entries = (page: Page) => page.getByTestId("roll-entry");

async function openChat(page: Page): Promise<void> {
  const trigger = page.getByTestId("world-dock-tab-chat");
  await expect(trigger).toBeVisible({ timeout: 15_000 });
  if ((await trigger.getAttribute("aria-expanded")) !== "true") {
    await trigger.click();
  }
  await expect(page.getByTestId("chat-panel")).toBeVisible({ timeout: 15_000 });
}

test("the GM clears the rolls in one tab: the other tab's feed empties, and a reload keeps it empty", async ({
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
    await gm.reload();
    await boardReady(gm);

    await roll(player, "1d20", "EVERYONE");
    await roll(gm, "1d6", "GM_ONLY");
    await openChat(gm);
    await openChat(player);
    await expect(entries(gm)).toHaveCount(2, { timeout: 15_000 });
    await expect(entries(player)).toHaveCount(1, { timeout: 15_000 });

    // The player has no button, and the backend refuses the player too.
    await expect(player.getByTestId("chat-clear-rolls")).toHaveCount(0);
    const refused = await ask(
      player,
      `mutation ($worldId: UUID!) { clearWorldRolls(worldId: $worldId) { clearedAt } }`,
      { worldId: WORLD_ID },
    );
    expect(JSON.stringify(refused.body.errors)).toContain(
      "Only the GM can clear the rolls.",
    );

    const playedBefore = (await dicePlayed(player)).length;
    gm.once("dialog", (dialog) => void dialog.accept());
    await gm.getByTestId("chat-clear-rolls").click();
    await expect(entries(gm)).toHaveCount(0, { timeout: 15_000 });
    await expect(entries(player)).toHaveCount(0, { timeout: 15_000 });
    // Nothing is thrown again on the other tab's board.
    await player.waitForTimeout(3_000);
    expect(await dicePlayed(player)).toHaveLength(playedBefore);

    // A reload of both reads the cleared world.
    await gm.reload();
    await player.reload();
    await Promise.all([boardReady(gm), boardReady(player)]);
    expect(await rollIds(gm)).toEqual([]);
    expect(await rollIds(player)).toEqual([]);
    await openChat(gm);
    await openChat(player);
    await expect(entries(gm)).toHaveCount(0);
    await expect(entries(player)).toHaveCount(0);

    // A roll after the clear is listed on both tabs.
    await roll(player, "1d8", "EVERYONE");
    await expect(entries(gm)).toHaveCount(1, { timeout: 15_000 });
    await expect(entries(player)).toHaveCount(1, { timeout: 15_000 });

    expect(outside, "nothing leaves the demo's own static files").toEqual([]);
  } finally {
    await context.close();
  }
});
