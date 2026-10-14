/**
 * Spec 091: the Proving Ground map, read into what the referee and the
 * engine use.
 *
 * `demo.json` walls carry no ids. Each gets one from its index, so the
 * referee, the engine and the e2e name the same wall the same way. The two
 * doors get names instead.
 */

import type { DoorState, Point, Wall } from "./walls.ts";

/** A wall as `demo.json` writes it. */
export interface MapFileWall {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  blocksMovement: boolean;
  blocksVision: boolean;
  doorState: DoorState;
  perimeter?: boolean;
}

/** A light as `demo.json` writes it. */
export interface MapFileLight {
  x: number;
  y: number;
  radius: number;
  brightRadius: number;
  color: string;
  intensity: number;
  castsShadows: boolean;
}

/** `demo.json`'s shape. Coordinates are world units, centred on the map. */
export interface MapFile {
  name: string;
  width: number;
  height: number;
  gridSize: number;
  walls: MapFileWall[];
  lights: MapFileLight[];
}

/** A light, in the shape of the engine's `upsert_light` payload. */
export interface Light extends MapFileLight {
  id: string;
}

export interface RoomMap {
  name: string;
  width: number;
  height: number;
  gridSize: number;
  walls: Wall[];
  lights: Light[];
}

/** The doors' names, in the order the map lists its doors. */
export const DOOR_IDS = ["door-hall", "door-west"] as const;

export type DoorId = (typeof DOOR_IDS)[number];

const numbered = (prefix: string, index: number): string =>
  `${prefix}-${String(index).padStart(2, "0")}`;

/** Read a `demo.json`-shaped map. The file is not changed. */
export function readMap(file: MapFile): RoomMap {
  let doors = 0;
  const walls = file.walls.map((wall, index): Wall => {
    const name = wall.doorState === "none" ? undefined : DOOR_IDS[doors++];
    return {
      id: name ?? numbered("wall", index),
      x1: wall.x1,
      y1: wall.y1,
      x2: wall.x2,
      y2: wall.y2,
      blocksVision: wall.blocksVision,
      blocksMovement: wall.blocksMovement,
      doorState: wall.doorState,
    };
  });
  const lights = file.lights.map(
    (light, index): Light => ({
      id: numbered("light", index),
      x: light.x,
      y: light.y,
      radius: light.radius,
      brightRadius: light.brightRadius,
      intensity: light.intensity,
      color: light.color,
      castsShadows: light.castsShadows,
    }),
  );
  return {
    name: file.name,
    width: file.width,
    height: file.height,
    gridSize: file.gridSize,
    walls,
    lights,
  };
}

/** Squares to world units. */
export function toWorld(square: Point, gridSize: number): Point {
  return { x: square.x * gridSize, y: square.y * gridSize };
}

/** World units to squares. */
export function toSquares(world: Point, gridSize: number): Point {
  return { x: world.x / gridSize, y: world.y / gridSize };
}

/**
 * The square a token the engine moved sits on: its snapped position, which
 * carries float noise (`-3, 0.5000004`), to the half-square the room keeps.
 */
export function toSquareCentre(world: Point, gridSize: number): Point {
  const { x, y } = toSquares(world, gridSize);
  return { x: Math.round(x * 2) / 2, y: Math.round(y * 2) / 2 };
}
