import { describe, expect, it, vi } from "vitest";
import type { WorldStore } from "../../store";
import { applySceneTimeWorldEvent, asHour, setSceneTime } from "../sceneTime";

/**
 * Spec 099 US5: a scene's hour reaches the engine's clock — on load at once,
 * and from the Game Master's change with its transition, so every client
 * runs the same one.
 */
function fakeStore() {
  const dispatch = vi.fn();
  return { store: { dispatch } as unknown as WorldStore, dispatch };
}

describe("asHour", () => {
  it("reads an hour, wraps 24 to midnight, and refuses anything else", () => {
    expect(asHour(22.5)).toBe(22.5);
    expect(asHour(24)).toBe(0);
    expect(asHour(-1)).toBeNull();
    expect(asHour(25)).toBeNull();
    expect(asHour(Number.NaN)).toBeNull();
    expect(asHour("12")).toBeNull();
    expect(asHour(null)).toBeNull();
    expect(asHour(undefined)).toBeNull();
  });
});

describe("setSceneTime", () => {
  it("jumps without a transition, and passes time with one", () => {
    const { store, dispatch } = fakeStore();
    setSceneTime(store, 6);
    setSceneTime(store, 22, 2);
    setSceneTime(store, null, 2);
    expect(dispatch.mock.calls).toEqual([
      [{ type: "set_time_of_day", hour: 6 }, "sync"],
      [{ type: "set_time_of_day", hour: 22, overSecs: 2 }, "sync"],
      [{ type: "set_time_of_day", hour: null }, "sync"],
    ]);
  });
});

describe("applySceneTimeWorldEvent", () => {
  it("hands this scene's change to the engine, transition and all", () => {
    const { store, dispatch } = fakeStore();
    const applied = applySceneTimeWorldEvent(store, "scene-1", {
      event_code: 43,
      token_event: { sceneId: "scene-1", hour: 22, overSecs: 2 },
    });
    expect(applied).toEqual({ hour: 22, overSecs: 2 });
    expect(dispatch).toHaveBeenCalledWith(
      { type: "set_time_of_day", hour: 22, overSecs: 2 },
      "sync",
    );
  });

  it("takes the clock away when the hour is cleared", () => {
    const { store, dispatch } = fakeStore();
    expect(
      applySceneTimeWorldEvent(store, "scene-1", {
        eventCode: 43,
        tokenEvent: { sceneId: "scene-1", hour: null, overSecs: null },
      }),
    ).toEqual({ hour: null, overSecs: 0 });
    expect(dispatch).toHaveBeenCalledWith(
      { type: "set_time_of_day", hour: null },
      "sync",
    );
  });

  it("ignores other events, and other scenes", () => {
    const { store, dispatch } = fakeStore();
    expect(
      applySceneTimeWorldEvent(store, "scene-1", {
        event_code: 25,
        token_event: { sceneId: "scene-1", ambientLight: "dark" },
      }),
    ).toBeNull();
    expect(
      applySceneTimeWorldEvent(store, "scene-1", {
        event_code: 43,
        token_event: { sceneId: "scene-2", hour: 22 },
      }),
    ).toBeNull();
    expect(dispatch).not.toHaveBeenCalled();
  });
});
