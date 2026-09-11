import { describe, expect, it } from "vitest";
import { screenToWorld } from "../screenToWorld";

/**
 * Playtest 2026-09-10 P10: text placed with the Text tool lands where it was
 * clicked. The engine's world is centred on the camera and grows upward.
 */
describe("screenToWorld", () => {
  const canvas = { left: 0, top: 0, width: 1200, height: 800 };

  it("puts the canvas centre at the camera", () => {
    expect(
      screenToWorld({ x: 600, y: 400 }, canvas, { x: 35, y: -20, scale: 1 }),
    ).toEqual({ x: 35, y: -20 });
  });

  it("flips y: a click above the centre is higher on the map", () => {
    const at = screenToWorld({ x: 600, y: 300 }, canvas, {
      x: 0,
      y: 0,
      scale: 1,
    });
    expect(at.y).toBe(100);
  });

  it("scales with zoom: zoomed out, a pixel covers more of the map", () => {
    expect(
      screenToWorld({ x: 700, y: 400 }, canvas, { x: 0, y: 0, scale: 3 }),
    ).toEqual({ x: 300, y: 0 });
  });

  it("measures from the canvas, wherever it sits on the page", () => {
    expect(
      screenToWorld(
        { x: 150, y: 90 },
        { left: 50, top: 40, width: 200, height: 100 },
        { x: 10, y: 10, scale: 2 },
      ),
    ).toEqual({ x: 10, y: 10 });
  });
});
