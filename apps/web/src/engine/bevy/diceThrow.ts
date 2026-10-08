// Spec 083: what the web tells the engine about a roll, and what it reads
// back (contracts/engine-dice.md). Kept apart from `index.ts` so the roll
// panel can read the timings without loading the engine module.

import type { WorldRollRecord } from "@/types/roll";

/** `trigger_dice_roll`'s `roll`, in `WorldRoll`'s own field names. */
export interface DiceThrowPayload {
  id: string;
  rollerName: string;
  label: string | null;
  formula: string;
  bindings: { placeholder: string; value: number }[];
  resultKind: "TOTAL" | "SUCCESS_COUNT";
  resultValue: number;
  dice: {
    sidesKind: string;
    numericSides: number | null;
    rolls: number[];
    steps: string[];
    kept: boolean;
    finalValue: number;
  }[];
}

/**
 * The payload for one roll, passed through with no renaming. `steps` and
 * `bindings` are `[]` for a roll stored before spec 083.
 */
export function buildDiceThrow(roll: WorldRollRecord): DiceThrowPayload {
  return {
    id: roll.id,
    rollerName: roll.rollerName,
    label: roll.label,
    formula: roll.resolution.formula || roll.formula,
    bindings: (roll.bindings ?? []).map(({ placeholder, value }) => ({
      placeholder,
      value,
    })),
    resultKind: roll.resolution.resultKind,
    resultValue: roll.resolution.resultValue,
    dice: roll.resolution.dice.map((die) => ({
      sidesKind: die.sidesKind,
      numericSides: die.numericSides,
      rolls: die.rolls.slice(),
      steps: (die.steps ?? []).slice(),
      kept: die.kept,
      finalValue: die.finalValue,
    })),
  };
}

/** How long each part of a throw takes, in ms, as the engine owns them. */
export interface DiceTimings {
  tumbleMs: number;
  stepMs: number;
  holdMs: number;
  fadeMs: number;
  reducedMs: number;
}

let readTimings: (() => string) | null = null;

/** Called by `index.ts` once the engine module is loaded. */
export function setDiceTimingsSource(read: (() => string) | null): void {
  readTimings = read;
}

/** The engine's timings, or `null` when no engine module is loaded. */
export function engineDiceTimings(): DiceTimings | null {
  if (!readTimings) return null;
  try {
    return JSON.parse(readTimings()) as DiceTimings;
  } catch {
    return null;
  }
}

const REDUCED_MOTION = "(prefers-reduced-motion: reduce)";

function reducedMotionQuery(): MediaQueryList | null {
  return typeof globalThis.matchMedia === "function"
    ? globalThis.matchMedia(REDUCED_MOTION)
    : null;
}

/** Whether the viewer prefers reduced motion right now. */
export function prefersReducedMotion(): boolean {
  return reducedMotionQuery()?.matches ?? false;
}

/**
 * Sends the viewer's reduced-motion preference once, then again on every
 * change. Returns a function that stops it.
 */
export function watchReducedMotion(send: (on: boolean) => void): () => void {
  const query = reducedMotionQuery();
  if (!query) return () => {};
  send(query.matches);
  const onChange = (event: MediaQueryListEvent) => send(event.matches);
  query.addEventListener("change", onChange);
  return () => query.removeEventListener("change", onChange);
}
