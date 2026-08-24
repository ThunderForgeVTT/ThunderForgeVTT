import React, { useState } from 'react';

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
  <div className="flex gap-1" role="img" aria-label={`${current} of ${max} segments filled`}>
    {Array.from({ length: max }).map((_, i) => (
      <span
        key={i}
        className={`inline-block w-4 h-4 rounded-sm border ${i < current ? filledClassName : 'bg-gray-100 border-gray-300'}`}
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
    <div className="p-4 border rounded-lg bg-white shadow-sm" data-testid="session-clocks">
      <h2 className="text-lg font-bold mb-2">Doom Clock</h2>
      <div className="flex items-center gap-3 mb-1">
        <ClockSegments current={doomClockCurrent} max={doomClockMax} filledClassName="bg-red-600 border-red-700" />
        <span className="text-sm text-gray-600">
          {doomClockCurrent} / {doomClockMax}
        </span>
      </div>
      {controlsEnabled && onAdvanceDoomClock && (
        <button
          type="button"
          className="mt-1 mb-4 px-3 py-1 text-sm bg-red-600 text-white rounded font-semibold disabled:opacity-50"
          onClick={() => onAdvanceDoomClock(1)}
          disabled={doomClockCurrent >= doomClockMax}
        >
          Advance Doom Clock
        </button>
      )}

      <h2 className="text-lg font-bold mt-4 mb-2">Puzzle Clocks</h2>
      {puzzleClocks.length === 0 && <p className="text-sm text-gray-500 mb-2">No Puzzle Clocks yet.</p>}
      <ul className="flex flex-col gap-2 mb-4">
        {puzzleClocks.map((clock) => {
          const resolved = !!clock.resolvedAt;
          return (
            <li key={clock.id} className="flex items-center justify-between gap-3 border-b pb-2 last:border-0">
              <div className="flex flex-col gap-1">
                <span className={`text-sm font-semibold ${resolved ? 'text-green-700' : ''}`}>
                  {clock.label}
                  {resolved && ' (Resolved)'}
                </span>
                <div className="flex items-center gap-2">
                  <ClockSegments
                    current={clock.segmentsCurrent}
                    max={clock.segmentsMax}
                    filledClassName="bg-blue-600 border-blue-700"
                  />
                  <span className="text-xs text-gray-600">
                    {clock.segmentsCurrent} / {clock.segmentsMax}
                  </span>
                </div>
              </div>
              {controlsEnabled && onAdvancePuzzleClock && !resolved && (
                <button
                  type="button"
                  className="px-2 py-1 text-xs bg-blue-600 text-white rounded font-semibold"
                  onClick={() => onAdvancePuzzleClock(clock.id, 1)}
                >
                  Advance
                </button>
              )}
            </li>
          );
        })}
      </ul>

      {controlsEnabled && onCreatePuzzleClock && (
        <form onSubmit={handleCreate} className="flex items-end gap-2">
          <div className="flex flex-col">
            <label htmlFor="new-clock-label" className="text-xs font-semibold text-gray-700">
              New Puzzle Clock
            </label>
            <input
              id="new-clock-label"
              type="text"
              className="border rounded p-1 text-sm"
              placeholder="Objective / station name"
              value={newLabel}
              onChange={(e) => setNewLabel(e.target.value)}
            />
          </div>
          <div className="flex flex-col">
            <label htmlFor="new-clock-segments" className="text-xs font-semibold text-gray-700">
              Segments
            </label>
            <input
              id="new-clock-segments"
              type="number"
              min={1}
              className="border rounded p-1 text-sm w-16"
              value={newSegmentsMax}
              onChange={(e) => setNewSegmentsMax(Number(e.target.value))}
            />
          </div>
          <button
            type="submit"
            className="px-3 py-1.5 bg-gray-800 text-white rounded text-sm font-semibold disabled:opacity-50"
            disabled={!newLabel.trim() || newSegmentsMax <= 0}
          >
            Create
          </button>
        </form>
      )}

      {sessionStatus !== 'ACTIVE' && (
        <p className={`mt-3 font-bold ${sessionStatus === 'WON' ? 'text-green-700' : 'text-red-700'}`}>
          Session {sessionStatus === 'WON' ? 'won' : 'lost'} — all clock mutations are now closed (FR-016).
        </p>
      )}
    </div>
  );
};

export default SessionClocks;
