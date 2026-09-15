import { describe, expect, it } from "vitest";
import {
  DEFAULT_FOOTPRINT,
  declaredSizesOf,
  footprintText,
  resolveSizeFootprint,
  sizeIdOnSheet,
  slotKey,
} from "../sizeCategory";

/** A manifest shaped like Genie's, after spec 046 moved its sizes. */
const MANIFEST = {
  combat: {
    sizes: {
      source: { slot: "traitData", field: "size_category" },
      categories: [
        { id: "diminutive", label: "Diminutive", footprint: 0.5 },
        { id: "medium", label: "Medium", footprint: 1 },
        { id: "large", label: "Large", footprint: 2 },
        { id: "colossal", label: "Colossal", footprint: 4 },
      ],
    },
  },
};

describe("declared sizes", () => {
  it("reads a manifest's combat.sizes", () => {
    const sizes = declaredSizesOf(MANIFEST);
    expect(sizes?.source).toEqual({
      slot: "traitData",
      field: "size_category",
    });
    expect(sizes?.categories.map((c) => c.id)).toEqual([
      "diminutive",
      "medium",
      "large",
      "colossal",
    ]);
  });

  it("is null for a manifest with no sizes, including the old sizeCategories table", () => {
    expect(declaredSizesOf({})).toBeNull();
    expect(declaredSizesOf(null)).toBeNull();
    expect(
      declaredSizesOf({ sizeCategories: { large: { scale: 2 } } }),
    ).toBeNull();
  });

  it("resolves a category to its footprint, and anything unknown to one square", () => {
    const sizes = declaredSizesOf(MANIFEST);
    expect(resolveSizeFootprint(sizes, "colossal")).toBe(4);
    expect(resolveSizeFootprint(sizes, "diminutive")).toBe(0.5);
    expect(resolveSizeFootprint(sizes, "gigantic")).toBe(DEFAULT_FOOTPRINT);
    expect(resolveSizeFootprint(sizes, null)).toBe(DEFAULT_FOOTPRINT);
    expect(resolveSizeFootprint(null, "colossal")).toBe(DEFAULT_FOOTPRINT);
  });

  it("reads the size off a sheet through the declared slot and field", () => {
    const sizes = declaredSizesOf(MANIFEST);
    expect(slotKey("traitData")).toBe("trait_data");
    expect(
      sizeIdOnSheet(sizes, { trait_data: { size_category: "large" } }),
    ).toBe("large");
    expect(sizeIdOnSheet(sizes, { trait_data: {} })).toBeNull();
    expect(sizeIdOnSheet(sizes, null)).toBeNull();
  });

  it("says how many squares a size fills", () => {
    expect(footprintText(0.5)).toBe("Fills ½ a square");
    expect(footprintText(1)).toBe("Fills 1 square");
    expect(footprintText(4)).toBe("Fills 4×4 squares");
  });
});
