import { describe, expect, it } from "vitest";
import {
  DEFAULT_SIZE_SCALE,
  resolveSizeScale,
  type SizeCategoriesLookup,
} from "../sizeCategory";

const GENIE_SIZE_CATEGORIES: SizeCategoriesLookup = {
  diminutive: { scale: 0.5, label: "Diminutive" },
  small: { scale: 0.75, label: "Small" },
  medium: { scale: 1.0, label: "Medium" },
  large: { scale: 2.0, label: "Large" },
  huge: { scale: 3.0, label: "Huge" },
  colossal: { scale: 4.0, label: "Colossal" },
};

describe("resolveSizeScale", () => {
  it("resolves a Diminutive NPC's token to a sub-1 scale (spec 018 quickstart Scenario 3)", () => {
    expect(resolveSizeScale(GENIE_SIZE_CATEGORIES, "diminutive")).toBe(0.5);
  });

  it("resolves a Colossal NPC's token to a multi-square-proportional scale (spec 018 quickstart Scenario 3)", () => {
    expect(resolveSizeScale(GENIE_SIZE_CATEGORIES, "colossal")).toBe(4.0);
  });

  it("falls back to the default scale for an unrecognized category", () => {
    expect(resolveSizeScale(GENIE_SIZE_CATEGORIES, "gigantic")).toBe(
      DEFAULT_SIZE_SCALE,
    );
  });

  it("falls back to the default scale when the actor has no size category", () => {
    expect(resolveSizeScale(GENIE_SIZE_CATEGORIES, null)).toBe(
      DEFAULT_SIZE_SCALE,
    );
    expect(resolveSizeScale(GENIE_SIZE_CATEGORIES, undefined)).toBe(
      DEFAULT_SIZE_SCALE,
    );
  });

  it("falls back to the default scale when the game system publishes no sizeCategories table", () => {
    expect(resolveSizeScale(undefined, "colossal")).toBe(DEFAULT_SIZE_SCALE);
    expect(resolveSizeScale(null, "colossal")).toBe(DEFAULT_SIZE_SCALE);
    expect(resolveSizeScale({}, "colossal")).toBe(DEFAULT_SIZE_SCALE);
  });
});
