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

vi.mock("@/api/shapes", () => ({
  createShape: vi.fn(),
  updateShape: vi.fn(),
  deleteShape: vi.fn(),
  getShapes: (...args: unknown[]) => getShapes(...args),
}));

const { createWorldStore } = await import("../../store");
const { applyShapeWorldEvent } = await import("../shapes");

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
