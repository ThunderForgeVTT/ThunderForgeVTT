import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LightRecord } from "@/types/light";

/**
 * The light mutation bridge sends one light's edits one at a time (spec 045
 * FR-061).
 *
 * The pre-playtest run of 2026-09-15 stored a light at 40 ft dim and 5 ft
 * bright. Clicking the light to select it sends a move; the Game Master then
 * set the dim reach. Sent together, the move's answer, still carrying the old
 * 10 ft reach, landed after the dim reach's and put 10 ft back in the store,
 * and the panel refused the 20 ft bright reach as past it. These pin the
 * order: an edit is not sent until the one before it on the same light has
 * been answered, so the last answer is always the newest.
 */

const updateLight = vi.fn();
const deleteLight = vi.fn();

vi.mock("@/api/lights", () => ({
  createLight: vi.fn(),
  updateLight: (...args: unknown[]) => updateLight(...args),
  deleteLight: (...args: unknown[]) => deleteLight(...args),
  getLights: vi.fn(),
}));

const { createWorldStore } = await import("../../store");
const { startLightMutationBridge } = await import("../lights");

const LIGHT = "light-1";

function record(radius: number, brightRadius: number): LightRecord {
  return {
    lightId: LIGHT,
    sceneId: "scene-1",
    x: 25,
    y: 25,
    radius,
    brightRadius,
    intensity: 1,
    color: null,
    attachedTokenId: null,
    castsShadows: true,
    metadata: null,
    createdBy: "gm",
    updatedBy: "gm",
    createdAt: "2026-09-15T00:00:00",
    updatedAt: "2026-09-15T00:00:00",
  } as LightRecord;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

describe("the light mutation bridge", () => {
  beforeEach(() => {
    updateLight.mockReset();
    deleteLight.mockReset();
  });

  it("sends a light's next edit only once the last one is answered, and keeps the newest answer", async () => {
    const store = createWorldStore({ worldId: "world-1" });
    store.dispatch({ type: "upsert_light", light: { ...toLight(100, 50) } });
    startLightMutationBridge(store, "scene-1");

    const move = deferred<LightRecord>();
    const dim = deferred<LightRecord>();
    updateLight
      .mockReturnValueOnce(move.promise)
      .mockReturnValueOnce(dim.promise);

    store.dispatch(
      { type: "update_light", lightId: LIGHT, changes: { x: 25, y: 25 } },
      "bevy",
    );
    store.dispatch(
      { type: "update_light", lightId: LIGHT, changes: { radius: 400 } },
      "ui",
    );
    await settle();
    expect(
      updateLight,
      "the dim reach waits on the move",
    ).toHaveBeenCalledTimes(1);

    move.resolve(record(100, 50));
    await settle();
    expect(updateLight).toHaveBeenCalledTimes(2);
    expect(updateLight.mock.calls[1][1]).toMatchObject({ radius: 400 });

    dim.resolve(record(400, 50));
    await settle();
    expect(store.getState().lights[LIGHT]).toMatchObject({
      radius: 400,
      brightRadius: 50,
    });
  });

  it("goes on to the next edit when one fails", async () => {
    const store = createWorldStore({ worldId: "world-1" });
    startLightMutationBridge(store, "scene-1");
    const errors = vi.spyOn(console, "error").mockImplementation(() => {});
    updateLight
      .mockRejectedValueOnce(new Error("refused"))
      .mockResolvedValueOnce(record(400, 200));

    store.dispatch(
      { type: "update_light", lightId: LIGHT, changes: { radius: 50 } },
      "ui",
    );
    store.dispatch(
      {
        type: "update_light",
        lightId: LIGHT,
        changes: { radius: 400, brightRadius: 200 },
      },
      "ui",
    );
    await settle();
    await settle();
    expect(updateLight).toHaveBeenCalledTimes(2);
    expect(store.getState().lights[LIGHT]).toMatchObject({
      radius: 400,
      brightRadius: 200,
    });
    errors.mockRestore();
  });

  it("deletes a light only after its edits are answered", async () => {
    const store = createWorldStore({ worldId: "world-1" });
    startLightMutationBridge(store, "scene-1");
    const edit = deferred<LightRecord>();
    updateLight.mockReturnValueOnce(edit.promise);
    deleteLight.mockResolvedValue(true);

    store.dispatch(
      { type: "update_light", lightId: LIGHT, changes: { radius: 400 } },
      "ui",
    );
    store.dispatch({ type: "delete_light", lightId: LIGHT }, "ui");
    await settle();
    expect(deleteLight).not.toHaveBeenCalled();

    edit.resolve(record(400, 50));
    await settle();
    await settle();
    expect(deleteLight).toHaveBeenCalledTimes(1);
    expect(store.getState().lights[LIGHT]).toBeUndefined();
  });
});

function toLight(radius: number, brightRadius: number) {
  const r = record(radius, brightRadius);
  return {
    id: r.lightId,
    sceneId: r.sceneId,
    x: r.x,
    y: r.y,
    radius,
    brightRadius,
    intensity: r.intensity,
    color: r.color,
    attachedTokenId: r.attachedTokenId,
    castsShadows: r.castsShadows,
  };
}
