import { test, expect } from "./fixtures/test";
import { type GmToolId } from "./fixtures/helpers";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  waitForEngineReady,
  clickCanvasAt,
} from "./fixtures/canvasAuthoring";

test.describe("Switching tools never authors (spec 031 FR-040, SC-008)", () => {
  test("no ordered pair of tool switches places anything", async ({ page }) => {
    test.setTimeout(240_000);

    // The defect this locks down: every engine authoring system used to be
    // gated on `IsGameMaster` alone, so all of them were armed at once and a
    // single canvas click was offered to each in turn. Switching tools placed
    // a stray marker — for every tool *except* text, which is the one sub-tool
    // handled in the DOM and therefore the only one that stops listening when
    // its panel unmounts.
    //
    // Every *ordered* pair, not just the pair someone happened to notice. The
    // asymmetry is the whole lesson of this bug: it was reported as "changing
    // to the light tool", and the cause had nothing to do with lights.
    await registerAndCreateWorld(page, `E2E Tool Switch ${uniqueSuffix()}`);
    await createScene(page, "Tool Switch Scene");
    await waitForEngineReady(page);

    const counts = async () =>
      page.evaluate(() => {
        const state = window.__worldProbe?.state();
        return state
          ? {
              tokens: state.counts.tokens ?? 0,
              walls: state.counts.walls ?? 0,
              lights: state.counts.lights ?? 0,
              shapes: state.counts.shapes ?? 0,
            }
          : null;
      });

    /**
     * Click a tool in the rail, without assuming it opens a panel.
     *
     * `openGmTool` waits for that tool's flyout, which is right for tools that
     * have one — and Select deliberately does not. It "renders no flyout: a
     * panel open by default would cover 256px of map on every load to say you
     * may now click things". So the shared helper cannot express "arm Select",
     * and this test needs exactly that, for every tool including Select.
     */
    const armTool = async (tool: GmToolId) => {
      await page.getByTestId(`gm-tool-${tool}`).click();
      await expect(page.getByTestId(`gm-tool-${tool}`)).toHaveAttribute(
        "aria-expanded",
        /.*/,
      );
    };

    const before = await counts();
    expect(
      before,
      "the world probe should be available in a dev build",
    ).not.toBeNull();

    const tools: GmToolId[] = [
      "select",
      "walls",
      "lights",
      "shapes",
      "tokens",
      "interactions",
    ];

    for (const from of tools) {
      for (const to of tools) {
        if (from === to) continue;
        await armTool(from);
        await armTool(to);

        const after = await counts();
        expect(
          after,
          `switching from ${from} to ${to} changed what is on the scene`,
        ).toEqual(before);
      }
    }
  });

  test("closing the rail disarms the tool, so a click on the map places nothing", async ({
    page,
  }) => {
    test.setTimeout(180_000);

    // Playtest 2026-09-10 P2, the "yellow dot" of two playtests. The test
    // above proves switching tools is inert; this is the case it could not
    // see. Closing the rail used to leave the engine armed with the last
    // tool, so after Lights every click on the map placed a light — a small
    // amber square — with nothing on screen saying a tool was armed.
    await registerAndCreateWorld(page, `E2E Rail Closed ${uniqueSuffix()}`);
    await createScene(page, "Rail Closed Scene");
    await waitForEngineReady(page);

    const lights = async () =>
      page.evaluate(() => window.__worldProbe?.state()?.counts.lights ?? null);
    const before = await lights();
    expect(
      before,
      "the world probe should be available in a dev build",
    ).not.toBeNull();

    // The control: with Lights armed, a click on the map does place a light.
    // Without it, "nothing was placed" below could just mean the click never
    // reached the engine.
    const lightsTool = page.getByTestId("gm-tool-lights");
    await lightsTool.click();
    await expect(lightsTool).toHaveAttribute("aria-expanded", "true");
    await clickCanvasAt(page, 120, 80);
    await expect.poll(lights, { timeout: 15_000 }).toBe(before! + 1);

    // Close the rail by clicking the open tool again.
    await lightsTool.click();
    await expect(lightsTool).toHaveAttribute("aria-expanded", "false");

    await clickCanvasAt(page, -160, -40);
    await clickCanvasAt(page, 60, -120);
    // Long enough for a would-be create to make its round trip — asserting
    // that something never happened needs a moment for it not to.
    await page.waitForTimeout(2_000);
    expect(await lights(), "a closed rail must place nothing").toBe(
      before! + 1,
    );
  });
});
