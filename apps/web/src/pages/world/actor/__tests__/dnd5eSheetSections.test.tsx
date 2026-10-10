import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";

import ClassesSection from "../../../../../../../packs/systems/dnd5e/web/src/sheet/ClassesSection.tsx";
import CoinsSection from "../../../../../../../packs/systems/dnd5e/web/src/sheet/CoinsSection.tsx";
import DefencesSection from "../../../../../../../packs/systems/dnd5e/web/src/sheet/DefencesSection.tsx";
import LinkedContent from "../../../../../../../packs/systems/dnd5e/web/src/sheet/LinkedContent.tsx";
import PersonaSection from "../../../../../../../packs/systems/dnd5e/web/src/sheet/PersonaSection.tsx";
import {
  CONDITIONS,
  DAMAGE_TYPES,
  classesPatch,
  classesProblem,
  readClasses,
  readCoins,
  readDefences,
  readPersona,
  type ClassEntry,
} from "../../../../../../../packs/systems/dnd5e/web/src/sheet/data.ts";

/**
 * Spec 048 T028: the 5e sheet's sections for what an imported character
 * brings. What each shows, and what each writes.
 */

const PACK = "../../../../../../../packs/systems/dnd5e";
const noop = () => {};
const here = path.dirname(fileURLToPath(import.meta.url));
const manifest = JSON.parse(
  readFileSync(path.resolve(here, PACK, "system.json"), "utf8"),
) as {
  damageTypes: { id: string }[];
  conditions: { id: string }[];
};

const fighterWizard: ClassEntry[] = [
  { name: "Fighter", subclass: "Champion", level: 3, hitDie: "d10" },
  { name: "Wizard", subclass: "", level: 2, hitDie: "d6" },
];

describe("the sheet's tables match the manifest", () => {
  it("names the damage types and conditions system.json declares", () => {
    expect([...DAMAGE_TYPES]).toEqual(manifest.damageTypes.map((t) => t.id));
    expect([...CONDITIONS]).toEqual(manifest.conditions.map((c) => c.id));
  });
});

describe("classes", () => {
  it("reads what is stored and skips what it cannot read", () => {
    const read = readClasses({
      classes: [
        { name: "Fighter", subclass: "Champion", level: 3, hit_die: "d10" },
        { name: "Wizard", level: 2, hit_die: "d6" },
        { name: "", level: 1, hit_die: "d8" },
        { name: "Bard", level: 1, hit_die: "d4" },
        "Rogue",
      ],
    });
    expect(read).toEqual(fighterWizard);
    expect(readClasses({})).toEqual([]);
  });

  it("writes level as the sum and class as the first", () => {
    expect(classesPatch(fighterWizard)).toEqual({
      classes: [
        { name: "Fighter", subclass: "Champion", level: 3, hit_die: "d10" },
        { name: "Wizard", level: 2, hit_die: "d6" },
      ],
      class: "Fighter",
      level: 5,
    });
    expect(classesPatch([])).toEqual({ classes: [] });
  });

  it("will not save a nameless class or more than 20 levels", () => {
    expect(classesProblem(fighterWizard)).toBeNull();
    expect(classesProblem([{ ...fighterWizard[0], name: " " }])).toMatch(
      /name/,
    );
    expect(
      classesProblem([
        { ...fighterWizard[0], level: 15 },
        { ...fighterWizard[1], level: 6 },
      ]),
    ).toMatch(/21/);
  });

  it("lists each class for a reader, and offers rows to an editor", () => {
    const read = renderToStaticMarkup(
      <ClassesSection
        classes={fighterWizard}
        canEdit={false}
        disabled={false}
        onSave={noop}
      />,
    );
    expect(read).toContain("Fighter 3");
    expect(read).toContain("(Champion)");
    expect(read).toContain("Wizard 2");
    expect(read).not.toContain("<input");

    const edit = renderToStaticMarkup(
      <ClassesSection
        classes={fighterWizard}
        canEdit
        disabled={false}
        onSave={noop}
      />,
    );
    expect(edit).toContain('data-testid="dnd5e-class-1"');
    expect(edit).toContain('data-testid="dnd5e-class-add"');
    expect(edit).toMatch(/disabled="" data-testid="dnd5e-classes-save"/);
  });
});

describe("defences", () => {
  it("keeps only declared ids, in the manifest's order", () => {
    const read = readDefences({
      resistances: ["fire", "psychic", "cold", "sonic"],
      condition_immunities: ["poisoned", "exhaustion"],
    });
    expect(read.resistances).toEqual(["cold", "fire", "psychic"]);
    expect(read.immunities).toEqual([]);
    expect(read.condition_immunities).toEqual(["exhaustion", "poisoned"]);
  });

  it("shows a reader only the lists that have something in them", () => {
    const html = renderToStaticMarkup(
      <DefencesSection
        defences={readDefences({ resistances: ["poison"] })}
        canEdit={false}
        disabled={false}
        onSave={noop}
      />,
    );
    expect(html).toContain("Poison");
    expect(html).toContain('data-testid="dnd5e-resistances"');
    expect(html).not.toContain('data-testid="dnd5e-immunities"');
    expect(html).not.toContain("<select");
  });

  it("lets an editor remove a chosen id and add one not yet chosen", () => {
    const html = renderToStaticMarkup(
      <DefencesSection
        defences={readDefences({ resistances: ["poison"] })}
        canEdit
        disabled={false}
        onSave={noop}
      />,
    );
    expect(html).toContain('aria-label="Remove Poison from Resistances"');
    expect(html).toContain('aria-label="Add to Condition immunities"');
    const resistanceChoices = html
      .split('aria-label="Add to Resistances"')[1]
      .split("</select>")[0];
    expect(resistanceChoices).not.toContain('value="poison"');
    expect(resistanceChoices).toContain('value="fire"');
  });
});

describe("coins", () => {
  it("reads whole, non-negative counts and zero for the rest", () => {
    expect(readCoins({ coins: { gp: 1215, sp: 40, cp: -3, ep: 1.5 } })).toEqual(
      { pp: 0, gp: 1215, ep: 0, sp: 40, cp: 0 },
    );
    expect(readCoins({})).toEqual({ pp: 0, gp: 0, ep: 0, sp: 0, cp: 0 });
  });

  it("shows each coin, editable or not", () => {
    const coins = readCoins({ coins: { gp: 12 } });
    const read = renderToStaticMarkup(
      <CoinsSection coins={coins} canEdit={false} disabled onSave={noop} />,
    );
    expect(read).toContain('aria-label="Gold: 12"');
    expect(read).not.toContain("<input");
    const edit = renderToStaticMarkup(
      <CoinsSection coins={coins} canEdit disabled={false} onSave={noop} />,
    );
    for (const coin of ["pp", "gp", "ep", "sp", "cp"]) {
      expect(edit).toContain(`data-testid="dnd5e-coins-${coin}-input"`);
    }
    expect(edit).toMatch(/data-testid="dnd5e-coins-gp-input" value="12"/);
  });
});

describe("persona", () => {
  it("reads the strings and leaves the rest blank", () => {
    const persona = readPersona({
      eyes: "Grey",
      backstory: "Born at sea.",
      age: 7,
    });
    expect(persona.eyes).toBe("Grey");
    expect(persona.backstory).toBe("Born at sea.");
    expect(persona.age).toBe("");
  });

  it("shows a reader only what was written", () => {
    const html = renderToStaticMarkup(
      <PersonaSection
        persona={readPersona({ eyes: "Grey", bonds: "My ship." })}
        canEdit={false}
        disabled={false}
        onSave={noop}
      />,
    );
    expect(html).toContain("Grey");
    expect(html).toContain("My ship.");
    expect(html).not.toContain("Hair");
    expect(html).not.toContain("<textarea");
    const empty = renderToStaticMarkup(
      <PersonaSection
        persona={readPersona({})}
        canEdit={false}
        disabled={false}
        onSave={noop}
      />,
    );
    expect(empty).toContain("Nothing written yet.");
  });

  it("gives an editor every field, bounded as the validator bounds it", () => {
    const html = renderToStaticMarkup(
      <PersonaSection
        persona={readPersona({})}
        canEdit
        disabled={false}
        onSave={noop}
      />,
    );
    expect(html).toMatch(/maxLength="100"[^>]*data-testid="dnd5e-hair-input"/);
    expect(html).toMatch(
      /<textarea[^>]*maxLength="8000"[^>]*data-testid="dnd5e-backstory-input"/,
    );
  });
});

describe("linked content", () => {
  it("groups what is linked and marks what the GM has not adopted", () => {
    const html = renderToStaticMarkup(
      <LinkedContent
        entries={[
          { id: "a", kind: "spell", name: "Shield" },
          { id: "b", kind: "spell", name: "Magic Missile", staged: "pending" },
          { id: "c", kind: "feature", name: "Second Wind" },
          { id: "d", kind: "item", name: "Longsword", staged: "declined" },
        ]}
      />,
    );
    expect(html).toContain("Spells");
    expect(html).toContain("Features");
    expect(html).toContain("Items");
    expect(html.indexOf("Magic Missile")).toBeLessThan(html.indexOf("Shield"));
    expect(html).toMatch(
      /data-testid="dnd5e-linked-b" data-staged="pending"[^>]*>.*awaiting the GM/,
    );
    expect(html).toMatch(
      /data-testid="dnd5e-linked-d" data-staged="declined"[^>]*>.*declined by the GM/,
    );
    expect(html).toMatch(/data-testid="dnd5e-linked-a" data-staged="no"[^>]*>/);
  });

  it("draws nothing when nothing is linked", () => {
    expect(renderToStaticMarkup(<LinkedContent entries={[]} />)).toBe("");
  });
});
