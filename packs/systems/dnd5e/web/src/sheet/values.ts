/**
 * The 5e sheet's shared shapes, limits and readers: what the sections in this
 * folder and `ActorSheet.tsx` all reach for.
 */

export type Json = Record<string, unknown>;
export type Slot =
  | "ability_data"
  | "resource_data"
  | "proficiency_data"
  | "trait_data"
  | "spell_data";

export const SPELL_LEVELS = [1, 2, 3, 4, 5, 6, 7, 8, 9] as const;

/**
 * The game's ceiling for an ability score, and the validator's. A character's
 * own progression stops at 20, but a belt can carry one past it and a giant
 * starts there, so the sheet does not refuse what the server accepts.
 */
export const MAX_SCORE = 30;
/** Likewise for a spell save DC: 8 is the formula's floor. */
export const MAX_SAVE_DC = 30;

/** The speeds and senses beside the walk speed and darkvision, in feet. */
export const OTHER_SPEEDS = [
  { key: "speed_fly", label: "Fly" },
  { key: "speed_swim", label: "Swim" },
  { key: "speed_climb", label: "Climb" },
  { key: "speed_burrow", label: "Burrow" },
] as const;
export const OTHER_SENSES = [
  { key: "blindsight", label: "Blindsight" },
  { key: "tremorsense", label: "Tremorsense" },
  { key: "truesight", label: "Truesight" },
] as const;

export function num(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

export function str(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/**
 * A proficiency set as the server's rules read it: a list of ids. The
 * validator also accepts the older map of booleans, so a sheet that finds one
 * reads the true keys and writes a list back.
 */
export function idList(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((item): item is string => typeof item === "string");
  }
  if (value && typeof value === "object") {
    return Object.entries(value as Record<string, unknown>)
      .filter(([, on]) => on === true)
      .map(([id]) => id);
  }
  return [];
}

export function stringList(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : [];
}

export function signed(n: number): string {
  return n >= 0 ? `+${n}` : `${n}`;
}

export function titleCase(id: string): string {
  return id.charAt(0).toUpperCase() + id.slice(1);
}

export const toggleId = (set: string[], id: string) =>
  set.includes(id) ? set.filter((x) => x !== id) : [...set, id];
