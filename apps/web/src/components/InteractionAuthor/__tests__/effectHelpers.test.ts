import { describe, expect, it } from "vitest";
import type { EffectDeclaration } from "@/api/interactives";
import {
  effectNamespace,
  helpersFor,
  missingRequiredFields,
} from "../effectHelpers";

/**
 * Spec 031 FR-028 — the helper row is the registry, not a list.
 *
 * # Why the fixtures are invented rather than imported
 *
 * These effects do not exist. That is the point: every assertion below has to
 * hold for a build whose contributors this file has never heard of, because
 * the guarantee ADR-054 asks for is that the authoring surface offers exactly
 * what was compiled in. A test written against the real `lore.open` would
 * pass just as happily if the implementation had the id typed into it.
 *
 * The one real-looking case is the required-field rule, which is asserted
 * against a reference field shaped like `lore.open`'s — because that is the
 * case that motivated it, and because "entry not chosen" is the state a Game
 * Master placing their first lore marker is actually in.
 */

function declaration(
  id: string,
  subjectKinds: EffectDeclaration["subjectKinds"],
  config: EffectDeclaration["config"] = [],
): EffectDeclaration {
  return { id, label: `Do ${id}`, description: "", subjectKinds, config };
}

const ENTRY_FIELD: EffectDeclaration["config"][number] = {
  key: "entry",
  label: "Lore page",
  kind: "reference",
  referenceOf: "loreEntry",
  options: null,
  required: true,
};

describe("effectNamespace", () => {
  it("takes the part before the first dot", () => {
    expect(effectNamespace("quill.inscribe")).toBe("quill");
    // Two effects from one subsystem share a namespace, which is what lets
    // presentation be decided once per subsystem rather than once per effect.
    expect(effectNamespace("quill.erase")).toBe(effectNamespace("quill.ink"));
  });

  it("treats an unnamespaced id as its own namespace", () => {
    // Ids are namespaced by convention and checked at assembly, not here. A
    // bare id still has to produce *a* group, because the alternative is a
    // helper with no icon rule and therefore no button.
    expect(effectNamespace("bare")).toBe("bare");
  });
});

describe("helpersFor", () => {
  const registry = [
    declaration("quill.inscribe", ["prop"]),
    declaration("portcullis.raise", ["door"]),
    declaration("quill.erase", ["prop", "door"]),
  ];

  it("offers only what attaches to the subject in hand", () => {
    expect(helpersFor(registry, "prop").map((h) => h.id)).toEqual([
      "quill.inscribe",
      "quill.erase",
    ]);
    expect(helpersFor(registry, "door").map((h) => h.id)).toEqual([
      "portcullis.raise",
      "quill.erase",
    ]);
  });

  it("offers nothing for a build that contributed nothing", () => {
    // Not an error and not an empty-looking form with a dead button: a build
    // with no contributors is legitimate, and the panel says so in words.
    expect(helpersFor([], "region")).toEqual([]);
  });

  it("keeps the registry's own order", () => {
    // Assembly order, not alphabetical. Re-sorting would mean this module
    // holding an opinion about which subsystem matters most.
    const reversed = [...registry].reverse();
    expect(helpersFor(reversed, "prop").map((h) => h.id)).toEqual([
      "quill.erase",
      "quill.inscribe",
    ]);
  });

  it("carries the namespace through, so presentation never re-parses the id", () => {
    expect(helpersFor(registry, "prop").map((h) => h.namespace)).toEqual([
      "quill",
      "quill",
    ]);
  });
});

describe("missingRequiredFields", () => {
  const opensLore = declaration("quill.open", ["prop"], [ENTRY_FIELD]);

  it("names a required reference nobody has chosen", () => {
    expect(missingRequiredFields(opensLore, {}).map((f) => f.key)).toEqual([
      "entry",
    ]);
    expect(missingRequiredFields(opensLore, { entry: "" })).toHaveLength(1);
  });

  it("is satisfied once the reference points somewhere", () => {
    expect(missingRequiredFields(opensLore, { entry: "abc" })).toEqual([]);
  });

  it("counts a required boolean answered 'no' as answered", () => {
    // A required boolean is asking to be decided. Treating `false` as blank
    // would make one of its two legitimate answers unsavable.
    const asks = declaration(
      "quill.ask",
      ["prop"],
      [
        {
          key: "loud",
          label: "Loudly",
          kind: "boolean",
          referenceOf: null,
          options: null,
          required: true,
        },
      ],
    );
    expect(missingRequiredFields(asks, { loud: false })).toEqual([]);
  });

  it("ignores fields the declaration did not require", () => {
    const optional = declaration(
      "quill.maybe",
      ["prop"],
      [{ ...ENTRY_FIELD, required: false }],
    );
    expect(missingRequiredFields(optional, {})).toEqual([]);
  });

  it("asks nothing of scenery", () => {
    // No effect chosen is a complete, savable state — an interactive that
    // carries no effect is legitimate, not an unfinished one.
    expect(missingRequiredFields(null, {})).toEqual([]);
  });
});
