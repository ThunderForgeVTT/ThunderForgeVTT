/**
 * Spec 091: the referee. The page plays the server's part with it.
 *
 * It judges every move with the same wall rule the server uses, opens and
 * closes doors in reach, finds the chest and judges the roll. It is pure:
 * each action takes a state and returns a new state and an outcome. It
 * never touches the state it was given, the DOM or the engine, and it
 * rolls nothing itself.
 *
 * Positions in the state are in squares. Positions from the engine are in
 * world units.
 */

import type { GoalId } from "./goals.ts";
import { toSquareCentre, toWorld } from "./map.ts";
import { inRect, type RoomScript } from "./script.ts";
import {
  distanceToSegment,
  doorInReach,
  movementBlockedBy,
  pathBlockedBy,
  type DoorState,
  type Point,
  type Wall,
} from "./walls.ts";

export type Direction = "north" | "south" | "east" | "west";

/** One try at the chest. */
export interface RollTry {
  total: number;
  dc: number;
  success: boolean;
}

export interface RoomState {
  /** The centre of the last square the referee accepted. */
  hero: Point;
  /** Each door's state, by wall id. */
  doors: Record<string, DoorState>;
  /** The goals met, in the order they were met. */
  goals: GoalId[];
  rolls: RollTry[];
}

/** What the page says after an action. One key per outcome. */
export const ANNOUNCEMENTS = [
  "moved",
  "blocked-by-wall",
  "blocked-by-door",
  "rolled-back",
  "door-opened",
  "door-closed",
  "no-door-in-reach",
  "chest-found",
  "roll-too-soon",
  "roll-failed",
  "roll-succeeded",
  "chest-already-open",
  "ignored",
  "reset",
] as const;

export type Announcement = (typeof ANNOUNCEMENTS)[number];

export interface Outcome {
  accepted: boolean;
  announcement: Announcement;
  /** The goal this action met, if it met one for the first time. */
  goal: GoalId | null;
  /** The wall that refused a move. */
  wallId?: string;
  /** The door this action opened or closed. */
  door?: { id: string; state: DoorState };
  /** For a refused engine move: the square to send the hero back to. */
  rollback?: Point;
  /** Why a refused action was refused, in words. */
  reason?: string;
}

export interface Judged {
  state: RoomState;
  outcome: Outcome;
}

/** The part of `thunderforge_dice::RollResolution` the referee reads. */
export interface RollResult {
  formula: string;
  kind: { Total: number } | { SuccessCount: number };
}

export interface Referee {
  start(): RoomState;
  step(state: RoomState, dir: Direction): Judged;
  judgeMove(
    state: RoomState,
    from: Point,
    to: Point,
    path?: readonly Point[],
  ): Judged;
  /** The engine refused a move at `wallId` before the room asked. */
  refused(state: RoomState, wallId: string): Judged;
  toggleDoor(state: RoomState): Judged;
  doorPressed(state: RoomState, wallId: string): Judged;
  entered(state: RoomState, interactiveId: string): Judged;
  rolled(state: RoomState, result: RollResult): Judged;
  reset(): Judged;
}

const NO_DOOR = "No door is within reach. Step next to one first.";

export function createReferee(script: RoomScript): Referee {
  const grid = script.map.gridSize;
  const doorIds = script.map.walls
    .filter((w) => w.doorState !== "none")
    .map((w) => w.id);

  const start = (): RoomState => ({
    hero: { ...script.start },
    doors: Object.fromEntries(
      script.map.walls
        .filter((w) => w.doorState !== "none")
        .map((w) => [w.id, w.doorState]),
    ),
    goals: [],
    rolls: [],
  });

  /** The map's walls with the doors as the state has them. */
  const walls = (state: RoomState): Wall[] =>
    script.map.walls.map((w) =>
      w.id in state.doors ? { ...w, doorState: state.doors[w.id]! } : w,
    );

  const meet = (state: RoomState, goal: GoalId): [RoomState, GoalId | null] =>
    state.goals.includes(goal)
      ? [state, null]
      : [{ ...state, goals: [...state.goals, goal] }, goal];

  /**
   * A move refused at a wall. The goal door, while shut, marks door-reached.
   * A move the engine already made is sent back to the last accepted square.
   */
  const refusedAt = (
    state: RoomState,
    wall: Wall,
    engineMoved: boolean,
  ): Judged => {
    const isDoor = doorIds.includes(wall.id);
    const [next, goal] =
      wall.id === script.goalDoor && state.doors[wall.id] === "closed"
        ? meet(state, "door-reached")
        : [state, null];
    const outcome: Outcome = {
      accepted: false,
      announcement: engineMoved
        ? "rolled-back"
        : isDoor
          ? "blocked-by-door"
          : "blocked-by-wall",
      goal,
      wallId: wall.id,
    };
    if (engineMoved) outcome.rollback = { ...state.hero };
    return { state: next, outcome };
  };

  /** A move accepted to `to`. Entering the chest's region finds it. */
  const movedTo = (state: RoomState, to: Point): Judged => {
    const moved = { ...state, hero: { ...to } };
    if (
      !inRect(state.hero, script.chest.region) &&
      inRect(to, script.chest.region)
    ) {
      const [next, goal] = meet(moved, "chest-found");
      if (goal)
        return {
          state: next,
          outcome: { accepted: true, announcement: "chest-found", goal },
        };
    }
    return {
      state: moved,
      outcome: { accepted: true, announcement: "moved", goal: null },
    };
  };

  /** The way a direction goes, in squares. */
  const offset = (dir: Direction): Point => {
    switch (dir) {
      case "north":
        return { x: 0, y: script.north };
      case "south":
        return { x: 0, y: -script.north };
      case "east":
        return { x: 1, y: 0 };
      case "west":
        return { x: -1, y: 0 };
    }
  };

  const toggle = (state: RoomState, door: Wall): Judged => {
    const now: DoorState = state.doors[door.id] === "open" ? "closed" : "open";
    const changed = { ...state, doors: { ...state.doors, [door.id]: now } };
    const [next, goal] =
      now === "open" && door.id === script.goalDoor
        ? meet(changed, "door-opened")
        : [changed, null];
    return {
      state: next,
      outcome: {
        accepted: true,
        announcement: now === "open" ? "door-opened" : "door-closed",
        goal,
        door: { id: door.id, state: now },
      },
    };
  };

  const heroToken = (state: RoomState) => ({
    center: toWorld(state.hero, grid),
    side: grid,
  });

  const refuse = (
    state: RoomState,
    announcement: Announcement,
    reason?: string,
  ): Judged => ({
    state,
    outcome: reason
      ? { accepted: false, announcement, goal: null, reason }
      : { accepted: false, announcement, goal: null },
  });

  return {
    start,

    step(state, dir) {
      const by = offset(dir);
      const to = { x: state.hero.x + by.x, y: state.hero.y + by.y };
      const blocked = movementBlockedBy(
        toWorld(state.hero, grid),
        toWorld(to, grid),
        walls(state),
      );
      return blocked ? refusedAt(state, blocked, false) : movedTo(state, to);
    },

    judgeMove(state, from, to, path) {
      const route = path && path.length >= 2 ? path : [from, to];
      const blocked = pathBlockedBy(route, walls(state));
      return blocked
        ? refusedAt(state, blocked, true)
        : movedTo(state, toSquareCentre(to, grid));
    },

    refused(state, wallId) {
      const wall = walls(state).find((w) => w.id === wallId);
      return wall ? refusedAt(state, wall, false) : refuse(state, "ignored");
    },

    toggleDoor(state) {
      const hero = heroToken(state);
      const inReach = walls(state)
        .filter((w) => doorIds.includes(w.id) && doorInReach(w, hero, grid))
        .map((w) => ({
          w,
          d: distanceToSegment(
            hero.center,
            { x: w.x1, y: w.y1 },
            { x: w.x2, y: w.y2 },
          ),
        }))
        .sort((a, b) => a.d - b.d);
      const nearest = inReach[0];
      return nearest
        ? toggle(state, nearest.w)
        : refuse(state, "no-door-in-reach", NO_DOOR);
    },

    doorPressed(state, wallId) {
      const door = walls(state).find(
        (w) => w.id === wallId && doorIds.includes(w.id),
      );
      if (!door) return refuse(state, "ignored");
      if (!doorInReach(door, heroToken(state), grid))
        return refuse(state, "no-door-in-reach", NO_DOOR);
      return toggle(state, door);
    },

    entered(state, interactiveId) {
      if (interactiveId !== script.chest.interactiveId)
        return refuse(state, "ignored");
      const [next, goal] = meet(state, "chest-found");
      return {
        state: next,
        outcome: { accepted: true, announcement: "chest-found", goal },
      };
    },

    rolled(state, result) {
      if (!("Total" in result.kind)) return refuse(state, "ignored");
      if (!state.goals.includes("chest-found")) {
        return refuse(
          state,
          "roll-too-soon",
          "Find the chest before you try to open it.",
        );
      }
      if (state.goals.includes("chest-opened"))
        return refuse(state, "chest-already-open");
      const { dc } = script.roll;
      const total = result.kind.Total;
      const success = total >= dc;
      const tried = {
        ...state,
        rolls: [...state.rolls, { total, dc, success }],
      };
      if (!success) {
        return {
          state: tried,
          outcome: { accepted: true, announcement: "roll-failed", goal: null },
        };
      }
      const [next, goal] = meet(tried, "chest-opened");
      return {
        state: next,
        outcome: { accepted: true, announcement: "roll-succeeded", goal },
      };
    },

    reset() {
      return {
        state: start(),
        outcome: { accepted: true, announcement: "reset", goal: null },
      };
    },
  };
}
