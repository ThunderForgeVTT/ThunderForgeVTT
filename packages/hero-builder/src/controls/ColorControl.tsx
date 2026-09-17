import { useEffect, useState } from "react";
import { HERO_PALETTES, HEX_COLOR } from "@thunderforge/heroes";
import { fieldLabel } from "./catalogue.ts";
import { LockToggle } from "./LockToggle.tsx";

export interface ColorControlProps {
  field: string;
  /** The resolved colour, whether set or following. */
  value: string;
  /** False when the colour follows its default or another colour (FR-005). */
  isSet: boolean;
  name: string;
  locked: boolean;
  onLock(): void;
  onPick(hex: string): void;
  onFollow(): void;
}

/**
 * A colour: the palette's swatches as radios, and the hex as text a person
 * can read and type (FR-008, FR-018). A half-typed hex changes nothing; the
 * hero redraws once the text is a whole `#rrggbb`.
 */
export function ColorControl(props: ColorControlProps) {
  const { field, value, isSet, name, locked, onLock, onPick, onFollow } = props;
  const label = fieldLabel(field);
  const swatches =
    (HERO_PALETTES as Record<string, readonly string[] | undefined>)[field] ??
    [];
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);

  return (
    <fieldset className="tfhb-group" data-testid={`hero-field-${field}`}>
      <legend className="tfhb-legend">
        <span>{label}</span>
        <span
          className="tfhb-following"
          data-testid={`hero-following-${field}`}
          hidden={isSet}
        >
          following
        </span>
        <LockToggle
          field={field}
          label={label}
          locked={locked}
          onToggle={onLock}
        />
      </legend>
      <div className="tfhb-choices">
        {swatches.map((hex, index) => (
          <label key={index} className="tfhb-swatch" title={hex}>
            <input
              type="radio"
              name={name}
              value={hex}
              checked={value === hex}
              aria-label={`${label} ${hex}`}
              data-testid={`hero-swatch-${field}-${index}`}
              onChange={() => onPick(hex)}
            />
            <span style={{ background: hex }} />
          </label>
        ))}
      </div>
      <div className="tfhb-hex-row">
        <label className="tfhb-hex">
          <span className="tfhb-visually-hidden">{`${label} hex`}</span>
          <span
            aria-hidden="true"
            className="tfhb-hex-dot"
            style={{ background: value }}
          />
          <input
            type="text"
            inputMode="text"
            spellCheck={false}
            autoComplete="off"
            maxLength={7}
            value={draft}
            data-testid={`hero-hex-${field}`}
            onChange={(event) => {
              const text = event.target.value.trim();
              setDraft(text);
              if (HEX_COLOR.test(text)) onPick(text.toLowerCase());
            }}
            onBlur={() => setDraft(value)}
          />
        </label>
        {isSet && (
          <button
            type="button"
            className="tfhb-button"
            data-testid={`hero-follow-${field}`}
            onClick={onFollow}
          >
            Follow default
          </button>
        )}
      </div>
    </fieldset>
  );
}
