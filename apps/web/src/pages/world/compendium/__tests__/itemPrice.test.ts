import { describe, expect, it } from "vitest";
import type { ItemPriceRecord } from "@/api/items";
import {
  formatItemPrice,
  parsePriceAmount,
} from "@/pages/world/compendium/itemPrice";

function price(overrides: Partial<ItemPriceRecord> = {}): ItemPriceRecord {
  return {
    itemId: "item-1",
    amount: 40,
    currencyLabel: "gp",
    isSuggested: false,
    updatedAt: "2026-09-01T00:00:00",
    ...overrides,
  };
}

describe("formatItemPrice", () => {
  it("puts the Game Master's own currency label after the number", () => {
    expect(formatItemPrice(price())).toBe("40 gp");
  });

  it("says nothing about currency when the Game Master named none", () => {
    expect(formatItemPrice(price({ currencyLabel: null }))).toBe("40");
    expect(formatItemPrice(price({ currencyLabel: "   " }))).toBe("40");
  });

  it("labels a suggestion as a suggestion (ADR-058)", () => {
    expect(formatItemPrice(price({ isSuggested: true }))).toBe(
      "40 gp (suggested)",
    );
  });

  it("renders nothing at all for an unpriced item", () => {
    expect(formatItemPrice(null)).toBeNull();
    expect(formatItemPrice(undefined)).toBeNull();
  });

  it("keeps free and unpriced distinct", () => {
    expect(formatItemPrice(price({ amount: 0, currencyLabel: null }))).toBe(
      "0",
    );
  });
});

describe("parsePriceAmount", () => {
  it("reads a whole number", () => {
    expect(parsePriceAmount("40")).toBe(40);
    expect(parsePriceAmount("  0 ")).toBe(0);
    expect(parsePriceAmount("-5")).toBe(-5);
  });

  it("treats an empty box as no note rather than as zero", () => {
    expect(parsePriceAmount("")).toBeNull();
    expect(parsePriceAmount("   ")).toBeNull();
  });

  it("refuses anything that is not a whole number", () => {
    expect(parsePriceAmount("forty")).toBeNull();
    expect(parsePriceAmount("4.5")).toBeNull();
  });
});
