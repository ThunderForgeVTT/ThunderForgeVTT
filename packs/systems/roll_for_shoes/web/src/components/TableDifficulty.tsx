import { useState } from "react";
import { Button, Input, StatusBadge } from "@thunderforge/host";

import {
  BAND_DICE,
  BAND_LABEL,
  BAND_TARGET,
  BANDS,
  type Band,
  type DifficultyMode,
  type TableDifficulty as TableDifficultyValue,
} from "../game.ts";
import { cardTitleClass, fieldClass, gmDieClass, hintClass } from "./styles.ts";

export interface TableDifficultyProps {
  difficulty: TableDifficultyValue;
  /** This world's difficulty mode: whether bands exist, and what one means. */
  mode: DifficultyMode;
  busy: boolean;
  /** Why the last change was refused, if it was. */
  refusal: string | null;
  onSetNumber: (target: number) => void;
  onSetBand: (band: Band) => void;
  onClear: () => void;
}

/**
 * What has to be beaten, for the whole table.
 *
 * Drawn two ways from the one value. The Game Master gets the control: a
 * number to name, the bands when this world uses them, and a way to take it
 * back. Everyone else gets the number and nothing to change — while the Game
 * Master's number stands, a player's sheet has no field of its own to type
 * into, and the server would not take a change from them if it had.
 *
 * In a world that rolls for difficulty the Game Master's dice are shown to
 * everyone, as they would be on a table. They were rolled by the server when
 * the band was chosen, once; choosing again is the only way to roll again,
 * and only the Game Master can choose.
 */
export function TableDifficulty({
  difficulty,
  mode,
  busy,
  refusal,
  onSetNumber,
  onSetBand,
  onClear,
}: TableDifficultyProps) {
  const [number, setNumber] = useState("");
  const { target, band, gmDice, canSet } = difficulty;
  const typed = Number(number);
  const nameable =
    number.trim() !== "" && Number.isInteger(typed) && typed >= 1;

  return (
    <section className="grid gap-2" data-testid="rfs-table-difficulty">
      <h3 className={cardTitleClass}>What has to be beaten</h3>

      {target === null ? (
        <p className={hintClass} data-testid="rfs-table-none">
          {canSet
            ? "Nothing is set. Until you set it, each player enters a number on their own sheet."
            : "The Game Master has not set it. Enter their number on your sheet."}
        </p>
      ) : (
        // Announced when it changes: a player with the sheet open and a
        // screen reader running hears the new number without hunting for it.
        <div className="grid gap-1" role="status" aria-live="polite">
          <p className="text-sm">
            <span className={hintClass}>
              {canSet
                ? "The table is rolling against "
                : "The Game Master says: beat "}
            </span>
            <span
              data-testid="rfs-table-target"
              className="text-base font-semibold tabular-nums"
            >
              {target}
            </span>
            {band ? (
              <span className={hintClass} data-testid="rfs-table-band">
                {" "}
                ({BAND_LABEL[band]})
              </span>
            ) : null}
          </p>
          {gmDice ? (
            <div
              className="flex flex-wrap items-center gap-1.5"
              data-testid="rfs-table-gm-dice"
            >
              {gmDice.map((face, index) => (
                <span
                  key={index}
                  className={gmDieClass}
                  data-testid={`rfs-table-gm-die-${index}`}
                >
                  {face}
                </span>
              ))}
              <span className={hintClass}>the Game Master&rsquo;s dice</span>
            </div>
          ) : null}
        </div>
      )}

      {canSet ? (
        <div className="grid gap-2">
          <form
            className="flex flex-wrap items-end gap-2"
            onSubmit={(event) => {
              event.preventDefault();
              if (nameable) {
                onSetNumber(typed);
                setNumber("");
              }
            }}
          >
            <label className="grid gap-1">
              <span className={hintClass}>A number</span>
              <Input
                type="number"
                inputMode="numeric"
                min={1}
                className={`${fieldClass} w-24`}
                data-testid="rfs-table-number"
                value={number}
                onChange={(event) => setNumber(event.target.value)}
              />
            </label>
            <Button
              type="submit"
              size="sm"
              disabled={busy || !nameable}
              data-testid="rfs-table-set"
            >
              Set for the table
            </Button>
            {target !== null ? (
              <Button
                type="button"
                size="sm"
                variant="secondary"
                disabled={busy}
                data-testid="rfs-table-clear"
                onClick={onClear}
              >
                Clear
              </Button>
            ) : null}
          </form>

          {mode === "free" ? null : (
            <div className="flex flex-wrap gap-1.5">
              {BANDS.map((each) => (
                <Button
                  key={each}
                  type="button"
                  size="sm"
                  variant={band === each ? "primary" : "secondary"}
                  disabled={busy}
                  data-testid={`rfs-table-band-${each}`}
                  onClick={() => onSetBand(each)}
                >
                  {BAND_LABEL[each]}
                  <span className={hintClass}>
                    {" "}
                    {mode === "rolled"
                      ? `${BAND_DICE[each]}d6`
                      : String(BAND_TARGET[each])}
                  </span>
                </Button>
              ))}
            </div>
          )}

          {refusal ? (
            <StatusBadge variant="danger" data-testid="rfs-table-refusal">
              {refusal}
            </StatusBadge>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}

export default TableDifficulty;
