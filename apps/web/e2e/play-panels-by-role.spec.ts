import { expect, test } from "./fixtures/test";
import {
  clickPlay,
  inviteAndJoinAsPlayer,
  openDockTab,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { waitForPlayView } from "./fixtures/rolls";

/**
 * The play view's panels, as each role sees them (playtest 2026-10-07).
 *
 * The GM chooses the scene and a player follows the one the GM launched, so
 * only the GM has the scenes dropdown. The GM's queue of what players asked
 * for is labelled as such. The dice roller rolls from a red d20 and explains
 * its formulas behind a `?`.
 */
test("the GM alone picks scenes, the requests tab says whose, and the dice roller explains itself", async ({
  page: gm,
  browser,
}) => {
  test.setTimeout(240_000);
  const worldId = await registerAndCreateWorld(
    gm,
    `E2E Panels ${uniqueSuffix()}`,
    "e2epanelgm",
  );
  const player = await inviteAndJoinAsPlayer(
    browser,
    gm,
    worldId,
    "e2epanelpc",
  );
  try {
    await clickPlay(gm);
    await player.goto(`/world/${worldId}/play`);
    await Promise.all([waitForPlayView(gm), waitForPlayView(player)]);

    await openDockTab(gm, "settings");
    await expect(gm.getByTestId("scene-switcher")).toBeVisible();
    await openDockTab(player, "settings");
    await expect(player.getByTestId("settings-panel")).toBeVisible();
    await expect(player.getByTestId("scene-switcher")).toHaveCount(0);

    await expect(gm.getByTestId("world-dock-tab-requests")).toHaveAttribute(
      "aria-label",
      "Player requests",
    );
    await expect(player.getByTestId("world-dock-tab-requests")).toHaveCount(0);

    const panel = player.getByTestId("dice-roller-panel");
    const roll = panel.getByTestId("dice-roll-button");
    await expect(roll).toHaveAccessibleName("Roll");
    await expect(roll.locator("svg")).toBeVisible();

    await panel.getByTestId("dice-formula-help-button").click();
    const help = player.getByRole("dialog", {
      name: "Writing a dice formula",
    });
    await expect(help).toBeVisible();
    await expect(help.getByTestId("dice-formula-help")).toContainText(
      "2d20kh1",
    );
    await expect(help).toContainText("Advantage");
    await player.keyboard.press("Escape");
    await expect(help).toBeHidden();

    // The die still rolls.
    await panel.getByTestId("dice-formula-input").fill("1d20");
    await roll.click();
    await expect(panel.getByTestId("dice-roll-result")).toContainText("1d20", {
      timeout: 15_000,
    });
  } finally {
    await player.context().close();
  }
});
