import { choiceLabel, fieldLabel } from "./catalogue.ts";
import { LockToggle } from "./LockToggle.tsx";

export interface ChoiceGroupProps {
  field: string;
  choices: readonly string[];
  value: string;
  /** Unique within the document; radio groups are named from it. */
  name: string;
  locked: boolean;
  onLock(): void;
  onChoose(choice: string): void;
}

/**
 * One part: native radios, so the group is one tab stop and the arrow keys
 * move through it (FR-017).
 */
export function ChoiceGroup(props: ChoiceGroupProps) {
  const { field, choices, value, name, locked, onLock, onChoose } = props;
  const label = fieldLabel(field);
  return (
    <fieldset className="tfhb-group" data-testid={`hero-field-${field}`}>
      <legend className="tfhb-legend">
        <span>{label}</span>
        <LockToggle
          field={field}
          label={label}
          locked={locked}
          onToggle={onLock}
        />
      </legend>
      <div className="tfhb-choices">
        {choices.map((choice) => (
          <label key={choice} className="tfhb-chip">
            <input
              type="radio"
              name={name}
              value={choice}
              checked={value === choice}
              data-testid={`hero-choice-${field}-${choice}`}
              onChange={() => onChoose(choice)}
            />
            <span>{choiceLabel(field, choice)}</span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}
