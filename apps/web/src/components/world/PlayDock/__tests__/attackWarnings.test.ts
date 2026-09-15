import { describe, expect, it } from "vitest";
import type { AttackPreviewRecord } from "@/types/attack";
import { warningTexts } from "../AttackLog/attackText";

function preview(over: Partial<AttackPreviewRecord>): AttackPreviewRecord {
  return {
    distance: 20,
    flags: [],
    turn: { allowed: true, activeLabel: null },
    reach: null,
    rangeNormal: null,
    rangeLong: null,
    unit: "ft",
    ...over,
  };
}

describe("an attack's warnings before rolling (spec 046 FR-033)", () => {
  it("says how far, and how far the attack reaches", () => {
    expect(
      warningTexts(preview({ flags: ["OUT_OF_REACH"], reach: 5 })),
    ).toEqual(["Out of reach: 20 ft, reach 5 ft"]);
  });

  it("says a long shot against its normal range, and beyond against its long", () => {
    const bow = { rangeNormal: 80, rangeLong: 320 };
    expect(
      warningTexts(preview({ ...bow, distance: 100, flags: ["LONG_RANGE"] })),
    ).toEqual(["Long range: 100 ft, normal range 80 ft"]);
    expect(
      warningTexts(preview({ ...bow, distance: 400, flags: ["BEYOND_RANGE"] })),
    ).toEqual(["Beyond range: 400 ft, range 320 ft"]);
  });

  it("names line of sight and an undeclared reach, one sentence per flag", () => {
    expect(
      warningTexts(
        preview({ flags: ["OUT_OF_REACH", "NO_LINE_OF_SIGHT"], reach: 5 }),
      ),
    ).toEqual([
      "Out of reach: 20 ft, reach 5 ft",
      "No line of sight: a wall is in the way",
    ]);
    expect(warningTexts(preview({ flags: ["NO_REACH_DECLARED"] }))).toEqual([
      "This attack declares no reach or range",
    ]);
  });

  it("warns without numbers it was not given", () => {
    expect(
      warningTexts(preview({ distance: null, flags: ["OUT_OF_REACH"] })),
    ).toEqual(["Out of reach"]);
  });
});
