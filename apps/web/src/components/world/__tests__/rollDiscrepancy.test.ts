import { describe, expect, it } from "vitest";
import { discrepancyToShow } from "../rollDiscrepancy";

/**
 * When a roll is worth a second look, and — far more often — when it is not
 * (spec 028 US7, T102a; FR-065, FR-067, FR-067a, FR-068).
 *
 * A missed discrepancy costs nothing: the Game Master runs their table either
 * way. A false one puts an innocent player under suspicion in front of the
 * only person who can act on it. Every test here is on the second side of that
 * asymmetry — the failures they catch are all "something was marked that
 * should not have been".
 *
 * What the mark *looks like* is e2e's business (T108a); this repo's vitest runs
 * in a `node` environment and has no component tests.
 */
describe("discrepancyToShow", () => {
  it("shows the two values when the server genuinely determined a different one", () => {
    expect(
      discrepancyToShow({ claimedValue: 18, determinedValue: 11 }, true),
    ).toEqual({
      claimedValue: 18,
      determinedValue: 11,
    });
  });

  /**
   * FR-067. The note exists for the Game Master and for nobody else — a player
   * catching sight of a mark against another player is the harm the whole
   * design is built to avoid, and it would arrive with no way to ask about it.
   */
  it("shows a player nothing, however far apart the numbers are", () => {
    expect(
      discrepancyToShow({ claimedValue: 20, determinedValue: 3 }, false),
    ).toBeNull();
  });

  /** An agreeing comparison is not a discrepancy; it is the ordinary case. */
  it("says nothing when the two values agree", () => {
    expect(
      discrepancyToShow({ claimedValue: 12, determinedValue: 12 }, true),
    ).toBeNull();
  });

  /**
   * FR-068. Where the server has no independent basis for an outcome there is
   * nothing to compare, and absence of evidence is not a flag.
   */
  it("says nothing when the server never compared anything", () => {
    expect(discrepancyToShow(null, true)).toBeNull();
    expect(discrepancyToShow(undefined, true)).toBeNull();
  });

  /**
   * FR-067a. A half-populated record is a timeout, a parse failure, or a
   * version the server could not replay — every one of them an ambiguity, and
   * every ambiguity reads as no discrepancy. Marking a roll because one number
   * was missing would accuse someone of a bug of ours.
   */
  it("says nothing when only one of the two values arrived", () => {
    expect(discrepancyToShow({ claimedValue: 18 }, true)).toBeNull();
    expect(discrepancyToShow({ determinedValue: 11 }, true)).toBeNull();
    expect(
      discrepancyToShow({ claimedValue: 18, determinedValue: null }, true),
    ).toBeNull();
  });

  /**
   * A value that is not a finite number cannot be compared. `NaN !== NaN`
   * would otherwise make every unparsed value a discrepancy — the loudest
   * possible false positive, and the easiest to ship.
   */
  it("says nothing when a value is not a number it can compare", () => {
    expect(
      discrepancyToShow(
        { claimedValue: Number.NaN, determinedValue: 11 },
        true,
      ),
    ).toBeNull();
    expect(
      discrepancyToShow(
        { claimedValue: 18, determinedValue: Number.POSITIVE_INFINITY },
        true,
      ),
    ).toBeNull();
    expect(
      discrepancyToShow(
        { claimedValue: "18" as unknown as number, determinedValue: 11 },
        true,
      ),
    ).toBeNull();
  });

  /**
   * The field's spelling on the wire is not settled until the server's
   * detection lands (T099), and reading the wrong one would silently disable
   * the whole feature rather than fail visibly.
   */
  it("reads either spelling the comparison may ship under", () => {
    expect(
      discrepancyToShow({ claimed_value: 18, determined_value: 11 }, true),
    ).toEqual({
      claimedValue: 18,
      determinedValue: 11,
    });
  });
});
