import { useCallback, useState } from "react";

import { useResetOnChange } from "@/hooks/useResetOnChange";
import type { RollVisibility } from "@/types/roll";

import { readChoice, writeChoice } from "./storedChoice";

/** The picker's value, read from and saved to this browser's choice. */
export function useRollVisibility(
  isGm: boolean,
): [RollVisibility, (choice: RollVisibility) => void] {
  const [choice, setChoice] = useState<RollVisibility>(() => readChoice(isGm));
  // The role can arrive after the first render; read again for it.
  useResetOnChange(isGm, () => setChoice(readChoice(isGm)));
  const choose = useCallback((next: RollVisibility) => {
    setChoice(next);
    writeChoice(next);
  }, []);
  return [choice, choose];
}
