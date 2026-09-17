import { describe, expect, it } from "vitest";
import { raceOnSheet, raceSourceOf } from "../raceOnSheet";

/** A manifest shaped like 5e's, declaring where a race is written. */
const DECLARED = {
  appearance: { race: { source: { slot: "traitData", field: "race" } } },
};

describe("raceSourceOf", () => {
  it("reads a declared source", () => {
    expect(raceSourceOf(DECLARED)).toEqual({
      slot: "traitData",
      field: "race",
    });
  });

  it("is null for a system with no races", () => {
    expect(raceSourceOf({})).toBeNull();
    expect(raceSourceOf({ appearance: {} })).toBeNull();
    expect(raceSourceOf(null)).toBeNull();
    expect(
      raceSourceOf({ appearance: { race: { source: { slot: "traitData" } } } }),
    ).toBeNull();
  });
});

describe("raceOnSheet", () => {
  it("reads the sheet's own text through the declared source", () => {
    expect(
      raceOnSheet(DECLARED, { trait_data: { race: "High Elf", size: "M" } }),
    ).toBe("High Elf");
  });

  it("is null when the system declares no race", () => {
    expect(raceOnSheet({}, { trait_data: { race: "High Elf" } })).toBeNull();
  });

  it("is null when the sheet has no value", () => {
    expect(raceOnSheet(DECLARED, null)).toBeNull();
    expect(raceOnSheet(DECLARED, {})).toBeNull();
    expect(raceOnSheet(DECLARED, { trait_data: {} })).toBeNull();
    expect(raceOnSheet(DECLARED, { trait_data: { race: "  " } })).toBeNull();
  });

  it("is null when the value is not text", () => {
    expect(raceOnSheet(DECLARED, { trait_data: { race: 7 } })).toBeNull();
    expect(
      raceOnSheet(DECLARED, { trait_data: { race: { name: "Elf" } } }),
    ).toBeNull();
  });
});
