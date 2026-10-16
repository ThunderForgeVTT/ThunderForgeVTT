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
  const [rollError, setRollError] = useState<string | null>(null);

  const roll = async (key: string, label: string, formula: string) => {
    setRolling(key);
    setRollError(null);
    setResult(null);
    try {
      setResult(await rollDice(worldId, formula, { label, visibility }));
    } catch (error) {
      setRollError(
        error instanceof Error ? error.message : "Failed to roll dice",
      );
    } finally {
      setRolling(null);
    }
  };

  return { isGm, visibility, setVisibility, rolling, result, rollError, roll };
}

export type CharacterRoller = ReturnType<typeof useCharacterRoller>;
