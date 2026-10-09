import { describe, expect, it } from "vitest";
import { parseMapImportedEvent, parseSceneLaunchedEvent } from "../scenes";

/**
 * The two scene-wide world events: a scene launched (16), and a map imported
 * into one (13). Each parser must answer only for its own code, or a launch
 * would reload the board's content and an import would switch scenes.
 */
describe("parseMapImportedEvent", () => {
  it("names the scene a map was imported into", () => {
    expect(
      parseMapImportedEvent({
        event_code: 13,
        token_event: { scene_id: "scene-1", background_image_set: true },
      }),
    ).toBe("scene-1");
    expect(
      parseMapImportedEvent({
        eventCode: 13,
        tokenEvent: { scene_id: "scene-2" },
      }),
    ).toBe("scene-2");
  });

  it("says nothing about any other event", () => {
    expect(
      parseMapImportedEvent({
        event_code: 16,
        token_event: { sceneId: "scene-1" },
      }),
    ).toBeNull();
    expect(
      parseSceneLaunchedEvent({
        event_code: 13,
        token_event: { scene_id: "scene-1" },
      }),
    ).toBeNull();
  });
});
