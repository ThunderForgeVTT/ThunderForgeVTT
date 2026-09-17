import { useEffect, useState } from "react";
import { HERO_LABELS, HERO_RACES, type RaceKey } from "@thunderforge/heroes";

export interface RollBarProps {
  race: RaceKey | null;
  seed: string;
  onRace(race: RaceKey | null): void;
  /** Roll with this seed. */
  onRoll(seed: string): void;
  /** Roll with a seed nobody has seen. */
  onRollFresh(): void;
}

/**
 * The race picker and the dice (FR-007, FR-007a). The race is a setting of
 * the roll, held by the builder and never written into the spec (B5a).
 */
export function RollBar({
  race,
  seed,
  onRace,
  onRoll,
  onRollFresh,
}: RollBarProps) {
  const [draft, setDraft] = useState(seed);
  useEffect(() => setDraft(seed), [seed]);
  const raceLabel = HERO_LABELS.fields.race ?? "Race";

  return (
    <div className="tfhb-roll" data-testid="hero-roll-bar">
      <label className="tfhb-text">
        <span>{raceLabel}</span>
        <select
          value={race ?? ""}
          data-testid="hero-race"
          onChange={(event) =>
            onRace(event.target.value === "" ? null : event.target.value)
          }
        >
          <option value="">Any</option>
          {Object.keys(HERO_RACES).map((key) => (
            <option key={key} value={key}>
              {HERO_LABELS.races[key] ?? key}
            </option>
          ))}
        </select>
      </label>
      <button
        type="button"
        className="tfhb-button tfhb-dice"
        data-testid="hero-roll"
        onClick={onRollFresh}
      >
        <span aria-hidden="true">🎲</span> Roll
      </button>
      <form
        className="tfhb-seed"
        onSubmit={(event) => {
          event.preventDefault();
          const typed = draft.trim();
          if (typed !== "") onRoll(typed);
        }}
      >
        <label className="tfhb-text">
          <span>Seed</span>
          <input
            type="text"
            value={draft}
            spellCheck={false}
            autoComplete="off"
            maxLength={64}
            data-testid="hero-seed"
            onChange={(event) => setDraft(event.target.value)}
          />
        </label>
        <button
          type="submit"
          className="tfhb-button"
          data-testid="hero-roll-seed"
        >
          Roll this seed
        </button>
      </form>
    </div>
  );
}
