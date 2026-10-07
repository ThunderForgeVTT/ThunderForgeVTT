import { describe, expect, it } from "vitest";
import type { WorldWall } from "@/engine/world/types";
import { doorIconAction, doorMenuActions } from "../canvasMenuActions";

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

describe("doorIconAction (spec 071 FR-009)", () => {
  it("opens a shut door and shuts an open one, for either role", () => {
    for (const viewer of [GM, PLAYER]) {
      expect(doorIconAction({ viewer, wall: wall() })).toEqual({
        kind: "door-state",
        open: true,
      });
      expect(
        doorIconAction({ viewer, wall: wall({ doorState: "open" }) }),
      ).toEqual({ kind: "door-state", open: false });
    }
  });

  it("unlocks for the Game Master and changes nothing for a player", () => {
    const locked = wall({ locked: true });
    expect(doorIconAction({ viewer: GM, wall: locked })).toEqual({
      kind: "door-lock",
      locked: false,
    });
    expect(doorIconAction({ viewer: PLAYER, wall: locked })).toEqual({
      kind: "door-locked",
    });
  });

  it("is nothing on a secret door or a plain wall", () => {
    for (const viewer of [GM, PLAYER]) {
      expect(
        doorIconAction({ viewer, wall: wall({ secret: true }) }),
      ).toBeNull();
      expect(
        doorIconAction({ viewer, wall: wall({ doorState: "none" }) }),
      ).toBeNull();
    }
  });

  it("is always an item the right-click menu offers the same viewer", () => {
    const doors = [
      wall(),
      wall({ doorState: "open" }),
      wall({ locked: true }),
      wall({ doorState: "open", locked: true }),
    ];
    for (const viewer of [GM, PLAYER]) {
      for (const door of doors) {
        const action = doorIconAction({ viewer, wall: door });
        const menu = doorMenuActions({ viewer, wall: door, canOpen: true });
        expect(menu).toContainEqual(action);
      }
    }
  });
});
