import { Button } from "@thunderforge/host";

import {
  BAND_DICE,
  BAND_LABEL,
  BAND_TARGET,
  BANDS,
  type Band,
  type DifficultyMode,
} from "../game.ts";
import { cardTitleClass, gmDieClass, hintClass } from "./styles.ts";

export interface DifficultyPickerProps {
  /** This world's mode. `free` never reaches here — the picker is not shown. */
  mode: Exclude<DifficultyMode, "free">;
  /** The band last chosen, so the choice stays visible after it is made. */
  chosen: Band | null;
  /** The Game Master's dice, when the world rolls them and a band was chosen. */
  gmDice: number[] | null;
  busy: boolean;
  onChoose: (band: Band) => void;
}

/**
 * How hard it is, chosen by name instead of typed as a number.
 *
 * Both modes end in the same place: a number in the opposition field, which
 * the player can still see and still overrule. That is the whole design — the
 * bands are a way of *arriving at* the opposition, never a second kind of
 * opposition running alongside it, so the comparison downstream stays the one
 * comparison the core game has always made.
 *
 * The Game Master's dice, when there are any, are drawn here and only here.
 * They are never part of the character's pool: a six on one of these earns
 * nobody anything.
 */
export function DifficultyPicker({
  mode,
  chosen,
  gmDice,
  busy,
  onChoose,
}: DifficultyPickerProps) {
  return (
    <section className="mt-3 grid gap-2" data-testid="rfs-difficulty">
      <h3 className={cardTitleClass}>
        How hard is it?{" "}
        <span className={hintClass} data-testid="rfs-difficulty-mode">
          {mode === "rolled"
            ? "the Game Master rolls for it"
            : "a fixed number for each"}
        </span>
      </h3>

      <div className="flex flex-wrap gap-1.5">
        {BANDS.map((band) => (
          <Button
            key={band}
            type="button"
            size="sm"
            variant={chosen === band ? "primary" : "secondary"}
            disabled={busy}
            data-testid={`rfs-band-${band}`}
            onClick={() => onChoose(band)}
          >
            {BAND_LABEL[band]}
            <span className={hintClass}>
              {" "}
              {mode === "rolled"
                ? `${BAND_DICE[band]}d6`
                : String(BAND_TARGET[band])}
            </span>
          </Button>
        ))}
      </div>

      {gmDice !== null ? (
        <div className="grid gap-1">
          <div
            className="flex flex-wrap items-center gap-1.5"
            data-testid="rfs-gm-dice"
          >
            {gmDice.map((face, index) => (
              <span
                key={index}
                data-testid={`rfs-gm-die-${index}`}
                className={gmDieClass}
              >
                {face}
              </span>
            ))}
          </div>
          <p className={hintClass}>
            The Game Master rolled{" "}
            <span
              data-testid="rfs-gm-total"
              className="font-semibold tabular-nums text-foreground"
            >
              {gmDice.reduce((running, face) => running + face, 0)}
            </span>
            . Beat it — matching it is not beating it.
          </p>
        </div>
      ) : null}
    </section>
  );
}

export default DifficultyPicker;
