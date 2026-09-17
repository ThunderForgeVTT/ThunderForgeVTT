/**
 * What the controls are generated from. Nothing below names a part, a choice
 * or a colour field (contract B1): each list is read from the catalogue, so a
 * part added to `packages/heroes` appears here without an edit.
 */
import {
  HERO_COLORS,
  HERO_FLAGS,
  HERO_LABELS,
  HERO_PARTS,
  minimalSpec,
  SIZES,
  type HeroSpec,
  type ResolvedHero,
} from "@thunderforge/heroes";

export type HeroField = keyof ResolvedHero;

/** Parts and size: one group of mutually exclusive choices each. */
export const CHOICE_FIELDS: readonly {
  field: HeroField;
  choices: readonly string[];
}[] = [
  { field: "size", choices: SIZES },
  ...Object.entries(HERO_PARTS).map(([field, choices]) => ({
    field: field as HeroField,
    choices: choices as readonly string[],
  })),
];

export const COLOR_FIELDS: readonly HeroField[] = HERO_COLORS;
export const FLAG_FIELDS: readonly HeroField[] = HERO_FLAGS;

/** A field's label, or its key where the catalogue has none (FR-002). */
export function fieldLabel(field: string): string {
  return HERO_LABELS.fields[field] ?? field;
}

/** A choice's label, or its key where the catalogue has none (FR-002). */
export function choiceLabel(field: string, choice: string): string {
  return HERO_LABELS.choices[field]?.[choice] ?? choice;
}

/**
 * The fields a spec sets, as opposed to those following their default or
 * another field (FR-005). A field is set exactly when the smallest spec that
 * draws the same hero still carries it — never by a list of which fields
 * derive.
 */
export function setFields(spec: HeroSpec): ReadonlySet<string> {
  return new Set(Object.keys(minimalSpec(spec)));
}
