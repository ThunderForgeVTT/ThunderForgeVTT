import { useEffect, useState } from "react";

export interface TextControlProps {
  field: string;
  label: string;
  value: string;
  maxLength: number;
  onText(text: string): void;
}

/** Name and title. Holds its own draft, so an empty name the spec refuses
 * stays in the box while the hero keeps its last good name. */
export function TextControl({
  field,
  label,
  value,
  maxLength,
  onText,
}: TextControlProps) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  return (
    <label className="tfhb-text">
      <span>{label}</span>
      <input
        type="text"
        value={draft}
        maxLength={maxLength}
        data-testid={`hero-text-${field}`}
        onChange={(event) => {
          setDraft(event.target.value);
          onText(event.target.value);
        }}
      />
    </label>
  );
}
