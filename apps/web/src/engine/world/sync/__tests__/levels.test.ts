import { beforeEach, describe, expect, it, vi } from "vitest";
import type { SceneLevel } from "@/api/levels";

/**
 * Which level of a scene a viewer is shown.
 *
 * The engine is level-unaware, so this decision is the whole of "levels" on
 * the web side, and it is made differently for a Game Master and a player.
 * These pin both, and the classification of the world events that mean the
 * question has to be asked again.
 */

const getSceneLevels = vi.fn();
const getTokens = vi.fn();

vi.mock("@/api/levels", () => ({
  getSceneLevels: (...args: unknown[]) => getSceneLevels(...args),
}));
vi.mock("@/api/tokens", () => ({
  getTokens: (...args: unknown[]) => getTokens(...args),
}));

const {
  boardKey,
  entryLevel,
  levelEventKind,
  pickedLevel,
  playerSeesLevelName,
  resolveLevelView,
} = await import("../levels");

const SCENE = "scene-1";

function level(over: Partial<SceneLevel> & { levelId: string }): SceneLevel {
  return {
    sceneId: SCENE,
    name: over.levelId,
    sortOrder: 0,
    isEntry: false,
    hidden: false,
    backgroundAssetId: null,
    backgroundUrl: null,
    width: 1000,
    height: 1000,
    ambientLight: "bright",
    tokenCount: null,
    ...over,
  };
}

const TAVERN = level({ levelId: "tavern", isEntry: true });
const UPSTAIRS = level({ levelId: "upstairs", sortOrder: 1 });
const STREET = level({ levelId: "street", sortOrder: 2 });

beforeEach(() => {
  getSceneLevels.mockReset();
  getTokens.mockReset();
});

describe("levelEventKind", () => {
  it("names a level change, a travel and a token change", () => {
    expect(levelEventKind({ event_code: 33 }, SCENE)).toBe("levels");
    expect(levelEventKind({ event_code: 34 }, SCENE)).toBe("travel");
    expect(levelEventKind({ eventCode: 14 }, SCENE)).toBe("token");
  });

  it("says nothing about any other event", () => {
    expect(levelEventKind({ event_code: 11 }, SCENE)).toBeNull();
    expect(levelEventKind({}, SCENE)).toBeNull();
  });

  it("ignores an event that names another scene", () => {
    expect(
      levelEventKind(
        { event_code: 34, token_event: { scene_id: "elsewhere" } },
        SCENE,
      ),
    ).toBeNull();
    expect(
      levelEventKind(
        { event_code: 34, token_event: { scene_id: SCENE } },
        SCENE,
      ),
    ).toBe("travel");
  });

  it("does not ignore an event that names no scene", () => {
    // Missing a change is worse than one spare read.
    expect(levelEventKind({ event_code: 33, token_event: {} }, SCENE)).toBe(
      "levels",
    );
  });
});

describe("entryLevel and pickedLevel", () => {
  it("opens on the entry level, else the lowest", () => {
    expect(entryLevel([UPSTAIRS, TAVERN])).toBe(TAVERN);
    expect(entryLevel([UPSTAIRS, STREET])).toBe(UPSTAIRS);
    expect(entryLevel([])).toBeNull();
  });

  it("keeps a Game Master's pick only while the level exists", () => {
    expect(pickedLevel([TAVERN, UPSTAIRS], "upstairs")).toBe("upstairs");
    expect(pickedLevel([TAVERN, UPSTAIRS], "deleted")).toBe("tavern");
    expect(pickedLevel([TAVERN, UPSTAIRS], null)).toBe("tavern");
    expect(pickedLevel([], "upstairs")).toBeNull();
  });
});

describe("resolveLevelView", () => {
  it("shows a Game Master the level they picked, without reading tokens", async () => {
    getSceneLevels.mockResolvedValue([TAVERN, UPSTAIRS, STREET]);

    const view = await resolveLevelView(
      SCENE,
      { userId: "gm", isGm: true },
      "street",
    );

    expect(view.levelId).toBe("street");
    expect(view.levels).toHaveLength(3);
    expect(getTokens).not.toHaveBeenCalled();
  });

  it("shows a player the one level the server answers them for", async () => {
    // A player upstairs is answered with Upstairs alone — that *is* where
    // their token stands, and there is nothing further to ask.
    getSceneLevels.mockResolvedValue([UPSTAIRS]);

    const view = await resolveLevelView(
      SCENE,
      { userId: "player", isGm: false },
      // A player's pick is ignored: they cannot choose.
      "tavern",
    );

    expect(view.levelId).toBe("upstairs");
    expect(getTokens).not.toHaveBeenCalled();
  });

  it("shows a player with tokens on two floors their primary token's", async () => {
    getSceneLevels.mockResolvedValue([TAVERN, UPSTAIRS]);
    getTokens.mockImplementation((_scene: string, levelId: string) =>
      Promise.resolve(
        levelId === "tavern"
          ? [{ ownerUserId: "player", isPrimary: false }]
          : [{ ownerUserId: "player", isPrimary: true }],
      ),
    );

    const view = await resolveLevelView(
      SCENE,
      { userId: "player", isGm: false },
      null,
    );

    expect(view.levelId).toBe("upstairs");
    expect(getTokens).toHaveBeenCalledWith(SCENE, "tavern");
    expect(getTokens).toHaveBeenCalledWith(SCENE, "upstairs");
  });

  it("falls back to the lowest floor a player owns a token on, then the entry level", async () => {
    getSceneLevels.mockResolvedValue([TAVERN, UPSTAIRS]);
    getTokens.mockImplementation((_scene: string, levelId: string) =>
      Promise.resolve(
        levelId === "upstairs"
          ? [{ ownerUserId: "player", isPrimary: false }]
          : [{ ownerUserId: "someone-else", isPrimary: true }],
      ),
    );
    expect(
      (await resolveLevelView(SCENE, { userId: "player", isGm: false }, null))
        .levelId,
    ).toBe("upstairs");

    getTokens.mockResolvedValue([]);
    expect(
      (await resolveLevelView(SCENE, { userId: "player", isGm: false }, null))
        .levelId,
    ).toBe("tavern");
  });

  it("answers no level for a scene that answered none", async () => {
    getSceneLevels.mockResolvedValue([]);
    const view = await resolveLevelView(
      SCENE,
      { userId: "player", isGm: false },
      null,
    );
    expect(view).toEqual({ levels: [], levelId: null });
  });
});

describe("playerSeesLevelName", () => {
  it("shows nothing on a scene with one level", () => {
    // A scene made before levels existed has to look exactly as it did.
    expect(playerSeesLevelName([TAVERN])).toBe(false);
    expect(playerSeesLevelName([])).toBe(false);
  });

  it("names the level for a player who is not where the scene opens", () => {
    expect(playerSeesLevelName([UPSTAIRS])).toBe(true);
  });

  it("names the level for a player answered for more than one", () => {
    expect(playerSeesLevelName([TAVERN, UPSTAIRS])).toBe(true);
  });
});

describe("boardKey", () => {
  it("is the scene and the level together, or the scene alone", () => {
    expect(boardKey(SCENE, "upstairs")).toBe("scene-1:upstairs");
    expect(boardKey(SCENE, null)).toBe(SCENE);
  });
});
