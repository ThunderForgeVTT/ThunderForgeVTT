/**
 * The catalogue is append-only (spec 044 FR-038, ADR-106).
 *
 * Since phase (d) the spec that drew an actor's image is stored beside it, in
 * the database and in every collection copy and personal export made from
 * it. A choice renamed or removed here would turn every stored spec that uses
 * it into one `validateHero` refuses, and the builder would open those
 * actors with a problem instead of a face.
 *
 * `shipped.json` records every field (and its kind) and every choice ever
 * shipped. Only names are recorded, so a choice may change how it draws. A new
 * field or choice is welcome; append it to the record in the same change, so
 * that removing it later is caught too.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { HERO_COLORS, HERO_FLAGS, HERO_PARTS, SIZES } from "./index.ts";

type Kind = "text" | "choice" | "colour" | "flag";
interface Shipped {
  fields: Record<string, Kind>;
  choices: Record<string, string[]>;
}

const SHIPPED: Shipped = JSON.parse(
  readFileSync(new URL("./shipped.json", import.meta.url), "utf8"),
);

function currentFields(): Record<string, Kind> {
  const fields: Record<string, Kind> = {
    name: "text",
    title: "text",
    size: "choice",
  };
  for (const part of Object.keys(HERO_PARTS)) fields[part] = "choice";
  for (const color of HERO_COLORS) fields[color] = "colour";
  for (const flag of HERO_FLAGS) fields[flag] = "flag";
  return fields;
}

function currentChoices(): Record<string, readonly string[]> {
  return { size: SIZES, ...HERO_PARTS };
}

/** Every recorded field or choice the catalogue no longer has as recorded. */
function retired(
  shipped: Shipped,
  fields: Record<string, Kind>,
  choices: Record<string, readonly string[]>,
): string[] {
  const problems: string[] = [];
  for (const [field, kind] of Object.entries(shipped.fields)) {
    if (fields[field] === undefined) problems.push(`${field}: removed`);
    else if (fields[field] !== kind) {
      problems.push(`${field}: was a ${kind}, now a ${fields[field]}`);
    }
  }
  for (const [field, list] of Object.entries(shipped.choices)) {
    for (const choice of list) {
      if (!(choices[field] ?? []).includes(choice)) {
        problems.push(`${field}.${choice}: removed`);
      }
    }
  }
  return problems;
}

/** Every current field or choice the record has not been told about. */
function unrecorded(
  shipped: Shipped,
  fields: Record<string, Kind>,
  choices: Record<string, readonly string[]>,
): string[] {
  const missing = Object.keys(fields).filter((f) => !(f in shipped.fields));
  for (const [field, list] of Object.entries(choices)) {
    for (const choice of list) {
      if (!(shipped.choices[field] ?? []).includes(choice)) {
        missing.push(`${field}.${choice}`);
      }
    }
  }
  return missing;
}

describe("the catalogue is append-only", () => {
  it("still has every field and choice ever shipped", () => {
    assert.deepEqual(
      retired(SHIPPED, currentFields(), currentChoices()),
      [],
      "a stored hero spec may use these; restore them (FR-038)",
    );
  });

  it("has recorded every field and choice it ships", () => {
    assert.deepEqual(
      unrecorded(SHIPPED, currentFields(), currentChoices()),
      [],
      "append these to packages/heroes/src/shipped.json",
    );
  });

  it("catches a renamed choice, a removed field and a field that changed kind", () => {
    const { tusks: _, ...fields } = currentFields();
    const choices = {
      ...currentChoices(),
      headgear: HERO_PARTS.headgear.map((c) => (c === "wizard" ? "mage" : c)),
    };
    assert.deepEqual(
      retired(SHIPPED, { ...fields, beard: "choice" }, choices).sort(),
      [
        "beard: was a flag, now a choice",
        "headgear.wizard: removed",
        "tusks: removed",
      ],
    );
  });

  it("allows a new choice and a new field", () => {
    const choices = {
      ...currentChoices(),
      prop: [...HERO_PARTS.prop, "lantern"],
    };
    const fields: Record<string, Kind> = {
      ...currentFields(),
      cloak: "choice",
    };
    assert.deepEqual(retired(SHIPPED, fields, choices), []);
  });
});
