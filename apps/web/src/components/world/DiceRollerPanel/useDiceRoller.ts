import { useState } from "react";
import { rollDice } from "@/api/roll";
import { useRollVisibility } from "@/components/world/RollVisibility/useRollVisibility";
import {
  engineDiceTimings,
  prefersReducedMotion,
} from "@/engine/bevy/diceThrow";
import type { RollResolutionRecord } from "@/types/roll";

import { revealDelayMs } from "./revealDelay";

/**
 * The dice roller's state, held by the board rather than the panel (owner's
 * T017 try-out): on a phone the panel lives in a sheet that a second tap
 * closes, and closing it lost the formula and the last roll. Held here, the
 * panel can come and go and a roll still in flight still lands.
 */
export function useDiceRoller(
  worldId: string,
  isGameMaster: boolean,
  engineReady: boolean,
) {
  const [formula, setFormula] = useState("1d20");
  const [isRolling, setIsRolling] = useState(false);
  const [result, setResult] = useState<RollResolutionRecord | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Spec 081: who sees the roll, remembered per browser.
  const [visibility, setVisibility] = useRollVisibility(isGameMaster);

  const roll = async (rolled: string = formula) => {
    setFormula(rolled);
    setIsRolling(true);
    setError(null);
    setResult(null);
    try {
      const resolution = await rollDice(worldId, rolled, { visibility });

      // Spec 081 (FR-013): the board animates this roll from its world
      // event, like every other client's roll; the panel only waits for the
      // dice to settle before showing the total.
      const delay = revealDelayMs({
        engineReady,
        timings: engineDiceTimings(),
        reducedMotion: prefersReducedMotion(),
      });
      if (delay > 0) {
        await new Promise((resolve) => setTimeout(resolve, delay));
      }

      setResult(resolution);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to roll dice");
    } finally {
      setIsRolling(false);
    }
  };

  return {
    formula,
    setFormula,
    isRolling,
    result,
    error,
    visibility,
    setVisibility,
    roll,
  };
}

export type DiceRoller = ReturnType<typeof useDiceRoller>;
