import { expect, test } from "./fixtures/test";
import {
  registerAndCreateWorld,
  setWorldSystem,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  claimNamedCharacter,
  importsOf,
  openImport,
  review,
  withSheetImportOn,
} from "./fixtures/sheetImport";

/**
 * Spec 048 User Story 2: no value lands that the person was not shown
 * (SC-003). The sheet is `review-checks.pdf`, generated from an invented
 * fighter: its Strength is printed "l6", which is not a number, and its
 * Perception is printed +6 where Wisdom and the bonus give +4.
 */

test.describe.configure({ mode: "serial" });
withSheetImportOn();

test.describe("Spec 048: the review before a sheet is brought in", () => {
  test("the player filters to what needs checking, sees the cross-check, corrects the misprinted score, and the GM sees which field was corrected", async ({
    page: gm,
    browser,
  }) => {
    test.setTimeout(300_000);
    const worldId = await registerAndCreateWorld(
      gm,
      `E2E Sheet Review ${uniqueSuffix()}`,
    );
    await setWorldSystem(gm, worldId, "dnd5e");
    const { player, actorId } = await claimNamedCharacter(browser, gm, worldId);
    try {
      await openImport(player, worldId, actorId);
      await review(player, "review-checks.pdf");

      // The misprint is shown for checking, with what was printed.
      const strength = player.getByTestId("sheet-import-field-abilities.str");
      await expect(strength).toHaveAttribute("data-certainty", "uncertain");
      await expect(strength).toContainText("l6");

      // Every changed row says whether it is new or replaces something.
      const name = player.getByTestId("sheet-import-field-identity.name");
      await expect(name).toHaveAttribute("data-change", /^(new|overwrites)$/);
      await expect(name.getByTestId("sheet-import-field-change")).toHaveText(
        /^(new|will overwrite)$/,
      );

      // The filters: what needs checking, then everything again.
      await player.getByTestId("sheet-import-filter-uncertain").click();
      await expect(strength).toBeVisible();
      await expect(name).toHaveCount(0);
      const rows = player.getByTestId("sheet-import-fields").locator("li");
      await expect(rows).not.toHaveCount(0);
      for (const certainty of await rows.evaluateAll((items) =>
        items.map((item) => item.getAttribute("data-certainty")),
      )) {
        expect(certainty).toBe("uncertain");
      }
      await player.getByTestId("sheet-import-filter-all").click();
      await expect(name).toBeVisible();

      // The cross-check shows the sheet's number and the system's.
      const perception = player.getByTestId(
        "sheet-import-crosscheck-derived.skill.perception",
      );
      await expect(
        perception.getByTestId("sheet-import-crosscheck-sheet"),
      ).toHaveText("6");
      await expect(
        perception.getByTestId("sheet-import-crosscheck-derived"),
      ).toHaveText("4");

      // The correction: the plan comes back with the row corrected.
      await player.getByTestId("sheet-import-correct-abilities.str").fill("16");
      await player
        .getByTestId("sheet-import-correct-abilities.str-use")
        .click();
      await expect(strength).toHaveAttribute("data-certainty", "corrected", {
        timeout: 15_000,
      });
      await expect(strength.getByTestId("sheet-import-field-new")).toHaveText(
        "16",
      );
      expect(await importsOf(player, actorId)).toHaveLength(0);

      await player.getByTestId("sheet-import-accept").click();
      await expect(player).toHaveURL(
        new RegExp(`/world/${worldId}/actor/${actorId}/view$`),
        { timeout: 30_000 },
      );
      await expect(player.getByTestId("dnd5e-score-strength")).toContainText(
        "16",
        { timeout: 15_000 },
      );

      // The GM sees what the player changed.
      const records = await importsOf(gm, actorId);
      expect(records).toHaveLength(1);
      expect(records[0].correctedFields).toEqual(["abilities.str"]);
    } finally {
      await player.context().close();
    }
  });
});
