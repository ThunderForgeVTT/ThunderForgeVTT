import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ShapeRecord } from "@/types/shape";

/**
 * A drawing the server stops returning leaves the board (spec 073 FR-007).
 *
 * A shape event carries only an id, so the sync re-fetches the scene's
 * drawings. A player is never sent a hidden one, which means a drawing the
 * Game Master has just hidden is simply missing from the answer. The sync
 * used to add what it was given and nothing more, so the player kept looking
 * at the drawing until they reloaded.
 */

const getShapes = vi.fn();
const updateShape = vi.fn();
const deleteShape = vi.fn();
const clearShapes = vi.fn();

vi.mock("@/api/shapes", () => ({
  createShape: vi.fn(),
  updateShape: (...args: unknown[]) => updateShape(...args),
  deleteShape: (...args: unknown[]) => deleteShape(...args),
  clearShapes: (...args: unknown[]) => clearShapes(...args),
  getShapes: (...args: unknown[]) => getShapes(...args),
}));

const { createWorldStore } = await import("../../store");
const { applyShapeWorldEvent, startShapeMutationBridge } =
  await import("../shapes");

const SCENE = "scene-1";

function record(shapeId: string): ShapeRecord {
  return {
    shapeId,
    sceneId: SCENE,
    levelId: "level-1",
    kind: "RECT",
    geometry: { x: 0, y: 0, width: 10, height: 10 },
    text: null,
    style: null,
    visibleToPlayers: true,
    metadata: null,
    createdBy: "gm",
    updatedBy: "gm",
    createdAt: "2026-10-05T00:00:00",
    updatedAt: "2026-10-05T00:00:00",
  } as ShapeRecord;
}

const updated = (shapeId: string) => ({
  event_code: 12,
  token_event: { action: "updated", shape_id: shapeId, scene_id: SCENE },
});

describe("applyShapeWorldEvent", () => {
  beforeEach(() => {
    getShapes.mockReset();
  });

  it("drops the drawing an update names once the server no longer returns it", async () => {
    const store = createWorldStore({ worldId: "world-1" });
    getShapes.mockResolvedValueOnce([record("a"), record("b")]);
    await applyShapeWorldEvent(store, SCENE, updated("a"));
    expect(Object.keys(store.getState().shapes).sort()).toEqual(["a", "b"]);

    getShapes.mockResolvedValueOnce([record("b")]);
    await applyShapeWorldEvent(store, SCENE, updated("a"));
    expect(Object.keys(store.getState().shapes)).toEqual(["b"]);
  });

  it("carries who drew a shape into the store, and so to the engine (spec 082)", async () => {
    const store = createWorldStore({ worldId: "world-1" });
    getShapes.mockResolvedValueOnce([record("a")]);
    await applyShapeWorldEvent(store, SCENE, updated("a"));
    expect(store.getState().shapes.a?.createdBy).toBe("gm");
  });

  it("keeps a drawing the update does not name", async () => {
    const store = createWorldStore({ worldId: "world-1" });
    getShapes.mockResolvedValueOnce([record("a"), record("b")]);
    await applyShapeWorldEvent(store, SCENE, updated("a"));

    getShapes.mockResolvedValueOnce([record("a")]);
    await applyShapeWorldEvent(store, SCENE, updated("a"));
    expect(Object.keys(store.getState().shapes).sort()).toEqual(["a", "b"]);
  });
});

/**
 * A refused edit puts the drawing back (spec 082 FR-006). The engine moves a
 * drawing as it is dragged, before the server has answered; when the server
 * says no — it is somebody else's — the board must show it where it was.
 */
describe("startShapeMutationBridge rolls back a refusal", () => {
  beforeEach(() => {
    updateShape.mockReset();
    deleteShape.mockReset();
  });

  function storeHolding() {
    const store = createWorldStore({ worldId: "world-1" });
    store.dispatch(
      {
        type: "upsert_shape",
        shape: {
          id: "a",
          sceneId: SCENE,
          kind: "rect",
          geometry: { x: 0, y: 0, w: 10, h: 10 },
          text: null,
          style: null,
          visibleToPlayers: true,
          createdBy: "gm",
        },
      },
      "sync",
    );
    const restored: unknown[] = [];
    store.subscribe((event) => {
      if (event.command.type === "upsert_shape" && event.source === "sync") {
        restored.push(event.command.shape);
      }
    });
    return { store, restored };
  }

  it("restores the drawing when an update is refused", async () => {
    const { store, restored } = storeHolding();
    const before = store.getState().shapes.a;
    updateShape.mockRejectedValueOnce(new Error("Shape not found"));
    startShapeMutationBridge(store, SCENE);
    store.dispatch(
      {
        type: "update_shape",
        shapeId: "a",
        changes: { geometry: { x: 50, y: 50, w: 10, h: 10 } },
      },
      "bevy",
    );
    await vi.waitFor(() => expect(restored).toEqual([before]));
    expect(store.getState().shapes.a).toEqual(before);
  });

  it("restores the drawing when a delete answers false", async () => {
    const { store, restored } = storeHolding();
    const before = store.getState().shapes.a;
    deleteShape.mockResolvedValueOnce(false);
    startShapeMutationBridge(store, SCENE);
    store.dispatch({ type: "delete_shape", shapeId: "a" }, "ui");
    await vi.waitFor(() => expect(restored).toEqual([before]));
    expect(store.getState().shapes.a).toEqual(before);
  });

  it("restores the drawing when a delete is refused outright", async () => {
    const { store, restored } = storeHolding();
    const before = store.getState().shapes.a;
    deleteShape.mockRejectedValueOnce(new Error("refused"));
    startShapeMutationBridge(store, SCENE);
    store.dispatch({ type: "delete_shape", shapeId: "a" }, "ui");
    await vi.waitFor(() => expect(restored).toEqual([before]));
  });
});

/**
 * The Game Master clears a scene (spec 082 FR-014). Nothing leaves the board
 * on the asking: the drawings go when the server's `deleted` events arrive,
 * so every board, the asker's included, empties from the same answer.
 */
describe("clearing a scene's drawings", () => {
  beforeEach(() => {
    clearShapes.mockReset();
  });

  function storeWith(...ids: string[]) {
    const store = createWorldStore({ worldId: "world-1" });
    for (const id of ids) {
      store.dispatch(
        {
          type: "upsert_shape",
          shape: {
            id,
            sceneId: SCENE,
            kind: "rect",
            geometry: { x: 0, y: 0, w: 10, h: 10 },
            text: null,
            style: null,
            visibleToPlayers: true,
            createdBy: "gm",
          },
        },
        "sync",
      );
    }
    return store;
  }

  it("sends clearShapes with the creators named and removes nothing itself", async () => {
    const store = storeWith("a", "b");
    clearShapes.mockResolvedValueOnce(2);
    startShapeMutationBridge(store, SCENE);
    store.dispatch(
      { type: "clear_shapes", sceneId: SCENE, createdBy: ["player-a"] },
      "ui",
    );
    await vi.waitFor(() =>
      expect(clearShapes).toHaveBeenCalledWith(SCENE, ["player-a"]),
    );
    expect(Object.keys(store.getState().shapes).sort()).toEqual(["a", "b"]);
  });

  it("sends no creators when every drawing goes", async () => {
    const store = storeWith("a");
    clearShapes.mockResolvedValueOnce(1);
    startShapeMutationBridge(store, SCENE);
    store.dispatch({ type: "clear_shapes", sceneId: SCENE }, "ui");
    await vi.waitFor(() =>
      expect(clearShapes).toHaveBeenCalledWith(SCENE, undefined),
    );
  });

  it("lets the deleted events take the drawings away", async () => {
    const store = storeWith("a", "b");
    for (const id of ["a", "b"]) {
      await applyShapeWorldEvent(store, SCENE, {
        event_code: 12,
        token_event: { action: "deleted", shape_id: id, scene_id: SCENE },
      });
    }
    expect(store.getState().shapes).toEqual({});
  });
});
