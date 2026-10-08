import { ADVANTAGE_CHOICES, ADVANTAGE_LABEL } from "./advantage";
import type { Advantage } from "./advantage";

export interface AdvantagePickerProps {
  value: Advantage;
  onChange: (choice: Advantage) => void;
  disabled?: boolean;
}

/**
 * Spec 084 FR-018: rolls the next check or attack with advantage or
 * disadvantage. It is drawn only for a system that declares roll facets, and
 * the caller puts it back to Normal once a roll is sent.
 */
export function AdvantagePicker({
  value,
  onChange,
  disabled = false,
}: AdvantagePickerProps) {
  return (
    <div
      role="radiogroup"
      aria-label="Roll with advantage"
      data-testid="roll-advantage-picker"
      className="inline-flex overflow-hidden rounded border border-input text-xs"
    >
      {ADVANTAGE_CHOICES.map((choice) => (
        <button
          key={choice}
          type="button"
          role="radio"
          aria-checked={value === choice}
          disabled={disabled}
          onClick={() => onChange(choice)}
          data-testid={`roll-advantage-${choice}`}
          className={`px-2 py-0.5 transition-colors disabled:opacity-60 ${
            value === choice
              ? "bg-primary text-primary-foreground"
              : "hover:bg-muted"
          }`}
        >
          {ADVANTAGE_LABEL[choice]}
        </button>
      ))}
    </div>
  );
}
