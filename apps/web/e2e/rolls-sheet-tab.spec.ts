import { expect, test } from "./fixtures/test";
import { dicePlayed, openChat, waitForEngineProbe } from "./fixtures/rolls";
import { claimFor, grantAbility } from "../playtest/combat";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";

/**
 * Spec 081 US2: a player's sheet in another tab rolls onto the play view.
 *
 * One player, one browser, two tabs: the play view and their character's
 * sheet page. A roll on the sheet page reaches the play tab the way any roll
 * reaches any board — as a world event from the server — so the play tab
 * animates it and its chat lists it by name (SC-002). Nothing passes between
 * the tabs. An attack needs a target picked on the board, so the sheet page
 * says so instead of rolling it (FR-014).
 */
test("a roll from the sheet page animates on the play view in another tab", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(5 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Wren"],
  });
  const [wren] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Wren",
      at: { x: 0, y: 0 },
      seat: wren,
    });
    await claimFor(table, wren, hero.actorId);
    await grantAbility(table, hero.actorId, {
      name: "Stealth",
      classification: "feat",
      effects: [{ effectType: "MODIFIER", formula: "1d20+3" }],
    });
    await grantAbility(table, hero.actorId, {
      name: "Shortsword",
      classification: "feat",
      effects: [{ effectType: "ATTACK_ROLL", formula: "1d20+5" }],
    });

    const play = wren.page;
    await sitDown(table, play);
    await waitForEngineProbe(play);
    await openChat(play);
    expect(await dicePlayed(play)).toEqual([]);

    const sheetTab = await play.context().newPage();
    await sheetTab.goto(`/world/${table.worldId}/actor/${hero.actorId}/view`);
    const rolls = sheetTab.getByTestId("sheet-rolls");
    await expect(rolls).toBeVisible({ timeout: 30_000 });

    // FR-014: the attack is there, and says where it is made.
    const attack = rolls.locator('[data-attack="true"]');
    await expect(attack).toHaveCount(1);
    await expect(attack).toBeDisabled();
    await expect(attack).toContainText("Pick the target on the board");

    await rolls.getByRole("button", { name: /Stealth/ }).click();
    const result = sheetTab.getByTestId("sheet-roll-result");
    await expect(result).toBeVisible({ timeout: 15_000 });
    const rolledAt = Date.now();

    // The play tab's board played exactly one d20, the one just rolled.
    await expect
      .poll(() => dicePlayed(play), { timeout: 15_000 })
      .toHaveLength(1);
    const [[die]] = await dicePlayed(play);
    expect(die).toBeGreaterThanOrEqual(1);
    expect(die).toBeLessThanOrEqual(20);
    test.info().annotations.push({
      type: "SC-002",
      description: `play tab animated ${Date.now() - rolledAt} ms after the sheet's result`,
    });

    const entry = play.getByTestId("roll-entry");
    await expect(entry).toHaveCount(1, { timeout: 15_000 });
    await expect(entry.getByTestId("roll-label")).toContainText("Stealth");
    await expect(entry.getByTestId("roll-total")).toHaveText(String(die + 3));
    await sheetTab.close();
  } finally {
    await closeTable(table);
  }
});
