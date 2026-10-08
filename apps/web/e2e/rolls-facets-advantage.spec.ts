import { expect, test, type Page } from "./fixtures/test";
import { openDockTab } from "./fixtures/helpers";
import { openChat } from "./fixtures/rolls";
import { claimFor, grantAbility, setAbilityScores } from "../playtest/combat";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";

/**
 * Spec 084 US1 (FR-001, FR-016, FR-018): a check rolled with Advantage and an
 * attack rolled with Disadvantage, from the player's own sheet, as the table
 * sees them.
 *
 * The server shapes the formula, so what is asserted is what it sent back to
 * everyone: two d20s with one struck through, the facet's tag on the roll in
 * both chats, and the picker back on Normal once the roll was sent.
 */

const SCORES = {
  strength: 10,
  dexterity: 16,
  constitution: 12,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};

/** The newest roll in a chat labelled exactly `label`. */
function rollNamed(page: Page, label: string) {
  return page
    .getByTestId("roll-entry")
    .filter({
      has: page.getByTestId("roll-label").getByText(new RegExp(`^${label}:`)),
    })
    .last();
}

/** Two d20s, exactly one struck through, and the facet's tag. */
async function expectShaped(page: Page, label: string, facet: string) {
  const entry = rollNamed(page, label);
  await expect(entry).toBeVisible({ timeout: 15_000 });
  await expect(entry.getByTestId("roll-facet")).toHaveText([facet]);
  await expect(entry.getByTestId("roll-dice").locator("> span")).toHaveCount(2);
  await expect(
    entry.getByTestId("roll-dice").locator(".line-through"),
  ).toHaveCount(1);
  return entry;
}

test("a check with Advantage and an attack with Disadvantage reach every chat, tagged", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(5 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Pip"],
  });
  const [pip] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Pip",
      at: { x: -192, y: 0 },
      seat: pip,
    });
    await placeCast(table, {
      label: "Goblin",
      at: { x: 0, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 13 },
        hitPoints: { current: 7, max: 7 },
      },
    });
    await setAbilityScores(table, hero.actorId, SCORES);
    await claimFor(table, pip, hero.actorId);
    await grantAbility(table, hero.actorId, {
      name: "Dagger",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+3" },
        { effectType: "DAMAGE", formula: "2" },
      ],
    });

    for (const client of [table.gm, pip.page]) {
      await sitDown(table, client);
      await openChat(client);
    }

    // The check, from the sheet page in a second tab.
    const sheetTab = await pip.page.context().newPage();
    await sheetTab.goto(`/world/${table.worldId}/actor/${hero.actorId}/view`);
    const checks = sheetTab.getByTestId("system-checks");
    await expect(checks).toBeVisible({ timeout: 30_000 });
    const picker = checks.getByTestId("roll-advantage-picker");
    await expect(picker).toBeVisible();
    await picker.getByTestId("roll-advantage-ADVANTAGE").click();
    await checks.getByTestId("system-check-dexterity").click();
    await expect(checks.getByTestId("system-check-result")).toBeVisible({
      timeout: 15_000,
    });
    await expect(picker.getByTestId("roll-advantage-NORMAL")).toHaveAttribute(
      "aria-checked",
      "true",
    );
    for (const client of [table.gm, pip.page]) {
      const entry = await expectShaped(client, "Dexterity", "Advantage");
      await expect(entry).toContainText("2d20kh1");
    }
    await sheetTab.close();

    // The attack, from the dock's sheet on the play view.
    const play = pip.page;
    await openDockTab(play, "actors");
    if (!(await play.getByTestId("in-pane-character-sheet").isVisible())) {
      await play.getByTestId(`actor-view-${hero.actorId}`).click();
    }
    const sheet = play.getByTestId("in-pane-character-sheet");
    await expect(sheet).toBeVisible({ timeout: 10_000 });
    await sheet
      .locator('[data-testid^="in-pane-roll-ability-"][data-attack="true"]')
      .filter({ hasText: "Dagger" })
      .first()
      .click();
    const flow = play.getByTestId("attack-flow");
    await expect(flow).toBeVisible();
    await flow
      .getByTestId("attack-flow-target")
      .selectOption({ label: "Goblin" });
    const attackPicker = flow.getByTestId("roll-advantage-picker");
    await expect(attackPicker).toBeVisible({ timeout: 10_000 });
    await attackPicker.getByTestId("roll-advantage-DISADVANTAGE").click();
    await flow.getByTestId("attack-flow-confirm").click();
    await expect(
      flow
        .getByTestId("attack-flow-result")
        .or(flow.getByTestId("attack-flow-error")),
    ).toBeVisible({ timeout: 15_000 });
    await expect(flow.getByTestId("attack-flow-error")).toHaveCount(0);
    await expect(
      attackPicker.getByTestId("roll-advantage-NORMAL"),
    ).toHaveAttribute("aria-checked", "true");
    // The Actors tab took the dock's place; the chat comes back for the check.
    await openChat(play);
    for (const client of [table.gm, play]) {
      const entry = await expectShaped(client, "Dagger", "Disadvantage");
      await expect(entry).toContainText("kl1");
    }
  } finally {
    await closeTable(table);
  }
});
