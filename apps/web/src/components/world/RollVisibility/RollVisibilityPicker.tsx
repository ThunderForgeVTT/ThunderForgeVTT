import type { RollVisibility } from "@/types/roll";

import { CHOICE_LABEL, choicesFor } from "./storedChoice";

export interface RollVisibilityPickerProps {
  isGm: boolean;
  value: RollVisibility;
  onChange: (choice: RollVisibility) => void;
}

/**
 * Spec 081: who sees the next roll. A player picks between the table and the
 * GM's eyes; a GM between the table and themselves alone.
 */
export function RollVisibilityPicker({
  isGm,
  value,
  onChange,
}: RollVisibilityPickerProps) {
  return (
    <label className="flex items-center gap-1.5 text-xs">
      <span className="text-muted-foreground">Roll for</span>
      <select
        value={value}
        onChange={(event) => onChange(event.target.value as RollVisibility)}
        data-testid="roll-visibility-picker"
        aria-label="Who sees this roll"
        className="rounded border border-input bg-transparent px-1 py-0.5 text-xs text-inherit"
      >
        {choicesFor(isGm).map((choice) => (
          <option key={choice} value={choice} className="text-black">
            {CHOICE_LABEL[choice]}
          </option>
        ))}
      </select>
    </label>
  );
}
