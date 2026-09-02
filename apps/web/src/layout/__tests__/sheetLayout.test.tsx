import { describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import React from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { SheetLayout } from "../SheetLayout";
import { resetUnknownKindWarnings } from "../resolve";
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
 * two things not appearing — a heading over an empty set, and a text box over
 * a computed number.
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
 * `data-value-id` appears on both a row and the control inside it, so the
 * repeats are collapsed: what is under test is the order, which is the
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
    // One row list, not two: the movement one in the last section. The
    // skills one is not there at all.
    expect(markup.match(/data-slot="row-list"/g)).toHaveLength(1);
    // The other three sections are still there, so the absence above is
    // about the empty set and not about nothing having rendered.
    expect(markup).toContain("Attributes");
    expect(markup).toContain("Resources");
  });

  it("renders nothing at all for a system that declares nothing", () => {
    expect(render(FORGE.layout, {})).toBe("");
  });

  it("keeps a section whose children are only partly empty", () => {
    // Forge's last section pairs movement with derived values. Movement
    // alone is enough for the section to exist.
    const movementOnly = render(FORGE.layout, {
      movement: [stored("walk", "Walk", "30")],
    });
    expect(movementOnly).toContain("Movement and Derived Values");
    expect(movementOnly).toContain("Walk");
    expect(movementOnly).not.toContain("Attributes");
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

describe("specific constructs", () => {
  const declarations: Partial<SheetDeclarations> = {
    attributes: [stored("strength", "Strength", "16", "STR")],
    derived: [
      derived("strengthMod", "Strength Modifier", "+3", "MOD"),
      derived("deathSaves", "Death Save Successes", "2"),
    ],
    resources: [
      stored("spellSlots1", "1st-level slots", "4"),
      stored("spellSlots1Spent", "1st-level spent", "1"),
      stored("spellSlots2", "2nd-level slots", "3"),
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

  it("renders a tracker as a bounded run of boxes", () => {
    const markup = render(
      [{ kind: "tracker", id: "deathSaves", boxes: 3, rows: 2 }],
      declarations,
    );
    expect(markup.match(/data-slot="tracker-box"/g)).toHaveLength(6);
    // Two of six filled, and — being derived — none of them a control.
    expect(markup.match(/aria-checked="true"/g)).toHaveLength(2);
    expect(markup).not.toContain("<button");
  });

  it("gives a stored tracker boxes that can be clicked", () => {
    const markup = render([{ kind: "tracker", id: "used", boxes: 3 }], {
      resources: [stored("used", "Uses", "1")],
    });
    expect(markup.match(/<button/g)).toHaveLength(3);
  });

  it("renders a slotGrid as one row per level the system declares", () => {
    const markup = render(
      [{ kind: "slotGrid", id: "spellSlots", levels: 9 }],
      declarations,
    );
    // Nine levels asked for, two declared: a caster shows the slots they
    // have rather than six empty rows.
    expect(markup.match(/data-slot="slot-level"/g)).toHaveLength(2);
    expect(markup).toContain('data-level="1"');
    expect(markup).toContain('data-level="2"');
  });

  it("renders nothing for an identifier the system does not declare", () => {
    expect(render([{ kind: "value", id: "ki" }], declarations)).toBe("");
    expect(
      render([{ kind: "tracker", id: "ki", boxes: 3 }], declarations),
    ).toBe("");
    expect(
      render([{ kind: "slotGrid", id: "kiPoints", levels: 9 }], declarations),
    ).toBe("");
    // And a section holding only unresolvable references is not a heading
    // over blank space either.
    expect(
      render(
        [
          {
            kind: "section",
            title: "Spellcasting",
            children: [{ kind: "slotGrid", id: "kiPoints", levels: 9 }],
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
});
