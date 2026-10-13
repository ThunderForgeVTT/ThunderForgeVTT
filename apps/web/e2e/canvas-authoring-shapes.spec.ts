import { test, expect } from "./fixtures/test";
import { waitForShapesLoaded } from "./fixtures/helpers";
import {
  uniqueSuffix,
  registerAndCreateWorld,
  createScene,
  switchToScene,
  canvasBox,
  waitForEngineReady,
  clickCanvasAt,
  dragCanvas,
} from "./fixtures/canvasAuthoring";

/**
 * T016-T020 (specs/002-canvas-authoring-asset-storage, closing T067's
 * hand-drawn-shape gap): freehand/rectangle/ellipse/line/text shape
 * creation, scene-switch isolation, and delete. Uses the engine's own
 * `1`-`5` keyboard shortcuts (`handle_shape_tool_selection`,
 * systems/shape.rs) to pick a sub-tool rather than ShapeTool.tsx's
 * toolbar buttons: those buttons only set local React UI state today and
 * were never wired to the engine's `ActiveShapeTool` resource (the same
 * category of gap `WallTool.tsx`'s "Draw wall" toggle has, pre-existing,
 * not fixed here — out of this pass's file scope). The Text sub-tool is
 * the one exception with a genuinely working end-to-end UI path
 * (`ShapeTool.tsx`'s own click-to-place popover), so it's driven through
 * its real button instead.
 */
test.describe("Hand-drawn shape authoring (US2)", () => {
  test("freehand, rectangle, ellipse, line/arrow, and text shapes can each be created and selected", async ({
    page,
  }) => {
    // Registration, world creation, a scene launch (a full navigation) and
    // the engine's own multi-second WASM startup do not leave room inside
    // Playwright's 30-second default for the drawing this test is about — it
    // was dying mid-setup, which looks like a hang rather than a clock.
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E Shapes ${uniqueSuffix()}`);
    await createScene(page, "Shape Scene");
    await waitForEngineReady(page, "shapes");
    await expect(page.getByTestId("shape-tool")).toBeVisible();

    // Freehand (key 1): a short multi-point drag.
    await page.keyboard.press("1");
    await dragCanvas(page, { dx: -220, dy: -150 }, { dx: -180, dy: -110 });

    // Rectangle (key 2): bounding box (-60,-150)-(20,-100), anchor (center)
    // at (-20,-125).
    await page.keyboard.press("2");
    await dragCanvas(page, { dx: -60, dy: -150 }, { dx: 20, dy: -100 });

    // Ellipse (key 3): bounding box (100,-150)-(200,-100), anchor at (150,-125).
    await page.keyboard.press("3");
    await dragCanvas(page, { dx: 100, dy: -150 }, { dx: 200, dy: -100 });

    // Line/arrow (key 4): anchor is the segment midpoint, (-150,75).
    await page.keyboard.press("4");
    await dragCanvas(page, { dx: -200, dy: 50 }, { dx: -100, dy: 100 });

    // Confirm the drag-drawn shapes are each selectable (existence proxy,
    // same technique the wall tests use — there is no shape-count testid).
    // Escape first to leave the ellipse/line drag-tool mode and drop back
    // to plain select-by-click (systems/shape.rs's handle_shape_tool_selection).
    await page.keyboard.press("Escape");
    // Escape also puts the Shapes tool down (back to Select, owner
    // request 2026-10-06), and "Selected shape" is the Shapes panel's.
    await page.getByTestId("gm-tool-shapes").click();
    await expect(page.getByTestId("shape-tool")).toBeVisible();
    await clickCanvasAt(page, -20, -125); // rectangle center
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    await clickCanvasAt(page, 150, -125); // ellipse center
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    await clickCanvasAt(page, -150, 75); // line midpoint
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });

    // Text: the one sub-tool with a real, already-working UI path —
    // ShapeTool.tsx's own click-to-place popover, not engine tool state.
    await page.getByRole("button", { name: "Text", exact: true }).click();
    const box = await canvasBox(page);
    await page.mouse.click(
      box.x + box.width / 2 + 100,
      box.y + box.height / 2 + 100,
    );
    await expect(page.getByTestId("shape-text-popover")).toBeVisible();
    await page.getByLabel("Text").fill("Trap!");
    await page.getByRole("button", { name: "Add text" }).click();
    await expect(page.getByTestId("shape-text-popover")).toBeHidden();
  });

  test("shapes are isolated per scene across 3 scene switches, and deleting one removes it", async ({
    page,
  }) => {
    // Registration, world creation, a scene launch (a full navigation) and
    // the engine's own multi-second WASM startup do not leave room inside
    // Playwright's 30-second default for the drawing this test is about — it
    // was dying mid-setup, which looks like a hang rather than a clock.
    test.setTimeout(240_000);

    await registerAndCreateWorld(page, `E2E Shape Isolation ${uniqueSuffix()}`);
    await createScene(page, "Shape Scene A");
    await waitForEngineReady(page, "shapes");

    await page.keyboard.press("2");
    await dragCanvas(page, { dx: -60, dy: -60 }, { dx: 60, dy: 60 });
    await page.keyboard.press("Escape");
    // Escape also puts the Shapes tool down (back to Select, owner
    // request 2026-10-06), and "Selected shape" is the Shapes panel's.
    await page.getByTestId("gm-tool-shapes").click();
    await expect(page.getByTestId("shape-tool")).toBeVisible();
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });

    await createScene(page, "Shape Scene B");
    await waitForEngineReady(page, "shapes");
    // A freshly created scene must not show Scene A's rectangle.
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toHaveCount(0);

    // Switch A -> B -> A -> B -> A (>= 3 switches, per SC-003) confirming
    // isolation holds each time, not just once.
    for (let i = 0; i < 3; i++) {
      await switchToScene(page, "Shape Scene A");
      await waitForEngineReady(page, "shapes");
      // A scene switch refetches that scene's shapes over their own round
      // trip; a click that lands first selects nothing. See
      // `waitForShapesLoaded`.
      await waitForShapesLoaded(page, 1);
      await clickCanvasAt(page, 0, 0);
      await expect(page.getByText("Selected shape")).toBeVisible({
        timeout: 10_000,
      });

      await switchToScene(page, "Shape Scene B");
      await waitForEngineReady(page, "shapes");
      await clickCanvasAt(page, 0, 0);
      await expect(page.getByText("Selected shape")).toHaveCount(0);
    }

    // Delete: back on Scene A, remove the rectangle and confirm it's gone.
    await switchToScene(page, "Shape Scene A");
    await waitForEngineReady(page, "shapes");
    await waitForShapesLoaded(page, 1);
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    await page.getByRole("button", { name: "Delete shape" }).click();
    await expect(page.getByText("Selected shape")).toHaveCount(0);

    await page.reload();
    await waitForEngineReady(page, "shapes");
    // No `waitForShapesLoaded` here, deliberately: the scene's only shape is
    // the one just deleted, so polling for one to arrive would wait out its
    // whole timeout on something that must never come back. The engine-ready
    // settle is what gives the refetch its chance to land.
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toHaveCount(0);
  });
});

test.describe("The Shapes panel's own buttons (playtest 2026-09-10 P10)", () => {
  test("clicking a shape button and dragging draws it; text lands where it is clicked; a swatch recolours", async ({
    page,
  }) => {
    test.setTimeout(240_000);

    // The buttons used to change only React state: the engine's shape kind
    // was reachable from the number keys alone, and the test above presses
    // 1–4, which is why it passed while clicking "Rectangle" drew nothing.
    // Nothing here presses a number key.
    await registerAndCreateWorld(page, `E2E Shape Buttons ${uniqueSuffix()}`);
    await createScene(page, "Shape Buttons Scene");
    await waitForEngineReady(page, "shapes");
    await expect(page.getByTestId("shape-tool")).toBeVisible();

    const shapeCount = async () =>
      page.evaluate(() => window.__worldProbe?.state()?.counts.shapes ?? null);
    const before = await shapeCount();
    expect(
      before,
      "the world probe should be available in a dev build",
    ).not.toBeNull();

    const rectangle = page.getByRole("button", { name: "Rectangle" });
    await rectangle.click();
    await expect(rectangle).toHaveAttribute("aria-pressed", "true");
    await dragCanvas(page, { dx: -60, dy: -60 }, { dx: 60, dy: 60 });
    await expect
      .poll(shapeCount, {
        timeout: 15_000,
        message: "a drag after clicking Rectangle must draw one",
      })
      .toBe(before! + 1);

    const ellipse = page.getByRole("button", { name: "Ellipse" });
    await ellipse.click();
    await dragCanvas(page, { dx: 140, dy: -60 }, { dx: 240, dy: 0 });
    await expect.poll(shapeCount, { timeout: 15_000 }).toBe(before! + 2);

    // Text is placed at the map point under the click — the engine's world is
    // centred on the camera and grows upward — not at screen pixels.
    await page.getByRole("button", { name: "Text", exact: true }).click();
    const camera = await page.evaluate(
      () =>
        (
          window as unknown as {
            __engineProbe?: {
              camera: () => { x: number; y: number; scale: number } | null;
            };
          }
        ).__engineProbe?.camera() ?? null,
    );
    expect(camera, "the engine probe should be available").not.toBeNull();
    const box = await canvasBox(page);
    await page.mouse.click(
      box.x + box.width / 2 + 100,
      box.y + box.height / 2 + 100,
    );
    const popover = page.getByTestId("shape-text-popover");
    await expect(popover).toBeVisible();
    const expected = {
      x: Math.round(camera!.x + 100 * camera!.scale),
      y: Math.round(camera!.y - 100 * camera!.scale),
    };
    await expect(popover).toContainText(
      `Placing text at (${expected.x}, ${expected.y})`,
    );
    await page.getByLabel("Text").fill("Trap!");
    await page.getByRole("button", { name: "Add text" }).click();
    await expect.poll(shapeCount, { timeout: 15_000 }).toBe(before! + 3);

    // Disarm (clicking the pressed button again), select the rectangle, and
    // recolour it: the swatch writes the index the engine actually reads.
    await page.getByRole("button", { name: "Text", exact: true }).click();
    await clickCanvasAt(page, 0, 0);
    await expect(page.getByText("Selected shape")).toBeVisible({
      timeout: 10_000,
    });
    const red = page
      .getByTestId("shape-color-swatches")
      .getByRole("button", { name: "Red" });
    await red.click();
    await expect(red).toHaveAttribute("aria-pressed", "true");
  });
});
