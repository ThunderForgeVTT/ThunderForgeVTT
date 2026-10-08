/**
 * Spec 084 (FR-019): the demo's mirror of 5e's roll facets, held to the
 * cases `packs/systems/dnd5e/server/src/roll_facets_tests.rs` holds the
 * server to.
 */
import { describe, expect, it } from "vitest";

import {
  NO_D20,
  NO_DAMAGE_ADVANTAGE,
  facetRows,
  shapeD20,
  shapeDamage,
} from "./facets";

describe("shapeD20", () => {
  it("rolls a check's d20 twice and keeps the higher for advantage", () => {
    expect(shapeD20("1d20 + MODIFIER", "ADVANTAGE")).toEqual({
      formula: "2d20kh1 + MODIFIER",
      facets: ["advantage"],
    });
  });

  it("keeps the lower for disadvantage", () => {
    expect(shapeD20("1d20+5", "DISADVANTAGE")).toEqual({
      formula: "2d20kl1+5",
      facets: ["disadvantage"],
    });
  });

  it("leaves a normal roll as declared", () => {
    expect(shapeD20("1d20 + MODIFIER", "NORMAL")).toEqual({
      formula: "1d20 + MODIFIER",
      facets: [],
    });
  });

  it("takes the first d20 that keeps nothing, and leaves the rest", () => {
    expect(shapeD20("2d20kh1 + 1d20 + d4", "ADVANTAGE").formula).toBe(
      "2d20kh1 + 2d20kh1 + d4",
    );
    expect(shapeD20("d20 + 3d20", "ADVANTAGE").formula).toBe("2d20kh1 + 3d20");
  });

  it("refuses a formula with no d20 to roll twice", () => {
    expect(() => shapeD20("1d6 + 2", "ADVANTAGE")).toThrow(NO_D20);
    expect(() => shapeD20("1d200", "ADVANTAGE")).toThrow(NO_D20);
  });
});

describe("shapeDamage", () => {
  it("rolls damage as declared, and never with advantage", () => {
    expect(shapeDamage("1d8 + 3", "NORMAL")).toEqual({
      formula: "1d8 + 3",
      facets: [],
    });
    expect(() => shapeDamage("1d8 + 3", "ADVANTAGE")).toThrow(
      NO_DAMAGE_ADVANTAGE,
    );
  });
});

describe("facetRows", () => {
  it("names each facet as 5e names it", () => {
    expect(facetRows(["advantage", "disadvantage"])).toEqual([
      { id: "advantage", label: "Advantage" },
      { id: "disadvantage", label: "Disadvantage" },
    ]);
  });
});
