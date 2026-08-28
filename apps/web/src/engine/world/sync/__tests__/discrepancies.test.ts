import { beforeEach, describe, expect, it } from "vitest";
import {
  discrepancyFor,
  noteDiscrepancy,
  resetDiscrepanciesForTests,
} from "../discrepancies";
import { discrepancyToShow } from "@/components/world/rollDiscrepancy";

/**
 * The carrier between the server that finds a discrepancy and the view that
 * shows one (spec 028 FR-064 to FR-067).
 *
 * This module exists because those two ends were built correctly and did not
 * meet: the server returned a discrepancy on a reconcile outcome, `RollResult`
 * rendered one from a record, and nothing carried the first to the second, so
 * the disclosure was unreachable in the running product. These tests are
 * mostly about that seam staying joined — including the field *names*, which
 * is where it came apart the first time.
 */

beforeEach(() => {
  resetDiscrepanciesForTests();
});

describe("disclosed discrepancies", () => {
  it("hands back what the server said about a roll", () => {
    noteDiscrepancy("roll-1", {
      userId: "player-1",
      reportedValue: 20,
      determinedValue: 7,
    });

    expect(discrepancyFor("roll-1")).toEqual({
      userId: "player-1",
      reportedValue: 20,
      determinedValue: 7,
    });
  });

  it("knows nothing about a roll nobody disclosed anything about", () => {
    expect(discrepancyFor("roll-unknown")).toBeNull();
    expect(discrepancyFor(null)).toBeNull();
    expect(discrepancyFor(undefined)).toBeNull();
  });

  /**
   * The accuracy obligation, at this layer. A half-populated record must be
   * dropped rather than shown with a guess in the missing half: a false
   * discrepancy puts an innocent player under suspicion in front of the only
   * person who can act on it, while a missed one costs nothing.
   */
  it("refuses anything it cannot state completely", () => {
    noteDiscrepancy("a", { userId: "u", reportedValue: 20 });
    noteDiscrepancy("b", { userId: "u", determinedValue: 7 });
    noteDiscrepancy("c", { reportedValue: 20, determinedValue: 7 });
    noteDiscrepancy("d", {
      userId: "u",
      reportedValue: Number.NaN,
      determinedValue: 7,
    });
    noteDiscrepancy("e", {
      userId: "u",
      reportedValue: 20,
      determinedValue: Number.POSITIVE_INFINITY,
    });
    noteDiscrepancy(null, { userId: "u", reportedValue: 20, determinedValue: 7 });

    for (const id of ["a", "b", "c", "d", "e"]) {
      expect(discrepancyFor(id), `${id} was incomplete and must be dropped`).toBeNull();
    }
  });

  /**
   * The specific break this seam already had: the server sends
   * `reportedValue` and the display was reading `claimedValue`, so a value
   * that travelled the whole way arrived and read as absent — which is
   * indistinguishable from "there was no discrepancy", the quietest possible
   * way for a disclosure to be lost.
   */
  it("shows a disclosure that arrived under the server's own field name", () => {
    const disclosed = { userId: "u", reportedValue: 20, determinedValue: 7 };
    noteDiscrepancy("roll-2", disclosed);
    const stored = discrepancyFor("roll-2")!;

    const shown = discrepancyToShow(
      { claimedValue: stored.reportedValue, determinedValue: stored.determinedValue },
      true,
    );
    expect(shown).toEqual({ claimedValue: 20, determinedValue: 7 });

    // And straight from the server's spelling, with no translation at all.
    expect(discrepancyToShow({ reportedValue: 20, determinedValue: 7 }, true)).toEqual({
      claimedValue: 20,
      determinedValue: 7,
    });
  });

  it("still shows nothing to a player, however it arrived", () => {
    noteDiscrepancy("roll-3", { userId: "u", reportedValue: 20, determinedValue: 7 });
    const stored = discrepancyFor("roll-3")!;

    expect(
      discrepancyToShow({ reportedValue: stored.reportedValue, determinedValue: 7 }, false),
      "FR-067: the note exists for the Game Master and for nobody else",
    ).toBeNull();
  });
});
