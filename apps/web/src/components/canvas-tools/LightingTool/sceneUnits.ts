import type { SceneUnits } from "@/types/light";

/** D&D 5e's five-foot square: what a scene measures in until it says. */
export const DEFAULT_SCENE_UNITS: SceneUnits = {
  perCell: 5,
  label: "ft",
  gridSize: 50,
};

/** World units to the system's units, rounded for a person to read. */
export function toSystemUnits(world: number, units: SceneUnits): number {
  const perWorld = units.perCell / Math.max(units.gridSize, 1);
  return Math.round(world * perWorld * 10) / 10;
}

/** The system's units to world units, as the board draws them. */
export function toWorldUnits(distance: number, units: SceneUnits): number {
  return (distance / Math.max(units.perCell, Number.EPSILON)) * units.gridSize;
}
