import { describe, expect, it } from "vitest";
import { onViewedLevel, setViewedLevel, viewedLevelId } from "../viewedLevel";

/**
 * The level on screen, as the per-scene api functions learn it.
 *
 * What matters is the default: a create with no level lands on the *entry*
 * level on the server, which is wrong for a Game Master looking at another
 * floor and fails silently. These pin that a viewed level is filled in, and
 * that a caller who names one is not overruled.
 */
describe("viewedLevel", () => {
  it("knows nothing about a scene nobody has said anything about", () => {
    expect(viewedLevelId("unseen")).toBeUndefined();
    expect(onViewedLevel({ sceneId: "unseen" })).toEqual({ sceneId: "unseen" });
  });

  it("places a create on the level being viewed", () => {
    setViewedLevel("scene-a", "upstairs");
    expect(viewedLevelId("scene-a")).toBe("upstairs");
    expect(onViewedLevel({ sceneId: "scene-a", x: 1 })).toEqual({
      sceneId: "scene-a",
      x: 1,
      levelId: "upstairs",
    });
  });

  it("leaves a create that names its own level alone", () => {
    setViewedLevel("scene-b", "upstairs");
    expect(onViewedLevel({ sceneId: "scene-b", levelId: "street" })).toEqual({
      sceneId: "scene-b",
      levelId: "street",
    });
  });

  it("keeps scenes apart, and forgets one when told to", () => {
    setViewedLevel("scene-c", "cellar");
    setViewedLevel("scene-d", "attic");
    expect(viewedLevelId("scene-c")).toBe("cellar");
    setViewedLevel("scene-c", null);
    expect(viewedLevelId("scene-c")).toBeUndefined();
    expect(viewedLevelId("scene-d")).toBe("attic");
  });
});
