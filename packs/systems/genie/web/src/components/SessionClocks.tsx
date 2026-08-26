import React, { useState } from 'react';
import {
  cardClass,
  cardTitleClass,
  dangerButtonClass,
  fieldClass,
  hintClass,
  primaryButtonClass,
  sectionHeadingClass,
  smallButtonClass,
} from './styles';

/**
 * Spec 018 (Genie) User Story 7 — the Doom Clock and Puzzle Clocks
 * (FR-015/FR-016). Props-driven like `SessionWishPool.tsx`: the host page
 * owns the `genieSession(worldId)` query and the
 * `advanceDoomClock`/`createPuzzleClock`/`advancePuzzleClock` mutations
 * (`contracts/genie-session-loop.md`); this component only renders the
 * clocks and, for the GM, the controls to advance/create them.
 */

export interface GeniePuzzleClockData {
  id: string;
  label: string;
  segmentsCurrent: number;
  segmentsMax: number;
  resolvedAt?: string | null;
}

export interface SessionClocksProps {
  doomClockCurrent: number;
  doomClockMax: number;
  puzzleClocks: GeniePuzzleClockData[];
  sessionStatus?: 'ACTIVE' | 'WON' | 'LOST';
  /** Whether the current viewer is this world's GM (clock mutations are GM-only, research.md R8). */
  isGm?: boolean;
  onAdvanceDoomClock?: (delta: number) => void | Promise<void>;
  onAdvancePuzzleClock?: (clockId: string, delta: number) => void | Promise<void>;
  onCreatePuzzleClock?: (label: string, segmentsMax: number) => void | Promise<void>;
}

/** A row of filled/empty segment wedges — the shared rendering for both the Doom Clock and every Puzzle Clock. */
const ClockSegments: React.FC<{ current: number; max: number; filledClassName: string }> = ({
  current,
  max,
  filledClassName,
}) => (
  <div className="flex flex-wrap gap-1" role="img" aria-label={`${current} of ${max} segments filled`}>
    {Array.from({ length: max }).map((_, i) => (
      <span
        key={i}
        className={`inline-block h-4 w-4 rounded-sm border ${i < current ? filledClassName : 'border-border bg-muted'}`}
      />
    ))}
  </div>
);

export const SessionClocks: React.FC<SessionClocksProps> = ({
  doomClockCurrent,
  doomClockMax,
  puzzleClocks,
  sessionStatus = 'ACTIVE',
  isGm = false,
  onAdvanceDoomClock,
  onAdvancePuzzleClock,
  onCreatePuzzleClock,
}) => {
  const [newLabel, setNewLabel] = useState('');
  const [newSegmentsMax, setNewSegmentsMax] = useState(4);

  const controlsEnabled = isGm && sessionStatus === 'ACTIVE';

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!controlsEnabled || !onCreatePuzzleClock || !newLabel.trim() || newSegmentsMax <= 0) return;
    await onCreatePuzzleClock(newLabel.trim(), newSegmentsMax);
    setNewLabel('');
    setNewSegmentsMax(4);
  };

  return (
    <div className={cardClass} data-testid="session-clocks">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h2 className={cardTitleClass}>Doom Clock</h2>
        <span className={hintClass}>
          {doomClockCurrent} / {doomClockMax}
        </span>
      </div>
      <div className="mt-3 flex flex-wrap items-center gap-3">
        <ClockSegments
          current={doomClockCurrent}
          max={doomClockMax}
          filledClassName="border-destructive bg-destructive"
        />
        {controlsEnabled && onAdvanceDoomClock && (
          <button
            type="button"
            className={dangerButtonClass}
            /* Visible text is just "Advance" (the heading beside it already
             * says Doom Clock), but the accessible name stays fully
             * qualified — it has to distinguish this from each Puzzle
             * Clock's own "Advance" button. */
            aria-label="Advance Doom Clock"
            onClick={() => onAdvanceDoomClock(1)}
            disabled={doomClockCurrent >= doomClockMax}
          >
            Advance
          </button>
        )}
      </div>

      <div className="mt-5 border-t border-border pt-4">
        <h3 className={sectionHeadingClass}>Puzzle Clocks</h3>
        {puzzleClocks.length === 0 ? (
          <p className={`mt-2 ${hintClass}`}>No Puzzle Clocks yet.</p>
        ) : (
          <ul className="mt-3 flex flex-col gap-2">
            {puzzleClocks.map((clock) => {
              const resolved = !!clock.resolvedAt;
              return (
                <li
                  key={clock.id}
                  className="flex items-center justify-between gap-3 rounded-lg border border-border bg-muted/40 p-3"
                >
                  <div className="flex flex-col gap-1.5">
                    <span
                      className={`text-sm font-medium ${resolved ? 'text-emerald-600 dark:text-emerald-400' : ''}`}
                    >
                      {clock.label}
                      {resolved && ' (Resolved)'}
                    </span>
                    <div className="flex flex-wrap items-center gap-2">
                      <ClockSegments
                        current={clock.segmentsCurrent}
                        max={clock.segmentsMax}
                        filledClassName="border-sky-600 bg-sky-500"
                      />
                      <span className={hintClass}>
                        {clock.segmentsCurrent} / {clock.segmentsMax}
                      </span>
                    </div>
                  </div>
                  {controlsEnabled && onAdvancePuzzleClock && !resolved && (
                    <button
                      type="button"
                      className={smallButtonClass}
                      onClick={() => onAdvancePuzzleClock(clock.id, 1)}
                    >
                      Advance
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        )}

        {controlsEnabled && onCreatePuzzleClock && (
          <form onSubmit={handleCreate} className="mt-3 flex flex-wrap items-end gap-2">
            <div className="flex min-w-0 flex-1 flex-col gap-1">
              <label htmlFor="new-clock-label" className={hintClass}>
                New Puzzle Clock
              </label>
              <input
                id="new-clock-label"
                type="text"
                className={`w-full ${fieldClass}`}
                placeholder="Objective / station name"
                value={newLabel}
                onChange={(e) => setNewLabel(e.target.value)}
              />
            </div>
            <div className="flex w-20 flex-col gap-1">
              <label htmlFor="new-clock-segments" className={hintClass}>
                Segments
              </label>
              <input
                id="new-clock-segments"
                type="number"
                min={1}
                className={`w-full ${fieldClass}`}
                value={newSegmentsMax}
                onChange={(e) => setNewSegmentsMax(Number(e.target.value))}
              />
            </div>
            <button
              type="submit"
              className={primaryButtonClass}
              disabled={!newLabel.trim() || newSegmentsMax <= 0}
            >
              Create
            </button>
          </form>
        )}
      </div>

      {/* FR-016: once the session is won or lost, every clock mutation is closed. */}
      {sessionStatus !== 'ACTIVE' && (
        <p
          className={`mt-4 border-t border-border pt-3 text-sm font-semibold ${
            sessionStatus === 'WON' ? 'text-emerald-600 dark:text-emerald-400' : 'text-destructive'
          }`}
        >
          Session {sessionStatus === 'WON' ? 'won' : 'lost'} — clocks are locked.
        </p>
      )}
    </div>
  );
};

export default SessionClocks;
