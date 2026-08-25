import { describe, expect, it } from "vitest";
import {
  DEFAULT_ABILITY_FACETS,
  resolveAbilityLabel,
  resolveAbilityPluralLabel,
  toAbilityClassificationKey,
  type AbilityFacetsLookup,
} from "@/utils/abilityFacets";

/**
 * Spec 025 (T019): covers every fallback case in
 * contracts/ability-facets.md. The point of these is that the resolver is
 * *total* — a pack can ship a missing, partial, or outright malformed
 * `abilityFacets` block and the UI must still render a sensible label rather
 * than blank text or a thrown error (FR-011).
 */
describe("resolveAbilityLabel", () => {
  it("falls back to the built-in label when no facets are published", () => {
    expect(resolveAbilityLabel(undefined, "spell")).toBe("Spell");
    expect(resolveAbilityLabel(null, "feat")).toBe("Feat");
  });

  it("falls back when the facets block is present but empty", () => {
    expect(resolveAbilityLabel({}, "power")).toBe("Power");
  });

  it("uses the system's label when one is published", () => {
    const lookup: AbilityFacetsLookup = { spell: { label: "Scroll" } };
    expect(resolveAbilityLabel(lookup, "spell")).toBe("Scroll");
  });

  it("falls back per-classification, not all-or-nothing", () => {
    // A pack that re-labels only one classification must not lose the others.
    const lookup: AbilityFacetsLookup = { spell: { label: "Scroll" } };
    expect(resolveAbilityLabel(lookup, "spell")).toBe("Scroll");
    expect(resolveAbilityLabel(lookup, "feat")).toBe("Feat");
    expect(resolveAbilityLabel(lookup, "talent")).toBe("Talent");
  });

  it("falls back when the label is empty or whitespace-only", () => {
    expect(resolveAbilityLabel({ spell: { label: "" } }, "spell")).toBe("Spell");
    expect(resolveAbilityLabel({ spell: { label: "   " } }, "spell")).toBe("Spell");
  });

  it("falls back when the entry is not an object", () => {
    // A pack shipping `"spell": "Scroll"` instead of `{ label: "Scroll" }`.
    const malformed = { spell: "Scroll" } as unknown as AbilityFacetsLookup;
    expect(resolveAbilityLabel(malformed, "spell")).toBe("Spell");
  });

  it("falls back when the entry is null", () => {
    const malformed = { spell: null } as unknown as AbilityFacetsLookup;
    expect(resolveAbilityLabel(malformed, "spell")).toBe("Spell");
  });

  it("ignores unknown keys rather than erroring", () => {
    const lookup = {
      cantrip: { label: "Cantrip" },
      spell: { label: "Scroll" },
    } as AbilityFacetsLookup;
    expect(resolveAbilityLabel(lookup, "spell")).toBe("Scroll");
  });
});

describe("resolveAbilityPluralLabel", () => {
  it("uses an explicit pluralLabel when published", () => {
    const lookup: AbilityFacetsLookup = {
      spell: { label: "Scroll", pluralLabel: "Scrolls" },
    };
    expect(resolveAbilityPluralLabel(lookup, "spell")).toBe("Scrolls");
  });

  it("falls back to the entry's own singular label, NOT label + 's'", () => {
    // Deliberate: not every term or language pluralizes by appending "s",
    // so reusing the singular beats guessing wrong.
    const lookup: AbilityFacetsLookup = { spell: { label: "Scroll" } };
    expect(resolveAbilityPluralLabel(lookup, "spell")).toBe("Scroll");
    expect(resolveAbilityPluralLabel(lookup, "spell")).not.toBe("Scrolls");
  });

  it("falls back to the built-in plural when the entry is unusable", () => {
    expect(resolveAbilityPluralLabel(undefined, "feat")).toBe("Feats");
    expect(resolveAbilityPluralLabel({}, "feat")).toBe("Feats");
    expect(resolveAbilityPluralLabel({ feat: { label: "  " } }, "feat")).toBe("Feats");
  });

  it("falls back to the singular when pluralLabel is empty", () => {
    const lookup: AbilityFacetsLookup = { spell: { label: "Scroll", pluralLabel: "  " } };
    expect(resolveAbilityPluralLabel(lookup, "spell")).toBe("Scroll");
  });
});

describe("toAbilityClassificationKey", () => {
  it("accepts the known keys in any case", () => {
    expect(toAbilityClassificationKey("SPELL")).toBe("spell");
    expect(toAbilityClassificationKey("Talent")).toBe("talent");
  });

  it("falls back to spell for anything unrecognized, matching the server", () => {
    expect(toAbilityClassificationKey("cantrip")).toBe("spell");
    expect(toAbilityClassificationKey("")).toBe("spell");
  });
});

describe("DEFAULT_ABILITY_FACETS", () => {
  it("covers every classification with a non-empty singular and plural", () => {
    for (const [key, entry] of Object.entries(DEFAULT_ABILITY_FACETS)) {
      expect(entry.label.trim(), `${key} label`).not.toBe("");
      expect(entry.pluralLabel.trim(), `${key} pluralLabel`).not.toBe("");
    }
  });
});
