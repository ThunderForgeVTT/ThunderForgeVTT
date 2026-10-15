import { useMemo } from "react";
import {
  createHero,
  HERO_COLORS,
  HERO_FLAGS,
  HERO_LABELS,
  HERO_PARTS,
  HERO_TEXT_LIMITS,
  randomHero,
  resolveHero,
  type HeroSpec,
} from "@thunderforge/heroes";
import { svgUrl } from "./heroArt.ts";

// The hero builder, on the page: every part and colour the heroes package
// knows, drawn as native controls so a thumb can work them, and the token
// redrawn as each one changes. "Roll" draws a fresh hero and keeps the name.

interface Props {
  spec: HeroSpec;
  onChange: (spec: HeroSpec) => void;
  onApply: () => void;
  busy: boolean;
}

const PART_KEYS = Object.keys(HERO_PARTS) as (keyof typeof HERO_PARTS)[];

export default function HeroBuilder({ spec, onChange, onApply, busy }: Props) {
  const full = useMemo(() => resolveHero(spec), [spec]);
  const preview = useMemo(() => svgUrl(createHero(spec).token()), [spec]);
  const set = (key: string, value: unknown) => onChange({ ...spec, [key]: value });

  return (
    <form
      className="builder"
      onSubmit={(e) => {
        e.preventDefault();
        onApply();
      }}
    >
      <div className="builder-head">
        <img className="builder-preview" src={preview} alt="" width={96} height={96} />
        <label className="builder-name">
          <span>{HERO_LABELS.fields.name}</span>
          <input
            type="text"
            value={spec.name}
            maxLength={HERO_TEXT_LIMITS.name}
            onChange={(e) => set("name", e.target.value)}
            required
          />
        </label>
        <div className="builder-actions">
          <button
            type="button"
            onClick={() =>
              onChange({ ...randomHero(`${spec.name}|${Date.now()}`), name: spec.name })
            }
          >
            Roll
          </button>
          <button type="submit" className="builder-apply" disabled={busy}>
            {busy ? "Drawing…" : "Walk as this hero"}
          </button>
        </div>
      </div>
      <div className="builder-grid">
        {PART_KEYS.map((key) => (
          <label key={key}>
            <span>{HERO_LABELS.fields[key] ?? key}</span>
            <select value={full[key]} onChange={(e) => set(key, e.target.value)}>
              {HERO_PARTS[key].map((choice) => (
                <option key={choice} value={choice}>
                  {HERO_LABELS.choices[key]?.[choice] ?? choice}
                </option>
              ))}
            </select>
          </label>
        ))}
        {HERO_FLAGS.map((key) => (
          <label key={key} className="builder-flag">
            <input
              type="checkbox"
              checked={full[key]}
              onChange={(e) => set(key, e.target.checked)}
            />
            <span>{HERO_LABELS.fields[key] ?? key}</span>
          </label>
        ))}
      </div>
      <div className="builder-swatches" aria-label="Colours">
        {HERO_COLORS.map((key) => (
          <label key={key} title={HERO_LABELS.fields[key] ?? key}>
            <input type="color" value={full[key]} onChange={(e) => set(key, e.target.value)} />
            <span>{HERO_LABELS.fields[key] ?? key}</span>
          </label>
        ))}
      </div>
    </form>
  );
}
