import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SheetLayout } from "../SheetLayout";
import { resetUnknownKindWarnings } from "../resolve";
import {
  declarationsDrift,
  declarationsFrom,
  resetDeclarationDriftWarnings,
} from "../declarations";
import type {
  LayoutDeclaration,
  LayoutNode,
  SheetDeclarations,
  SheetValue,
} from "../types";

/**
 * Spec 032, T041 and T042 — what an interface pack's layout puts on a sheet.
 *
 * # Why this renders to markup rather than into a DOM
 *
 * `apps/web` has neither jsdom nor testing-library, and adding either means a
 * lockfile change shared with every other workstream. The renderer is a pure
 * function of its props — no state, no effects — so server-rendering it
 * observes exactly what a viewer would see. That is also the right level for
 * the rules under test: they are about what *appears*, and specifically about
 * several things not appearing — a heading over an empty set, a text box over
 * a computed number, and a bar where a player expects boxes to tick.
 *
 * The Forge layout is read from `packs/interface/forge/interface.json` rather
 * than restated here, so a change to the shipping pack is felt by this test
 * rather than passing it.
 */

const FORGE = JSON.parse(
  readFileSync(
    fileURLToPath(
      new URL(
        "../../../../../packs/interface/forge/interface.json",
        import.meta.url,
      ),
    ),
    "utf8",
  ),
) as { layout: LayoutDeclaration };

function stored(
  id: string,
  label: string,
  value: string,
  abbreviation: string | null = null,
): SheetValue {
  return { id, label, abbreviation, value, origin: "stored" };
}

/**
 * A pool, with both numbers as numbers.
 *
 * The `value` string is deliberately *not* parseable here in some tests: the
 * bar comes from `fraction`, and that is the whole point of T019a.
 */
function pool(
  id: string,
  label: string,
  current: number,
  max: number | null,
  rendered = max === null ? String(current) : `${current} / ${max}`,
): SheetValue {
  return {
    id,
    label,
    abbreviation: null,
    value: rendered,
    fraction: { current, max },
    origin: "stored",
  };
}

/** A bounded run of marks. Same two numbers as a pool; a different thing. */
function track(
  id: string,
  label: string,
  filled: number,
  of: number,
  origin: SheetValue["origin"] = "stored",
): SheetValue {
  return {
    id,
    label,
    abbreviation: null,
    value: `${filled} / ${of}`,
    track: { filled, of },
    origin,
  };
}

/** An ordered ladder of named states, one of them current. */
function ladder(
  id: string,
  label: string,
  current: string | null,
  options: string[],
  origin: SheetValue["origin"] = "stored",
): SheetValue {
  return {
    id,
    label,
    abbreviation: null,
    value: current ?? "",
    state: { current, options },
    origin,
  };
}

function grouped(value: SheetValue, group: string): SheetValue {
  return { ...value, group };
}

function derived(
  id: string,
  label: string,
  value: string,
  abbreviation: string | null = null,
): SheetValue {
  return { id, label, abbreviation, value, origin: "derived" };
}

/**
 * A Genie-shaped actor: three abilities, two resources, and — as Genie's
 * manifest actually says — no skills at all.
 */
const GENIE: Partial<SheetDeclarations> = {
  attributes: [
    stored("might", "Might", "3", "MGT"),
    stored("cunning", "Cunning", "2", "CUN"),
    stored("spirit", "Spirit", "4", "SPI"),
  ],
  resources: [
    stored("wishPoints", "Wish Points", "4 / 7"),
    stored("insight", "Insight", "2"),
  ],
  skills: [],
  movement: [stored("walk", "Walk", "30")],
  derived: [derived("manifestation", "Manifestation", "7")],
};

/**
 * The identifiers the markup mentions, first mention first.
 *
 * `data-value-id` appears on the line and again on the control inside it, so
 * the repeats are collapsed: what is under test is the order, which is the
 * system's own and never the pack's.
 */
function idsInOrder(markup: string): string[] {
  const seen: string[] = [];
  for (const match of markup.matchAll(/data-value-id="([a-zA-Z0-9]+)"/g)) {
    if (seen[seen.length - 1] !== match[1] && !seen.includes(match[1])) {
      seen.push(match[1]);
    }
  }
  return seen;
}

function render(
  layout: LayoutDeclaration,
  declarations: Partial<SheetDeclarations>,
): string {
  return renderToStaticMarkup(
    React.createElement(SheetLayout, { layout, declarations }),
  );
}

describe("Forge's layout against a Genie-shaped actor", () => {
  const markup = render(FORGE.layout, GENIE);

  it("renders every declared attribute", () => {
    for (const abbreviation of ["MGT", "CUN", "SPI"]) {
      expect(markup).toContain(abbreviation);
    }
    for (const id of ["might", "cunning", "spirit"]) {
      expect(markup).toContain(`data-value-id="${id}"`);
    }
  });

  it("renders every declared resource", () => {
    expect(markup).toContain("Wish Points");
    expect(markup).toContain("Insight");
  });

  it("omits the skills section entirely rather than heading blank space", () => {
    // The failure this rule exists to prevent: a "Skills" heading with
    // nothing under it, telling a player their sheet is broken.
    expect(markup).not.toContain("Skills");
    // One row list, not two: the movement one. The skills one is not there
    // at all, and neither is Forge's trailing `other` list — Genie's values
    // are all claimed by a named set.
    expect(markup.match(/data-slot="row-list"/g)).toHaveLength(1);
    expect(markup).not.toContain("Everything Else");
    // The other three sections are still there, so the absence above is
    // about the empty set and not about nothing having rendered.
    expect(markup).toContain("Attributes");
    expect(markup).toContain("Resources");
  });

  it("renders nothing at all for a system that declares nothing", () => {
    expect(render(FORGE.layout, {})).toBe("");
  });

  it("keeps a section whose children are only partly empty", () => {
    // Forge's last-but-one section pairs movement with derived values.
    // Movement alone is enough for the section to exist.
    const movementOnly = render(FORGE.layout, {
      movement: [stored("walk", "Walk", "30")],
    });
    expect(movementOnly).toContain("Movement and Derived Values");
    expect(movementOnly).toContain("Walk");
    expect(movementOnly).not.toContain("Attributes");
  });

  it("draws a Genie-shaped actor's unclaimed values in Forge's last section", () => {
    // FR-034: Forge ends with `other`, so nothing a system publishes falls
    // off the bottom of the base pack.
    const withExtras = render(FORGE.layout, {
      ...GENIE,
      all: [
        ...(GENIE.attributes ?? []),
        ...(GENIE.resources ?? []),
        stored("boon", "Bound Boon", "A debt owed to a sultan"),
      ],
    });
    expect(withExtras).toContain("Everything Else");
    expect(withExtras).toContain("Bound Boon");
    expect(withExtras.match(/data-slot="row-list"/g)).toHaveLength(2);
  });
});

describe("origin decides whether a value may be edited", () => {
  const layout: LayoutDeclaration = [
    { kind: "badgeGrid", of: "attributes", columns: 2 },
  ];

  it("gives a stored value an editable control", () => {
    const markup = render(layout, {
      attributes: [stored("strength", "Strength", "16", "STR")],
    });
    expect(markup).toContain("<input");
    expect(markup).toContain('data-origin="stored"');
    expect(markup).toContain('value="16"');
  });

  it("gives a derived value no editable control at all", () => {
    const markup = render(layout, {
      attributes: [derived("strengthMod", "Strength Modifier", "+3", "STR")],
    });
    // A text box over a computed number invites the two to disagree.
    expect(markup).not.toContain("<input");
    expect(markup).not.toContain("<textarea");
    expect(markup).toContain('aria-readonly="true"');
    expect(markup).toContain('data-origin="derived"');
    expect(markup).toContain("+3");
  });

  it("mixes the two in one grid without letting either take the other's form", () => {
    const markup = render(layout, {
      attributes: [
        stored("strength", "Strength", "16", "STR"),
        derived("strengthMod", "Strength Modifier", "+3", "MOD"),
      ],
    });
    expect(markup.match(/<input/g)).toHaveLength(1);
    expect(markup.match(/aria-readonly="true"/g)).toHaveLength(1);
  });

  it("never offers to change a derived value", () => {
    const onValueChange = vi.fn();
    const markup = renderToStaticMarkup(
      React.createElement(SheetLayout, {
        layout,
        declarations: { attributes: [derived("mod", "Modifier", "+3")] },
        onValueChange,
      }),
    );
    expect(markup).not.toContain("<input");
    expect(onValueChange).not.toHaveBeenCalled();
  });
});

describe("generic constructs keep the system's declaration order", () => {
  it("renders a badgeGrid in declaration order, not sorted", () => {
    // Reverse alphabetical on purpose: a renderer that sorted would produce
    // the opposite order and this would fail.
    const attributes = [
      stored("zeal", "Zeal", "1", "ZEA"),
      stored("might", "Might", "2", "MGT"),
      stored("agility", "Agility", "3", "AGI"),
    ];
    const markup = render([{ kind: "badgeGrid", of: "attributes" }], {
      attributes,
    });
    expect(idsInOrder(markup)).toEqual(["zeal", "might", "agility"]);
  });

  it("renders a rowList and a barStack in declaration order too", () => {
    const markup = render(
      [
        { kind: "rowList", of: "skills" },
        { kind: "barStack", of: "resources" },
      ],
      {
        skills: [
          stored("stealth", "Stealth", "2"),
          stored("athletics", "Athletics", "1"),
        ],
        resources: [
          stored("stress", "Stress", "1 / 9"),
          stored("harm", "Harm", "0 / 3"),
        ],
      },
    );
    expect(idsInOrder(markup)).toEqual([
      "stealth",
      "athletics",
      "stress",
      "harm",
    ]);
  });

  it("draws a bar only for a pool that has a maximum", () => {
    const markup = render([{ kind: "barStack", of: "resources" }], {
      resources: [
        pool("stress", "Stress", 1, 9),
        // A counter: no maximum to be a proportion of, so no bar. Blades in
        // the Dark's coin counts up with nothing to fill.
        pool("coin", "Coin", 2, null),
      ],
    });
    expect(markup.match(/role="meter"/g)).toHaveLength(1);
  });

  /**
   * The regression T019a exists to prevent.
   *
   * The bar used to be recovered by parsing the rendered string, so a system
   * whose value read "4 of 7" — or "4 out of 7", or any wording but the one
   * shape the parser knew — silently lost its bar, with nothing failing
   * anywhere. The numbers now arrive as numbers and the string is only read.
   */
  it("draws a bar from the numbers even when the text is unparseable", () => {
    const markup = render([{ kind: "barStack", of: "resources" }], {
      resources: [pool("health", "Health", 4, 7, "4 of 7")],
    });

    expect(markup).toContain('role="meter"');
    expect(markup).toContain('aria-valuenow="4"');
    expect(markup).toContain('aria-valuemax="7"');
  });

  /**
   * And the converse: a string that *looks* like a fraction is not one. If
   * anything ever starts parsing again, this goes red.
   */
  it("draws no bar for a value that merely looks like a fraction", () => {
    const markup = render([{ kind: "barStack", of: "resources" }], {
      resources: [stored("initiative", "Initiative", "2 / 3")],
    });
    expect(markup).not.toContain('role="meter"');
  });
});

/**
 * The point of this whole block: a track and a pool can carry the same two
 * numbers and are not the same thing. A pool is a quantity with a maximum and
 * the numbers are the point; a track is a set of marks and the count is the
 * point. Draw one as the other and a player who expected boxes to tick gets a
 * bar they cannot touch.
 */
describe("a track is a run of marks, and a pool is a bar", () => {
  const layout: LayoutDeclaration = [{ kind: "value", id: "stress" }];

  it("renders a track as `filled` of `of` marks, none of them a bar", () => {
    const markup = render(layout, {
      resources: [track("stress", "Stress", 3, 8)],
    });
    expect(markup.match(/data-slot="track-mark"/g)).toHaveLength(8);
    expect(markup.match(/aria-checked="true"/g)).toHaveLength(3);
    expect(markup).toContain('data-track-filled="3"');
    expect(markup).toContain('data-track-of="8"');
    // Not a bar, and not a text box either: the marks are the control.
    expect(markup).not.toContain('role="meter"');
    expect(markup).not.toContain("<input");
  });

  it("renders a pool with the same numbers as a bar, and no marks", () => {
    const markup = render(layout, {
      resources: [pool("stress", "Stress", 3, 8)],
    });
    expect(markup).toContain('role="meter"');
    expect(markup).toContain('aria-valuenow="3"');
    expect(markup).toContain('aria-valuemax="8"');
    expect(markup).not.toContain('data-slot="track-mark"');
  });

  it("gives a stored track marks that can be clicked and a derived one none", () => {
    const editable = render(layout, {
      resources: [track("stress", "Stress", 1, 4)],
    });
    expect(editable.match(/<button/g)).toHaveLength(4);

    const readonly = render(layout, {
      resources: [track("stress", "Stress", 1, 4, "derived")],
    });
    expect(readonly).not.toContain("<button");
    expect(readonly.match(/data-slot="track-mark"/g)).toHaveLength(4);
  });

  it("draws a track from its numbers, not from its text", () => {
    // The T019a failure, restated for marks: the string says something else
    // entirely and the marks are still right.
    const odd = {
      ...track("stress", "Stress", 2, 5),
      value: "two boxes out of five",
    };
    const markup = render(layout, { resources: [odd] });
    expect(markup.match(/data-slot="track-mark"/g)).toHaveLength(5);
    expect(markup.match(/aria-checked="true"/g)).toHaveLength(2);
  });
});

describe("a state set is a ladder with a rung marked", () => {
  const layout: LayoutDeclaration = [{ kind: "value", id: "damage" }];
  const RUNGS = ["hale", "impaired", "debilitated", "dead"];

  it("renders every option the system declares, in the system's order", () => {
    const markup = render(layout, {
      resources: [ladder("damage", "Damage Track", "impaired", RUNGS)],
    });
    for (const rung of RUNGS) {
      expect(markup).toContain(`data-state-option="${rung}"`);
    }
    expect(markup.indexOf("hale")).toBeLessThan(markup.indexOf("dead"));
    // Exactly one rung is current.
    expect(markup.match(/data-current="true"/g)).toHaveLength(1);
    expect(markup).toContain('data-state-current="impaired"');
    // A ladder has no marks and no bar.
    expect(markup).not.toContain('data-slot="track-mark"');
    expect(markup).not.toContain('role="meter"');
  });

  it("marks no rung when the character is on none of them", () => {
    // A real answer, not a missing one: an uninjured character is at no
    // position on a damage track.
    const markup = render(layout, {
      resources: [ladder("damage", "Damage Track", null, RUNGS)],
    });
    expect(markup.match(/data-state-option=/g)).toHaveLength(4);
    expect(markup).not.toContain('data-current="true"');
    expect(markup).not.toContain("data-state-unknown");
  });

  it("tells a rung named with an empty string apart from no rung at all", () => {
    // T019i. `stateReading` folded `""` into null, so a system that named a
    // rung with an empty string read as a character standing on none of them.
    // The flattened `value` string renders both as empty — which is exactly
    // why the structured `state` field exists, and reading the string's
    // ambiguity back into it gave the string the last word after all.
    const markup = render(layout, {
      resources: [ladder("damage", "Damage Track", "", ["", "impaired"])],
    });
    // The empty rung is current, and it is a *known* rung: it is declared.
    expect(markup).toContain('data-current="true"');
    expect(markup).not.toContain("data-state-unknown");

    // Contrast, on the same declaration: null really is no rung.
    const none = render(layout, {
      resources: [ladder("damage", "Damage Track", null, ["", "impaired"])],
    });
    expect(none).not.toContain('data-current="true"');
  });

  /**
   * The failure this prevents is specific and quiet: a saved character whose
   * condition was renamed reading as the *first* option — which on a damage
   * track means silently healed.
   */
  it("renders a stored state that is not among its options as unknown", () => {
    const markup = render(layout, {
      resources: [ladder("damage", "Damage Track", "maimed", RUNGS)],
    });
    expect(markup).toContain('data-state-unknown="true"');
    expect(markup).toContain('data-slot="state-unknown"');
    expect(markup).toContain("maimed");
    // And emphatically not "hale".
    expect(markup).not.toContain('data-current="true"');
  });

  it("lets a player move a stored ladder but not a derived one", () => {
    const editable = render(layout, {
      resources: [ladder("damage", "Damage Track", "hale", RUNGS)],
    });
    expect(editable.match(/<button/g)).toHaveLength(4);

    const readonly = render(layout, {
      resources: [ladder("damage", "Damage Track", "hale", RUNGS, "derived")],
    });
    expect(readonly).not.toContain("<button");
    expect(readonly.match(/aria-readonly="true"/g)).toHaveLength(4);
  });
});

describe("`other` catches everything the named sets did not claim", () => {
  const layout: LayoutDeclaration = [{ kind: "rowList", of: "other" }];

  it("renders a value that belongs to no named set", () => {
    // FR-034 / SC-012: a value missing from a sheet is indistinguishable
    // from the character not having it.
    const markup = render(layout, {
      attributes: [stored("might", "Might", "3", "MGT")],
      all: [
        stored("might", "Might", "3", "MGT"),
        stored("heritage", "Heritage", "Djinn-touched"),
      ],
    });
    expect(markup).toContain("Heritage");
    expect(markup).toContain("Djinn-touched");
    // The claimed one is not repeated into `other`.
    expect(markup).not.toContain("MGT");
  });

  it("renders nothing when every value is claimed", () => {
    expect(
      render(layout, {
        attributes: [stored("might", "Might", "3", "MGT")],
        all: [stored("might", "Might", "3", "MGT")],
      }),
    ).toBe("");
  });

  it("renders nothing when the caller passes no full set at all", () => {
    // Honest rather than an error: nothing was published, so nothing went
    // unclaimed.
    expect(
      render(layout, { attributes: [stored("might", "Might", "3", "MGT")] }),
    ).toBe("");
  });

  it("keeps whatever kind an unclaimed value is", () => {
    const markup = render(layout, {
      all: [track("scars", "Scars", 2, 4)],
    });
    expect(markup.match(/data-slot="track-mark"/g)).toHaveLength(4);
  });
});

describe("values sharing a group render as one thing", () => {
  /**
   * A Fate consequence is a severity *and* the aspect written into it. Two
   * unrelated rows would be the renderer contradicting the system (FR-033).
   */
  const CONSEQUENCE: SheetValue[] = [
    grouped(stored("mildSeverity", "Severity", "2"), "mildConsequence"),
    grouped(
      stored("mildAspect", "Aspect", "Sprained ankle"),
      "mildConsequence",
    ),
  ];

  it("puts both halves inside one group frame", () => {
    const markup = render([{ kind: "rowList", of: "other" }], {
      all: CONSEQUENCE,
    });
    expect(markup.match(/data-slot="value-group"/g)).toHaveLength(1);
    expect(markup).toContain('data-group="mildConsequence"');
    // One row, not two: the group is the unit the list holds.
    expect(markup.match(/data-slot="row"/g)).toHaveLength(1);
    expect(markup).toContain("Sprained ankle");
  });

  it("names a group as the system named it, not after its first member", () => {
    // T019g. The frame's accessible name was `unit.values[0].label`, so a
    // Cypher stat group read "Might" only because `might` happens to be
    // declared before `mightEdge`. Reorder the manifest and the same group
    // reads "Might Edge" — a rendering decision made by declaration order,
    // which is not a thing declaration order should decide.
    const markup = render([{ kind: "rowList", of: "other" }], {
      all: [
        {
          ...grouped(stored("mightEdge", "Might Edge", "1"), "might"),
          groupLabel: "Might",
        },
        {
          ...grouped(stored("mightPool", "Might Pool", "12"), "might"),
          groupLabel: "Might",
          headline: true,
        },
      ],
    });
    // The group *frame*'s own name — the inner value lines carry their own
    // labels, so this has to name the frame rather than search the document.
    expect(markup).toContain('role="group" aria-label="Might"');
    expect(markup).not.toContain('role="group" aria-label="Might Edge"');
    // And the headline is the member the system named, declared second.
    expect(markup).toContain('data-group-headline="mightPool"');
  });

  it("falls back to the first member when the system named neither", () => {
    // The old behaviour, kept — a system may group values purely to keep them
    // together and have nothing to add about the grouping. Now an explicit
    // fallback rather than an unstated assumption.
    const markup = render([{ kind: "rowList", of: "other" }], {
      all: CONSEQUENCE,
    });
    expect(markup).toContain('role="group" aria-label="Severity"');
    expect(markup).toContain('data-group-headline="mildSeverity"');
  });

  it("keeps declaration order within the group", () => {
    const markup = render([{ kind: "rowList", of: "other" }], {
      all: CONSEQUENCE,
    });
    expect(idsInOrder(markup)).toEqual(["mildSeverity", "mildAspect"]);
  });

  it("gathers members the system did not declare adjacently, without reordering the set", () => {
    // A Cypher stat is a current value, a pool and an edge; the group takes
    // the position of its first member and pulls the rest up to it.
    const markup = render([{ kind: "rowList", of: "other" }], {
      all: [
        grouped(stored("mightCurrent", "Might", "9"), "might"),
        stored("speedPool", "Speed Pool", "12"),
        grouped(stored("mightEdge", "Might Edge", "1"), "might"),
      ],
    });
    expect(idsInOrder(markup)).toEqual([
      "mightCurrent",
      "mightEdge",
      "speedPool",
    ]);
    expect(markup.match(/data-slot="value-group"/g)).toHaveLength(1);
  });

  it("leaves an ungrouped value in no frame at all", () => {
    const markup = render([{ kind: "rowList", of: "other" }], {
      all: [stored("heritage", "Heritage", "Djinn-touched")],
    });
    expect(markup).not.toContain('data-slot="value-group"');
  });
});

describe("specific constructs", () => {
  const declarations: Partial<SheetDeclarations> = {
    attributes: [stored("strength", "Strength", "16", "STR")],
    derived: [derived("strengthMod", "Strength Modifier", "+3", "MOD")],
    resources: [
      track("deathSaves", "Death Save Successes", 2, 3),
      stored("notes", "Notes", "Owes the smith a favour"),
    ],
  };

  it("renders a value by identifier", () => {
    const markup = render([{ kind: "value", id: "strength" }], declarations);
    expect(markup).toContain('data-slot="layout-value"');
    expect(markup).toContain("Strength");
    expect(markup).toContain('value="16"');
  });

  it("renders a pair side by side", () => {
    const markup = render(
      [{ kind: "pair", value: "strength", beside: "strengthMod" }],
      declarations,
    );
    expect(markup).toContain('data-slot="layout-pair"');
    expect(markup).toContain("STR");
    expect(markup).toContain("MOD");
    // The score is typed in; the modifier is not.
    expect(markup.match(/<input/g)).toHaveLength(1);
  });

  it("gives a block the same value with room to breathe", () => {
    const markup = render([{ kind: "block", id: "notes" }], declarations);
    expect(markup).toContain('data-slot="layout-block"');
    expect(markup).toContain("<textarea");
    expect(markup).toContain("Owes the smith a favour");
  });

  it("renders a block of a track as marks, not as prose", () => {
    // A block says how much room, never what the value is.
    const markup = render([{ kind: "block", id: "deathSaves" }], declarations);
    expect(markup.match(/data-slot="track-mark"/g)).toHaveLength(3);
    expect(markup).not.toContain("<textarea");
  });

  it("renders nothing for an identifier the system does not declare", () => {
    expect(render([{ kind: "value", id: "ki" }], declarations)).toBe("");
    expect(render([{ kind: "block", id: "ki" }], declarations)).toBe("");
    // And a section holding only unresolvable references is not a heading
    // over blank space either.
    expect(
      render(
        [
          {
            kind: "section",
            title: "Spellcasting",
            children: [{ kind: "value", id: "kiPoints" }],
          },
        ],
        declarations,
      ),
    ).toBe("");
  });
});

describe("a node kind this build does not know", () => {
  it("is skipped without taking the sheet down with it", () => {
    resetUnknownKindWarnings();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const layout = [
      { kind: "diceTray", of: "attributes" },
      { kind: "badgeGrid", of: "attributes" },
    ] as unknown as LayoutNode[];

    const markup = render(layout, {
      attributes: [stored("might", "Might", "3", "MGT")],
    });

    expect(markup).toContain("MGT");
    expect(warn).toHaveBeenCalledWith(
      "[layout] ignoring unknown node kind: diceTray",
    );
    warn.mockRestore();
  });

  it("is skipped when it is a node kind that was removed", () => {
    // `tracker` and `slotGrid` were real kinds and are not any more. A pack
    // still carrying one loses that node and keeps the rest of the sheet.
    resetUnknownKindWarnings();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
    const markup = render(
      [
        { kind: "tracker", id: "deathSaves", boxes: 3 },
        { kind: "value", id: "might" },
      ] as unknown as LayoutNode[],
      { attributes: [stored("might", "Might", "3", "MGT")] },
    );
    expect(markup).toContain('data-value-id="might"');
    expect(markup).not.toContain('data-slot="tracker"');
    warn.mockRestore();
  });
});

describe("the declared sets and `all` can disagree, and now say so (T019h)", () => {
  /**
   * Six lists arrive and nothing checked they agreed. `other` is computed as
   * the part of `all` the named sets did not claim, so `all` being incomplete
   * does not break the named sets — it quietly loses the values that are in
   * *no* named set, which is FR-035's failure exactly: a value missing from a
   * sheet is indistinguishable from the character not having it.
   */
  it("names the ids a set claims that `all` has never heard of", () => {
    expect(
      declarationsDrift({
        attributes: [stored("might", "Might", "3")],
        all: [stored("cunning", "Cunning", "2")],
      }),
    ).toEqual(["might"]);
  });

  it("is silent when the sets and `all` agree", () => {
    const might = stored("might", "Might", "3");
    expect(declarationsDrift({ attributes: [might], all: [might] })).toEqual(
      [],
    );
  });

  it("is silent when a caller supplies no `all` at all", () => {
    // The documented old behaviour: nothing was published, so nothing went
    // unclaimed and there is nothing for the two to disagree about. Reporting
    // here would make every existing caller noisy for doing nothing wrong.
    expect(
      declarationsDrift({ attributes: [stored("might", "Might", "3")] }),
    ).toEqual([]);
  });

  it("warns once rather than once per render", () => {
    resetDeclarationDriftWarnings();
    const warn = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const drifting = {
      attributes: [stored("might", "Might", "3")],
      all: [stored("cunning", "Cunning", "2")],
    };
    declarationsFrom(drifting);
    declarationsFrom(drifting);
    declarationsFrom(drifting);
    expect(warn).toHaveBeenCalledTimes(1);
    warn.mockRestore();
  });
});

describe("a section that declares itself collapsed actually collapses (T019b)", () => {
  const body: LayoutNode[] = [{ kind: "badgeGrid", of: "attributes" }];

  it("renders a closed disclosure a reader can open", () => {
    // The bug: `collapsed` reached the DOM as `data-collapsed` and collapsed
    // nothing, so the format admitted a field that did not work and a pack
    // author had no way to find that out except by looking.
    const markup = render(
      [
        {
          kind: "section",
          title: "Spellcasting",
          collapsed: true,
          children: body,
        },
      ],
      GENIE,
    );
    expect(markup).toContain("<details");
    expect(markup).toContain("<summary");
    expect(markup).toContain("Spellcasting");
    // Closed: no `open` attribute. The content is still in the document, so
    // nothing is lost and find-in-page still reaches it.
    expect(markup).not.toContain("<details open");
    expect(markup).toContain("Might");
  });

  it("leaves a section that did not ask to collapse exactly as it was", () => {
    const markup = render(
      [{ kind: "section", title: "Attributes", children: body }],
      GENIE,
    );
    expect(markup).not.toContain("<details");
    expect(markup).toContain("<h3");
    expect(markup).not.toContain("data-collapsed");
  });

  it("ignores `collapsed` on a section with no title", () => {
    // There would be nothing to click. Honouring it would render a section a
    // reader cannot open, which is worse than not honouring it.
    const markup = render(
      [{ kind: "section", collapsed: true, children: body }],
      GENIE,
    );
    expect(markup).not.toContain("<details");
    expect(markup).toContain("Might");
  });
});
