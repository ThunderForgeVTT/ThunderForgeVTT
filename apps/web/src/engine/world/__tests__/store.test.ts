import { describe, expect, it } from "vitest";

import { createWorldStore } from "../store";

/**
 * A move the engine reports carries only where the token went (spec 045
 * T065). Everything else the store knows about that token — above all whose
 * it is — has to survive it, because the page names a player's token to the
 * engine from that owner, and losing it for a moment loses the keyboard.
 */
describe("upsert_token", () => {
  it("keeps a token's owner through a move that carries only its transform", () => {
    const store = createWorldStore({
      worldId: "w",
      initialTokens: [
        {
          id: "brom",
          x: 25,
          y: 25,
          z: 0,
          label: "Brom",
          ownerUserId: "u-brom",
          isPrimary: true,
          attributes: { str: 16 },
        },
      ],
    });

    store.dispatch(
      {
        type: "upsert_token",
        token: { id: "brom", x: -25, y: 25, z: 0, scale: 1, rotation: 0 },
      },
      "bevy",
    );

    expect(store.getState().tokens.brom).toEqual({
      id: "brom",
      x: -25,
      y: 25,
      z: 0,
      scale: 1,
      rotation: 0,
      label: "Brom",
      ownerUserId: "u-brom",
      isPrimary: true,
      attributes: { str: 16 },
    });
  });

  it("lets a field the command does carry win, a cleared one included", () => {
    const store = createWorldStore({
      worldId: "w",
      initialTokens: [
        { id: "t", x: 0, y: 0, z: 0, label: "Secret", ownerUserId: "u-1" },
      ],
    });

    store.dispatch(
      {
        type: "upsert_token",
        token: {
          id: "t",
          x: 0,
          y: 0,
          z: 0,
          label: undefined,
          ownerUserId: null,
        },
      },
      "sync",
    );

    expect(store.getState().tokens.t.label).toBeUndefined();
    expect(store.getState().tokens.t.ownerUserId).toBeNull();
  });

  it("adds a token it has never seen", () => {
    const store = createWorldStore({ worldId: "w" });
    store.dispatch(
      { type: "upsert_token", token: { id: "new", x: 1, y: 2, z: 0 } },
      "sync",
    );
    expect(store.getState().tokens.new).toEqual({
      id: "new",
      x: 1,
      y: 2,
      z: 0,
    });
  });
});

/**
 * Spec 085: several things selected together. The engine's box reports the
 * whole group; each kind's single selection stays its first member.
 */
describe("group selection", () => {
  const wall = (id: string) => ({
    id,
    sceneId: "s",
    x1: 0,
    y1: 0,
    x2: 10,
    y2: 0,
    blocksVision: true,
    blocksMovement: true,
    doorState: "none" as const,
  });

  it("select_group sets the four lists and each kind's primary", () => {
    const store = createWorldStore({ worldId: "w" });
    store.dispatch(
      {
        type: "select_group",
        tokenIds: ["t1", "t2"],
        wallIds: ["w1", "w2"],
        lightIds: [],
        shapeIds: ["s1"],
      },
      "bevy",
    );

    const state = store.getState();
    expect(state.selectedTokenIds).toEqual(["t1", "t2"]);
    expect(state.selectedTokenId).toBe("t1");
    expect(state.selectedWallIds).toEqual(["w1", "w2"]);
    expect(state.selectedWallId).toBe("w1");
    expect(state.selectedLightIds).toEqual([]);
    expect(state.selectedLightId).toBeNull();
    expect(state.selectedShapeIds).toEqual(["s1"]);
    expect(state.selectedShapeId).toBe("s1");
  });

  it("a single selection of a wall, light or shape is a list of one, or none", () => {
    const store = createWorldStore({ worldId: "w" });
    store.dispatch({ type: "select_wall", wallId: "w1" }, "bevy");
    store.dispatch({ type: "select_light", lightId: "l1" }, "bevy");
    store.dispatch({ type: "select_shape", shapeId: "s1" }, "bevy");
    expect(store.getState().selectedWallIds).toEqual(["w1"]);
    expect(store.getState().selectedLightIds).toEqual(["l1"]);
    expect(store.getState().selectedShapeIds).toEqual(["s1"]);

    store.dispatch({ type: "select_wall", wallId: null }, "bevy");
    store.dispatch({ type: "select_light", lightId: null }, "bevy");
    store.dispatch({ type: "select_shape", shapeId: null }, "bevy");
    expect(store.getState().selectedWallIds).toEqual([]);
    expect(store.getState().selectedLightIds).toEqual([]);
    expect(store.getState().selectedShapeIds).toEqual([]);
  });

  it("a removed wall, light or shape leaves its list", () => {
    const store = createWorldStore({ worldId: "w" });
    store.dispatch(
      {
        type: "select_group",
        tokenIds: [],
        wallIds: ["w1", "w2"],
        lightIds: ["l1", "l2"],
        shapeIds: ["s1", "s2"],
      },
      "bevy",
    );
    store.dispatch({ type: "remove_wall", wallId: "w1" }, "sync");
    store.dispatch({ type: "remove_light", lightId: "l2" }, "sync");
    store.dispatch({ type: "remove_shape", shapeId: "s1" }, "sync");

    expect(store.getState().selectedWallIds).toEqual(["w2"]);
    expect(store.getState().selectedLightIds).toEqual(["l1"]);
    expect(store.getState().selectedShapeIds).toEqual(["s2"]);
  });

  it("delete_group changes nothing in the store", () => {
    const store = createWorldStore({
      worldId: "w",
      initialWalls: [wall("w1")],
    });
    const before = store.getState();
    store.dispatch({ type: "delete_group" }, "ui");
    expect(store.getState()).toBe(before);
  });

  it("set_walls_hidden hides each named wall at once, and ignores one it lacks", () => {
    const store = createWorldStore({
      worldId: "w",
      initialWalls: [wall("w1"), wall("w2"), wall("w3")],
    });
    store.dispatch(
      { type: "set_walls_hidden", wallIds: ["w1", "w3", "gone"], hidden: true },
      "ui",
    );
    const { walls } = store.getState();
    expect(walls.w1.secret).toBe(true);
    expect(walls.w2.secret).toBeUndefined();
    expect(walls.w3.secret).toBe(true);
    expect(walls.gone).toBeUndefined();
  });
});
