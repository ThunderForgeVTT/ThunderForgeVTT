import { expect, test, type Page } from "@playwright/test";
import { openDockTab } from "../e2e/fixtures/helpers";
import {
  openBroughtByPlayers,
  review,
  stagedPiece,
} from "../e2e/fixtures/sheetImport";
import { claimFor } from "./combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
  snapshot,
} from "./table";

/**
 * Spec 048 FR-061: a player brings their character in, and plays it.
 *
 * Aubrel's player brings her cleric in from the actor screen, from
 * `cleric-7.pdf` (an invented character the 5e pack's tests generate). Her
 * domain spell Bless is new to the world, so it waits for the Game Master,
 * who adopts it from "Brought by players". Then she casts it at the table,
 * from her sheet in the play dock.
 *
 * It plays the shipped default: `feature.sheet_import` is on (T097), so no
 * administrator signs in to turn it on.
 *
 * A gap goes in as a soft check whose message starts FINDING.
 */

/** The roll buttons Aubrel's sheet offers in the dock, by their text. */
async function rollsOnSheet(page: Page, actorId: string): Promise<string[]> {
  await openDockTab(page, "actors");
  const sheet = page.getByTestId("in-pane-character-sheet");
  if (!(await sheet.isVisible().catch(() => false))) {
    await page.getByTestId(`actor-view-${actorId}`).click();
  }
  await expect(sheet).toBeVisible({ timeout: 20_000 });
  return page.locator('[data-testid^="in-pane-roll-"]').allTextContents();
}

test("a player brings a cleric in from her sheet and casts her domain spell", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(600_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aubrel"],
    sceneName: "The Chapel",
  });
  const [aubrel] = table.players;

  try {
    const { actorId } = await placeCast(table, {
      label: "Sister Aubrel",
      at: { x: 300, y: 300 },
      seat: aubrel,
    });
    await claimFor(table, aubrel, actorId);
    const view = `/world/${table.worldId}/actor/${actorId}/view`;

    await test.step("Aubrel's player brings her sheet in from the actor screen", async () => {
      await aubrel.page.goto(view);
      await aubrel.page.getByTestId("actor-bring-in-sheet").click();
      await expect(aubrel.page).toHaveURL(
        new RegExp(`/world/${table.worldId}/actor/${actorId}/import$`),
        { timeout: 15_000 },
      );
      await review(aubrel.page, "cleric-7.pdf");
      await snapshot(table, "1 · the review");
      await aubrel.page.getByTestId("sheet-import-accept").click();
      await expect(aubrel.page).toHaveURL(new RegExp(`${view}$`), {
        timeout: 30_000,
      });
      await expect(
        aubrel.page.getByText("awaiting the GM").first(),
      ).toBeVisible({ timeout: 15_000 });
      await snapshot(table, "2 · brought in, Bless awaiting the GM");
    });

    const bless = await stagedPiece(table.gm, table.worldId, "spell", "Bless");
    expect(bless.state).toBe("PENDING");

    await test.step("the Game Master adopts Bless", async () => {
      await openBroughtByPlayers(table.gm, table.worldId);
      const row = table.gm
        .getByTestId("staged-row")
        .filter({ hasText: "Bless" });
      await row.getByRole("button", { name: "Adopt Bless" }).click();
      await expect(row).toHaveAttribute("data-state", "ADOPTED", {
        timeout: 15_000,
      });
      await snapshot(table, "3 · Bless adopted");
    });

    const adopted = await stagedPiece(
      table.gm,
      table.worldId,
      "spell",
      "Bless",
    );
    expect(adopted.adoptedAbilityId).toBeTruthy();

    await test.step("Aubrel casts Bless at the table", async () => {
      await sitDown(table, aubrel.page);
      // The sheet's rolls are loaded once its stats are offered. Her mace is
      // not among them: a weapon swings through an attack on a target, not
      // from this list.
      await expect
        .poll(
          async () =>
            (await rollsOnSheet(aubrel.page, actorId)).some((text) =>
              text.includes("Wisdom"),
            ),
          { timeout: 20_000 },
        )
        .toBe(true);
      const before = await rollsOnSheet(aubrel.page, actorId);
      expect
        .soft(
          before.some((text) => text.includes("Bless")),
          "FINDING: an adopted spell the sheet gave no dice to cannot be cast " +
            "from the dock until the Game Master gives it an effect",
        )
        .toBe(true);

      if (!before.some((text) => text.includes("Bless"))) {
        // What a Game Master does at the table: give the spell its die.
        await must(
          table.gm,
          `mutation ($abilityId: UUID!, $effect: AbilityEffectInput!) {
            addAbilityEffect(abilityId: $abilityId, effect: $effect) { id }
          }`,
          {
            abilityId: adopted.adoptedAbilityId,
            effect: {
              effectType: "MODIFIER",
              formula: "1d4",
              target: "up to three creatures",
              triggerKind: "ON_USE",
              sortOrder: 0,
            },
          },
        );
        await aubrel.page.reload();
      }

      await expect
        .poll(
          async () =>
            (await rollsOnSheet(aubrel.page, actorId)).some((text) =>
              text.includes("Bless"),
            ),
          { timeout: 20_000 },
        )
        .toBe(true);
      await aubrel.page
        .locator('[data-testid^="in-pane-roll-ability-"]')
        .filter({ hasText: "Bless" })
        .first()
        .click();
      await expect(aubrel.page.getByTestId("in-pane-roll-result")).toBeVisible({
        timeout: 30_000,
      });
      await snapshot(table, "4 · Bless cast");
    });
  } finally {
    await closeTable(table);
  }
});
