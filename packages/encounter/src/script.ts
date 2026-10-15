/**
 * Spec 091: the shape of a room's fixed facts.
 *
 * The landing writes the one real script (`apps/landing/src/room/script.ts`,
 * T031). The package only knows its shape, so the tests can use their own.
 * Positions are in squares unless a field says otherwise.
 */

import type { GoalId } from "./goals.ts";
import type { RoomMap } from "./map.ts";
import type { Point } from "./walls.ts";

/** A rectangle in squares, from its low corner. */
export interface SquareRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** A named part of the map, for the words. */
export interface Area {
  /** As the words say it: "a dark room", "the hall". */
  phrase: string;
  bounds: SquareRect;
}

export interface RoomScript {
  map: RoomMap;
  /** The centre of the hero's starting square. */
  start: Point;
  /**
   * The sign of y that the room calls north: -1 when north is toward
   * negative y. East is always positive x, as the engine's `Step::East`.
   */
  north: 1 | -1;
  /** The door whose goals count. */
  goalDoor: string;
  /** Each door's name in the words, by wall id: "the door to the hall". */
  doorNames: Record<string, string>;
  areas: Area[];
  /** The chest the room ends on. A room without one ends at its `exit`. */
  chest?: {
    tokenId: string;
    /** The centre of the chest's square. */
    square: Point;
    /** The engine region whose entry finds the chest. */
    interactiveId: string;
    region: SquareRect;
    /** The chest shut, and the chest once the roll opens it. */
    art: string;
    openedArt: string;
  };
  /**
   * Where the hero leaves a walk-through room: a step from `from` into
   * `region` meets `room-left`.
   */
  exit?: { from: SquareRect; region: SquareRect };
  roll: { formula: string; dc: number };
  scene: {
    background: string;
    /** A carried torch, in squares. */
    torch: { bright: number; dim: number };
    /** What the camera fits, in squares. */
    camera: SquareRect;
  };
}

/** Whether a point lies in a rectangle, edges included, as `RegionGeometry::contains` (canvas-core `interaction.rs`). */
export function inRect(point: Point, rect: SquareRect): boolean {
  return (
    point.x >= rect.x &&
    point.x <= rect.x + rect.width &&
    point.y >= rect.y &&
    point.y <= rect.y + rect.height
  );
}

/** The goals a room plays, in order: the goal door's, then its ending's. */
export function goalsOf(script: RoomScript): GoalId[] {
  return [
    "door-reached",
    "door-opened",
    ...(script.chest ? (["chest-found", "chest-opened"] as const) : []),
    ...(script.exit ? (["room-left"] as const) : []),
  ];
}
