import { describe, expect, it } from "vitest";
import type { WorldPlayState } from "@/api/playPause";
import {
  formatPauseMoment,
  pauseHistoryRows,
  pausedSince,
} from "@/pages/world/playPauseStatus";

/** Spec 051 US5 (T059): what members are told about a pause, from times alone. */

const EARLIER = "2026-09-01T18:00:00Z";
const LIFTED = "2026-09-02T09:30:00Z";
const LATER = "2026-09-10T20:15:00Z";

function state(overrides: Partial<WorldPlayState>): WorldPlayState {
  return { paused: false, pausedAt: null, history: [], ...overrides };
}

describe("pausedSince", () => {
  it("is null when nothing is known, or play is not paused", () => {
    expect(pausedSince(null)).toBeNull();
    expect(
      pausedSince(
        state({ history: [{ pausedAt: EARLIER, liftedAt: LIFTED }] }),
      ),
    ).toBeNull();
  });

  it("is the pause's start while paused", () => {
    expect(
      pausedSince(
        state({
          paused: true,
          pausedAt: LATER,
          history: [
            { pausedAt: LATER, liftedAt: null },
            { pausedAt: EARLIER, liftedAt: LIFTED },
          ],
        }),
      ),
    ).toBe(LATER);
  });

  it("falls back to the open span when pausedAt is missing", () => {
    expect(
      pausedSince(
        state({
          paused: true,
          history: [
            { pausedAt: EARLIER, liftedAt: LIFTED },
            { pausedAt: LATER, liftedAt: null },
          ],
        }),
      ),
    ).toBe(LATER);
  });
});

describe("pauseHistoryRows", () => {
  it("is empty for an unknown or never-paused world", () => {
    expect(pauseHistoryRows(null)).toEqual([]);
    expect(pauseHistoryRows(state({}))).toEqual([]);
  });

  it("orders newest first without changing what it was given", () => {
    const history = [
      { pausedAt: EARLIER, liftedAt: LIFTED },
      { pausedAt: LATER, liftedAt: null },
    ];
    const rows = pauseHistoryRows(state({ paused: true, history }));
    expect(rows.map((row) => row.pausedAt)).toEqual([LATER, EARLIER]);
    expect(history[0].pausedAt).toBe(EARLIER);
  });

  it("carries times and nothing else", () => {
    const rows = pauseHistoryRows(
      state({ history: [{ pausedAt: EARLIER, liftedAt: LIFTED }] }),
    );
    expect(Object.keys(rows[0]).sort()).toEqual(["liftedAt", "pausedAt"]);
  });
});

describe("formatPauseMoment", () => {
  it("writes a date and a time, the long form at least as long as the short", () => {
    const long = formatPauseMoment(LATER);
    const short = formatPauseMoment(LATER, "short");
    expect(long).toMatch(/\d/);
    expect(short).toMatch(/\d/);
    expect(long.length).toBeGreaterThanOrEqual(short.length);
    expect(long).toContain(String(new Date(LATER).getFullYear()));
  });
});
