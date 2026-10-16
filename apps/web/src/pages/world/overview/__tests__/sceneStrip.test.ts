import { describe, expect, it } from "vitest";
import type { SceneHeadline } from "@/api/worldStatistics";
import {
  pictureOf,
  SCENE_CARD_COUNT,
  scenesForStrip,
} from "@/pages/world/overview/sceneStrip";

function scene(id: string, day: number): SceneHeadline {
  return {
    sceneId: id,
    name: id,
    updatedAt: new Date(Date.UTC(2026, 0, day)).toISOString(),
    hidden: false,
    previewUrl: null,
    backgroundUrl: null,
  };
}

describe("scenesForStrip", () => {
  const scenes = [1, 2, 3, 4, 5, 6, 7].map((day) => scene(`s${day}`, day));

  it("puts the table's scene first, even when it is the oldest", () => {
    const order = scenesForStrip(scenes, "s1").map((s) => s.sceneId);
    expect(order).toEqual(["s1", "s7", "s6", "s5", "s4"]);
  });

  it("is newest first when no scene is active", () => {
    const order = scenesForStrip(scenes, null).map((s) => s.sceneId);
    expect(order).toEqual(["s7", "s6", "s5", "s4", "s3"]);
  });

  it("ignores an active scene this viewer was not sent", () => {
    expect(scenesForStrip(scenes, "hidden")).toHaveLength(SCENE_CARD_COUNT);
    expect(scenesForStrip(scenes, "hidden")[0].sceneId).toBe("s7");
  });

  it("draws the preview before the full background", () => {
    const both = { ...scene("a", 1), previewUrl: "p", backgroundUrl: "b" };
    expect(pictureOf(both)).toBe("p");
    expect(pictureOf({ ...both, previewUrl: null })).toBe("b");
    expect(pictureOf(scene("none", 1))).toBeNull();
  });
});
