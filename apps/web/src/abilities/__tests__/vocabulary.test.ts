import { describe, expect, it } from "vitest";

import {
  DEFAULT_VOCABULARY,
  labelFor,
  pluralLabelFor,
  recognises,
  typeFor,
  type AbilityVocabulary,
} from "../vocabulary";

/**
 * Spec 033 — the browser's half of the vocabulary, which is lookup and
 * nothing else. Assembly is the server's and is tested there
 * (`ability_vocabulary_tests.rs`); what matters here is that every lookup is
 * total, and that an unrecognised identity is shown as itself rather than
 * dressed up as something else.
 */
const genie: AbilityVocabulary = {
  umbrella: { label: "Ability", pluralLabel: "Abilities" },
  types: [
    {
      id: "spell",
      label: "Scroll",
      pluralLabel: "Scrolls",
      order: 0,
      builtin: true,
      binds: "CHARACTER",
      grade: null,
    },
    {
      id: "talent",
      label: "Knack",
      pluralLabel: "Knacks",
      order: 1,
      builtin: true,
      binds: "CHARACTER",
      grade: null,
    },
  ],
};

describe("resolving a type's words", () => {
  it("uses the system's own word", () => {
    expect(labelFor(genie, "spell")).toBe("Scroll");
    expect(pluralLabelFor(genie, "spell")).toBe("Scrolls");
    expect(labelFor(genie, "talent")).toBe("Knack");
  });

  it("shows an unrecognised type as itself, never as another type", () => {
    // FR-034 and FR-035. The reader this replaced resolved anything unknown
    // to "spell", which is precisely the silent mislabelling the spec forbids
    // — an Enchantment in a Genie world would have read as a Scroll.
    expect(labelFor(genie, "enchantment")).toBe("enchantment");
    expect(pluralLabelFor(genie, "enchantment")).toBe("enchantment");
  });

  it("answers whether a type is recognised, which decides how it is presented", () => {
    expect(recognises(genie, "spell")).toBe(true);
    expect(recognises(genie, "enchantment")).toBe(false);
    expect(typeFor(genie, "enchantment")).toBeNull();
  });
});

describe("the default vocabulary", () => {
  it("names the concept with the application's word", () => {
    expect(DEFAULT_VOCABULARY.umbrella.label).toBe("Ability");
    expect(DEFAULT_VOCABULARY.umbrella.pluralLabel).toBe("Abilities");
  });

  it("resolves anything to itself rather than throwing or blanking", () => {
    // What renders while the real vocabulary is in flight, and if it never
    // arrives. A pack failure must produce no blank label (SC-013).
    expect(labelFor(DEFAULT_VOCABULARY, "spell")).toBe("spell");
    expect(labelFor(DEFAULT_VOCABULARY, "")).toBe("");
    expect(recognises(DEFAULT_VOCABULARY, "spell")).toBe(false);
  });
});

describe("facets a type may declare", () => {
  it("carries a grade where one is declared and null where it is not", () => {
    const fivee: AbilityVocabulary = {
      umbrella: { label: "Spell", pluralLabel: "Spells" },
      types: [
        {
          id: "spell",
          label: "Spell",
          pluralLabel: "Spells",
          order: 0,
          builtin: true,
          binds: "CHARACTER",
          grade: { label: "Level", min: 0, max: 9 },
        },
        {
          id: "enchantment",
          label: "Enchantment",
          pluralLabel: "Enchantments",
          order: 1,
          builtin: false,
          binds: "ITEM",
          grade: null,
        },
      ],
    };

    expect(typeFor(fivee, "spell")?.grade?.label).toBe("Level");
    expect(typeFor(fivee, "enchantment")?.grade).toBeNull();
    expect(typeFor(fivee, "enchantment")?.binds).toBe("ITEM");
  });
});
