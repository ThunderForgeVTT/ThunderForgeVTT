import { useState } from "react";

import { rollDice } from "@/api/roll";
import { useRollVisibility } from "@/components/world/RollVisibility/useRollVisibility";
import type { RollResolutionRecord } from "@/types/roll";

/**
 * One character's roll state: who sees the next roll, which roll is in
 * flight, and the last answer. Shared by the roll buttons and a sheet's own
 * tappable numbers (spec 102), so both read from one picker and one result.
 */
export function useCharacterRoller(worldId: string, isGm: boolean) {
  const [visibility, setVisibility] = useRollVisibility(isGm);
  const [rolling, setRolling] = useState<string | null>(null);
  const [result, setResult] = useState<RollResolutionRecord | null>(null);
  /** What the last result was a roll of: "Acrobatics", "Longsword (damage)". */
  const [resultLabel, setResultLabel] = useState<string | null>(null);
  const [rollError, setRollError] = useState<string | null>(null);

  const roll = async (key: string, label: string, formula: string) => {
    setRolling(key);
    setRollError(null);
    // The last result stays until this one lands: clearing it first made the
    // phone's pinned top collapse and grow back on every tap.
    try {
      const resolution = await rollDice(worldId, formula, {
        label,
        visibility,
      });
      setResult(resolution);
      setResultLabel(label);
    } catch (error) {
      setResult(null);
      setRollError(
        error instanceof Error ? error.message : "Failed to roll dice",
      );
    } finally {
      setRolling(null);
    }
  };

  return {
    isGm,
    visibility,
    setVisibility,
    rolling,
    result,
    resultLabel,
    rollError,
    roll,
  };
}

export type CharacterRoller = ReturnType<typeof useCharacterRoller>;
