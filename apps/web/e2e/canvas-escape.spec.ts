import { test, expect } from "./fixtures/test";
import { clickBoard } from "./fixtures/boardPointer";
import {
  launchSceneByName,
  openGmTool,
  registerAndCreateWorld,
  uniqueSuffix,
  waitForEngineReady,
} from "./fixtures/helpers";
import { createScene } from "./fixtures/world-cache";

/**
 * Escape puts the tool down (owner request 2026-10-06): with an authoring
 * tool open it returns to Select; on Select already, it opens Settings, and
 * a second press closes it.
 */
test.describe("Escape puts the tool down", () => {
  test("Escape returns to Select, and Escape on Select opens Settings", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    await registerAndCreateWorld(page, `E2E Escape ${uniqueSuffix()}`);
    const worldId = /\/world\/([^/]+)/.exec(new URL(page.url()).pathname)![1];
    await createScene(page, worldId, "Escape Room");
    await launchSceneByName(page, worldId, "Escape Room");
    await waitForEngineReady(page);

    const dock = page.getByTestId("world-dock");
    await openGmTool(page, "walls");
    await expect(page.getByTestId("gm-tool-walls")).toHaveAttribute(
      "aria-expanded",
      "true",
    );

    await expect(dock).toHaveAttribute("data-open-section", "");
    // A click on the board first: the canvas then holds focus and the engine
    // marks every key it hears as default-prevented, which must not swallow
    // the press (this is how a GM mostly arrives at Escape).
    await clickBoard(page, { x: 0, y: 0 });
    await page.keyboard.press("Escape");
    await expect(page.getByTestId("gm-tool-panel-walls")).toBeHidden();
    await expect(page.getByTestId("gm-tool-select")).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    await expect(dock, "one Escape only drops the tool").toHaveAttribute(
      "data-open-section",
      "",
    );

    await page.keyboard.press("Escape");
    await expect(dock).toHaveAttribute("data-open-section", "settings");
    await expect(page.getByTestId("world-dock-panel-settings")).toBeVisible();

    // Escape closes what Escape opened.
    await page.keyboard.press("Escape");
    await expect(dock).toHaveAttribute("data-open-section", "");
  });
});
