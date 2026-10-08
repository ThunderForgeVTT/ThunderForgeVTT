import { beforeEach, describe, expect, it, vi } from "vitest";
import type { WallRecord } from "@/types/wall";
import type { WorldWall } from "../../types";

/**
 * Spec 085: the wall bridge hides walls one mutation per wall, puts back what
 * the server refuses, and reports a group's refusals in one message.
 */

const updateWall = vi.fn();
const deleteWall = vi.fn();
const setDoorSecret = vi.fn();
const warning = vi.fn();

vi.mock("@/api/walls", () => ({
  createWall: vi.fn(),
  updateWall: (...args: unknown[]) => updateWall(...args),
  deleteWall: (...args: unknown[]) => deleteWall(...args),
  getWalls: vi.fn(),
}));

vi.mock("@/api/interactives", () => ({
  setDoorSecret: (...args: unknown[]) => setDoorSecret(...args),
}));

vi.mock("sonner", () => ({
  toast: { warning: (...args: unknown[]) => warning(...args) },
}));

const { createWorldStore } = await import("../../store");
const { startWallMutationBridge } = await import("../walls");
const { forgetAllGroups } = await import("../groupMoves");

function wall(id: string, x1 = 0): WorldWall {
  return {
    id,
    sceneId: "scene-1",
    x1,
    y1: 0,
    x2: x1 + 100,
    y2: 0,
    blocksVision: true,
    blocksMovement: true,
    doorState: "none",
    locked: false,
    secret: false,
  };
}

function record(w: WorldWall): WallRecord {
  return {
    wallId: w.id,
    sceneId: w.sceneId,
    levelId: "level-1",
    x1: w.x1,
    y1: w.y1,
    x2: w.x2,
    y2: w.y2,
    blocksVision: w.blocksVision,
    blocksMovement: w.blocksMovement,
    doorState: "NONE",
    locked: w.locked ?? false,
    secret: w.secret ?? false,
    metadata: null,
    createdBy: "gm",
    updatedBy: "gm",
  } as WallRecord;
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

const settle = () => new Promise((r) => setTimeout(r, 0));

function seeded() {
  const store = createWorldStore({ worldId: "world-1" });
  for (const id of ["w1", "w2", "w3"]) {
    store.dispatch({ type: "upsert_wall", wall: wall(id) }, "sync");
  }
  startWallMutationBridge(store, "scene-1");
  return store;
}

describe("the wall mutation bridge", () => {
  beforeEach(() => {
    updateWall.mockReset();
    deleteWall.mockReset();
    setDoorSecret.mockReset();
    warning.mockReset();
    forgetAllGroups();
  });

  it("hides three walls with one mutation each", async () => {
    const store = seeded();
    setDoorSecret.mockResolvedValue(true);

    store.dispatch({
      type: "set_walls_hidden",
      wallIds: ["w1", "w2", "w3"],
      hidden: true,
    });
    await settle();

    expect(setDoorSecret.mock.calls).toEqual([
      ["w1", true],
      ["w2", true],
      ["w3", true],
    ]);
    expect(store.getState().walls.w2.secret).toBe(true);
    expect(warning).not.toHaveBeenCalled();
  });

  it("sends a wall's hide only once its last edit is answered", async () => {
    const store = seeded();
    const move = deferred<WallRecord>();
    updateWall.mockReturnValueOnce(move.promise);
    setDoorSecret.mockResolvedValue(true);

    store.dispatch({
      type: "update_wall",
      wallId: "w1",
      changes: { x1: 50 },
    });
    store.dispatch({ type: "set_walls_hidden", wallIds: ["w1"], hidden: true });
    await settle();
    expect(setDoorSecret).not.toHaveBeenCalled();

    move.resolve(record(wall("w1", 50)));
    await settle();
    expect(setDoorSecret).toHaveBeenCalledWith("w1", true);
  });

  it("puts back only the refused wall, and says so once", async () => {
    const store = seeded();
    setDoorSecret
      .mockResolvedValueOnce(true)
      .mockRejectedValueOnce(new Error("no"))
      .mockResolvedValueOnce(true);
    const restored: string[] = [];
    store.subscribe((event) => {
      if (event.command.type === "upsert_wall" && event.source === "sync") {
        restored.push(event.command.wall.id);
      }
    });

    store.dispatch({
      type: "set_walls_hidden",
      wallIds: ["w1", "w2", "w3"],
      hidden: true,
    });
    await settle();

    expect(restored).toEqual(["w2"]);
    const walls = store.getState().walls;
    expect([walls.w1.secret, walls.w2.secret, walls.w3.secret]).toEqual([
      true,
      false,
      true,
    ]);
    expect(warning).toHaveBeenCalledTimes(1);
    expect(warning).toHaveBeenCalledWith("1 of 3 could not be hidden.");
  });

  it("treats a false answer to a hide as a refusal", async () => {
    const store = seeded();
    setDoorSecret.mockResolvedValue(false);

    store.dispatch({ type: "set_walls_hidden", wallIds: ["w1"], hidden: true });
    await settle();

    expect(store.getState().walls.w1.secret).toBe(false);
  });

  it("puts back the wall as it stood before a refused group move", async () => {
    const store = seeded();
    updateWall.mockRejectedValue(new Error("no"));
    const group = { id: "g-1", size: 1 };

    store.dispatch(
      {
        type: "update_wall",
        wallId: "w1",
        changes: { x1: 40, x2: 140 },
        group,
      },
      "bevy",
    );
    await settle();

    expect(store.getState().walls.w1.x1).toBe(0);
    expect(warning).toHaveBeenCalledWith("1 of 1 could not be moved.");
  });

  it("puts back a wall whose delete the server refused", async () => {
    const store = seeded();
    deleteWall.mockResolvedValue(false);
    const restored: string[] = [];
    store.subscribe((event) => {
      if (event.command.type === "upsert_wall" && event.source === "sync") {
        restored.push(event.command.wall.id);
      }
    });

    // The engine took it off its board before asking.
    store.dispatch(
      { type: "delete_wall", wallId: "w2", group: { id: "g-2", size: 1 } },
      "bevy",
    );
    await settle();

    expect(restored).toEqual(["w2"]);
    expect(store.getState().walls.w2).toBeDefined();
    expect(warning).toHaveBeenCalledWith("1 of 1 could not be deleted.");
  });
});
