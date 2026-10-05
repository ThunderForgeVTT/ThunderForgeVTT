import { describe, expect, it } from "vitest";
import type { WorldWall } from "@/engine/world/types";
import {
  actionLabel,
  doorMenuActions,
  isGmLocked,
  wallName,
} from "../canvasMenuActions";
import { isDoubleRightClick, type WallClick } from "../useCanvasContextMenu";

function wall(overrides: Partial<WorldWall> = {}): WorldWall {
  return {
    id: "w",
    sceneId: "s",
    x1: 0,
    y1: 0,
    x2: 100,
    y2: 0,
    blocksVision: true,
    blocksMovement: true,
    doorState: "closed",
    locked: false,
    secret: false,
    ...overrides,
  };
}

const GM = { isGameMaster: true, userId: "gm" };
const PLAYER = { isGameMaster: false, userId: "p" };

function kinds(
  viewer: typeof GM,
  target: WorldWall,
  canOpen = false,
): string[] {
  return doorMenuActions({ viewer, wall: target, canOpen }).map((action) =>
    actionLabel(action, ""),
  );
}

describe("a Game Master's right-click on a wall or a door", () => {
  it("offers to make a plain wall a door, and nothing else", () => {
    expect(kinds(GM, wall({ doorState: "none" }))).toEqual([
      "Make this a door",
    ]);
  });

  it("opens, locks, locks as a wall and undoes a shut door", () => {
    expect(kinds(GM, wall())).toEqual([
      "Open",
      "Lock",
      "Lock as a wall",
      "Make it an ordinary wall",
    ]);
  });

  it("closes an open door and unlocks a locked one", () => {
    expect(kinds(GM, wall({ doorState: "open", locked: true }))).toEqual([
      "Close",
      "Unlock",
      "Lock as a wall",
      "Make it an ordinary wall",
    ]);
  });

  it("offers to show a hidden door, and not to lock as a wall what already is one", () => {
    const gmLocked = wall({ locked: true, secret: true });
    expect(isGmLocked(gmLocked)).toBe(true);
    expect(kinds(GM, gmLocked)).toEqual([
      "Open",
      "Unlock",
      "Show it to the table",
      "Make it an ordinary wall",
    ]);
  });

  it("still offers to lock as a wall a hidden door that stands open", () => {
    const open = wall({ doorState: "open", locked: true, secret: true });
    expect(isGmLocked(open)).toBe(false);
    expect(kinds(GM, open)).toContain("Lock as a wall");
  });
});

describe("a player's right-click on a door", () => {
  it("opens or shuts one they may use", () => {
    expect(kinds(PLAYER, wall(), true)).toEqual(["Open"]);
    expect(kinds(PLAYER, wall({ doorState: "open" }), true)).toEqual(["Close"]);
  });

  it("is told a locked door is locked, and offered nothing", () => {
    const actions = doorMenuActions({
      viewer: PLAYER,
      wall: wall({ locked: true }),
      canOpen: true,
    });
    expect(actions).toEqual([{ kind: "door-locked" }]);
  });

  it("offers nothing on a door they may not use, a plain wall or a hidden door", () => {
    expect(kinds(PLAYER, wall(), false)).toEqual([]);
    expect(kinds(PLAYER, wall({ doorState: "none" }), true)).toEqual([]);
    expect(kinds(PLAYER, wall({ secret: true }), true)).toEqual([]);
    expect(kinds(PLAYER, wall({ secret: true, locked: true }), true)).toEqual(
      [],
    );
  });
});

describe("what the menu calls it", () => {
  it("names a wall, a door and a hidden door", () => {
    expect(wallName(wall({ doorState: "none" }))).toBe("Wall");
    expect(wallName(wall())).toBe("Door");
    expect(wallName(wall({ secret: true }))).toBe("Hidden door");
  });
});

describe("a double right-click", () => {
  const first: WallClick = { wallId: "w", at: { x: 100, y: 100 }, time: 1000 };

  it("is a second click on the same door, soon and near", () => {
    expect(
      isDoubleRightClick(first, {
        ...first,
        at: { x: 104, y: 97 },
        time: 1300,
      }),
    ).toBe(true);
  });

  it("is not one with nothing before it, too late, too far or on another door", () => {
    expect(isDoubleRightClick(null, first)).toBe(false);
    expect(isDoubleRightClick(first, { ...first, time: 1600 })).toBe(false);
    expect(
      isDoubleRightClick(first, {
        ...first,
        at: { x: 140, y: 100 },
        time: 1200,
      }),
    ).toBe(false);
    expect(
      isDoubleRightClick(first, { ...first, wallId: "other", time: 1200 }),
    ).toBe(false);
  });
});
