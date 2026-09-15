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
