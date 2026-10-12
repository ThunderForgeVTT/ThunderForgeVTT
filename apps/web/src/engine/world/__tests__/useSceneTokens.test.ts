import { describe, expect, it } from "vitest";

import { createWorldStore } from "../store";
import { sameSceneTokens, sceneTokensOf } from "../useSceneTokens";

/**
 * What a panel reads of the scene's tokens from the world store. The hook
 * itself is React's; these are the two decisions it makes: which tokens are
 * the scene's, and whether a change is one a panel can see.
 */
describe("sceneTokensOf", () => {
  it("reads a token's character, link and level as the server sent them", () => {
    const store = createWorldStore({ worldId: "world-1" });
    store.dispatch(
      {
        type: "upsert_token",
        token: {
          id: "token-1",
          x: 10,
          y: 20,
          z: 0,
          label: "Wolf",
          sceneId: "scene-1",
          levelId: "level-1",
          actorId: "actor-1",
          linked: true,
          ownerUserId: "user-1",
          isPrimary: true,
        },
      },
      "sync",
    );

    expect(sceneTokensOf(store.getState().tokens, "scene-1")).toEqual([
      {
        tokenId: "token-1",
        sceneId: "scene-1",
        levelId: "level-1",
        actorId: "actor-1",
        ownerUserId: "user-1",
        isPrimary: true,
        photoUrl: null,
        tokenType: "character",
        linked: true,
        name: "Wolf",
        nameVisibleToPlayers: true,
      },
    ]);
  });

  it("leaves out another scene's token and keeps one not yet read back", () => {
    const store = createWorldStore({
      worldId: "world-1",
      initialTokens: [
        { id: "elsewhere", x: 0, y: 0, z: 0, sceneId: "scene-2" },
        // Made by the engine a moment ago: no scene until the server answers.
        { id: "fresh", x: 0, y: 0, z: 0 },
      ],
    });

    const tokens = sceneTokensOf(store.getState().tokens, "scene-1");
    expect(tokens.map((token) => [token.tokenId, token.sceneId])).toEqual([
      ["fresh", "scene-1"],
    ]);
  });

  it("keeps a token's character when the engine moves it", () => {
    const store = createWorldStore({
      worldId: "world-1",
      initialTokens: [
        { id: "token-1", x: 0, y: 0, z: 0, actorId: "actor-1", sceneId: "s" },
      ],
    });
    // The engine's own move carries the transform and nothing else.
    store.dispatch(
      { type: "upsert_token", token: { id: "token-1", x: 5, y: 5, z: 0 } },
      "bevy",
    );

    expect(sceneTokensOf(store.getState().tokens, "s")[0].actorId).toBe(
      "actor-1",
    );
  });
});

describe("sameSceneTokens", () => {
  it("does not see a move, and does see a change of character", () => {
    const before = sceneTokensOf(
      { t: { id: "t", x: 0, y: 0, z: 0, actorId: "a", sceneId: "s" } },
      "s",
    );
    const moved = sceneTokensOf(
      { t: { id: "t", x: 9, y: 9, z: 0, actorId: "a", sceneId: "s" } },
      "s",
    );
    const recast = sceneTokensOf(
      { t: { id: "t", x: 0, y: 0, z: 0, actorId: "b", sceneId: "s" } },
      "s",
    );

    expect(sameSceneTokens(before, moved)).toBe(true);
    expect(sameSceneTokens(before, recast)).toBe(false);
  });
});
