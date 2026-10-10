import { expect, test } from "./fixtures/test";
import { openDockTab } from "./fixtures/helpers";
import { claimFor, setTraits } from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";

/**
 * A 5e character's stat rolls are 5e's, in the dock and on the sheet page.
 *
 * 5e keeps ability *scores* in `ability_data`, and Armor Class beside them.
 * The host's generic reading used to offer every number there as a d20
 * modifier: "Wisdom 1d20+18" and "Armor class 1d20+18". The 5e pack now
 * contributes its own stat rolls (`packs/systems/dnd5e/web/src/statRolls.ts`):
 * a check per score at its modifier, a save per proficient ability, and
 * nothing for Armor Class.
 *
 * The saves are written the way a sheet import writes them, as an object of
 * flags, so the dock is proved against what an imported character carries.
 */
test("a 5e character's dock and sheet offer checks from scores, and no Armor Class roll", async ({
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
      sheet: {
        scores: {
          strength: 8,
          dexterity: 14,
          constitution: 12,
          intelligence: 10,
          wisdom: 18,
          charisma: 10,
          armor_class: 18,
        },
        hitPoints: { current: 30, max: 30 },
      },
    });
    await claimFor(table, wren, hero.actorId);
    await setTraits(table, hero.actorId, { level: 5 });
    await must(
      table.gm,
      `mutation ($input: GraphQLUpdateActorSystemDataInput!) {
        updateActorSystemData(input: $input) { id }
      }`,
      {
        input: {
          actorId: hero.actorId,
          gameSystemId: table.system,
          dataType: "proficiency_data",
          data: {
            saving_throw_proficiencies: { wisdom: true, charisma: false },
          },
        },
      },
    );

    // The dock's sheet.
    const play = wren.page;
    await sitDown(table, play);
    await openDockTab(play, "actors");
    if (!(await play.getByTestId("in-pane-character-sheet").isVisible())) {
      await play.getByTestId(`actor-view-${hero.actorId}`).click();
    }
    const sheet = play.getByTestId("in-pane-character-sheet");
    await expect(sheet).toBeVisible({ timeout: 10_000 });

    const wisdom = sheet.getByTestId("in-pane-roll-stat-wisdom");
    await expect(wisdom).toBeVisible({ timeout: 15_000 });
    await expect(wisdom).toContainText("Wisdom");
    await expect(wisdom).toContainText("1d20+4");
    await expect(sheet.getByTestId("in-pane-roll-stat-strength")).toContainText(
      "1d20-1",
    );
    await expect(
      sheet.getByTestId("in-pane-roll-stat-intelligence"),
    ).toContainText(/1d20$/);
    // Level 5 is +3, so a proficient Wisdom save is +7; Charisma is not
    // proficient and offers no save of its own.
    await expect(sheet.getByTestId("in-pane-roll-save-wisdom")).toContainText(
      "1d20+7",
    );
    await expect(sheet.getByTestId("in-pane-roll-save-charisma")).toHaveCount(
      0,
    );
    await expect(
      sheet.getByTestId("in-pane-roll-stat-armor_class"),
    ).toHaveCount(0);
    await expect(
      sheet.getByRole("button", { name: /Armor class/i }),
    ).toHaveCount(0);
    await expect(sheet).not.toContainText("1d20+18");

    // The sheet page reads the same rolls through the same hook (spec 081).
    const sheetTab = await play.context().newPage();
    await sheetTab.goto(`/world/${table.worldId}/actor/${hero.actorId}/view`);
    const rolls = sheetTab.getByTestId("sheet-rolls");
    await expect(rolls).toBeVisible({ timeout: 30_000 });
    await expect(rolls.getByTestId("sheet-roll-stat-wisdom")).toContainText(
      "1d20+4",
    );
    await expect(rolls.getByTestId("sheet-roll-stat-armor_class")).toHaveCount(
      0,
    );
    await expect(rolls).not.toContainText("1d20+18");
    await sheetTab.close();
  } finally {
    await closeTable(table);
  }
});
