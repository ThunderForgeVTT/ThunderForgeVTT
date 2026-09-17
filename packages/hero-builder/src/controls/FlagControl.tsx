import { fieldLabel } from "./catalogue.ts";
import { LockToggle } from "./LockToggle.tsx";

export interface FlagControlProps {
  field: string;
  value: boolean;
  locked: boolean;
  onLock(): void;
  onToggle(on: boolean): void;
}

/** A flag: a switch. */
export function FlagControl({
  field,
  value,
  locked,
  onLock,
  onToggle,
}: FlagControlProps) {
  const label = fieldLabel(field);
  return (
    <div className="tfhb-flag" data-testid={`hero-field-${field}`}>
      <label className="tfhb-chip">
        <input
          type="checkbox"
          role="switch"
          checked={value}
          data-testid={`hero-flag-${field}`}
          onChange={(event) => onToggle(event.target.checked)}
        />
        <span>{label}</span>
      </label>
      <LockToggle
        field={field}
        label={label}
        locked={locked}
        onToggle={onLock}
      />
    </div>
  );
}
