// Spec 083 (FR-017): how long the roll panel waits before it shows a total,
// so the total appears as the dice on the board land. The engine owns the
// timings (`dice_timings()`); this only picks one.

import type { DiceTimings } from "@/engine/bevy/diceThrow";

/** Used when the board has an engine but its module has not answered. */
export const FALLBACK_REVEAL_MS = 1200;

export function revealDelayMs({
  engineReady,
  timings,
  reducedMotion,
}: {
  engineReady: boolean;
  timings: DiceTimings | null;
  reducedMotion: boolean;
}): number {
  // A board with no engine never plays a throw, so nothing is waited on
  // (spec 014 FR-016).
  if (!engineReady) return 0;
  if (!timings) return FALLBACK_REVEAL_MS;
  return reducedMotion ? timings.reducedMs : timings.tumbleMs;
}
