/**
 * Spec 084 (FR-019): 5e's roll facets, mirrored for the demo's own formulas
 * from `packs/systems/dnd5e/server/src/roll_facets.rs`. The server rewrites
 * the formula's first d20 that keeps nothing; this does the same to the
 * demo's `1d20 + MODIFIER` checks and its attacks' to-hit formulas, and the
 * dice crate built for the page rolls what comes out.
 */
import { GraphQLError } from "graphql";

/** How a d20 test is rolled. The GraphQL `Advantage` enum. */
export type Advantage = "NORMAL" | "ADVANTAGE" | "DISADVANTAGE";

/** `RollKind`, as the demo stores it on a roll record. */
export type RollKind = "check" | "to_hit" | "damage";

export const NO_D20 = "This roll has no d20 to roll twice.";
export const NO_DAMAGE_ADVANTAGE =
  "A damage roll is never rolled with advantage.";

/** 5e's `LABELS`: what a facet is called where a roll shows it. */
export const FACET_LABELS: Record<string, string> = {
  advantage: "Advantage",
  disadvantage: "Disadvantage",
  halfling_luck: "Halfling Luck",
  inspiration: "Heroic Inspiration",
  lucky: "Lucky",
  luck_point: "Luck Point",
  great_weapon_fighting: "Great Weapon Fighting",
};

/** The formula to roll and the facets that shaped it. */
export interface Shaped {
  formula: string;
  facets: string[];
}

const KEEP = { ADVANTAGE: "kh1", DISADVANTAGE: "kl1" } as const;
const FACET = { ADVANTAGE: "advantage", DISADVANTAGE: "disadvantage" } as const;

/** A dice term: count, `d`, sides, then its modifiers (`kh1`, `r1`, …). */
const TERM = /(\d*)d(\d+)((?:[a-z]+\d*)*)/gi;

/** Whether a sheet's `trait_data.facets` lists `id` (5e's `has_facet`). */
export function hasFacet(traitData: unknown, id: string): boolean {
  const facets = (traitData as { facets?: unknown } | null | undefined)?.facets;
  return Array.isArray(facets) && facets.includes(id);
}

/**
 * `shape_d20`: the first d20 that keeps nothing takes the choice, and a
 * halfling's sheet rerolls its natural 1s once (`r1`), unless the term
 * already rerolls.
 */
export function shapeD20(
  formula: string,
  advantage: Advantage,
  traitData?: unknown,
): Shaped {
  const lucky = hasFacet(traitData, "halfling_luck");
  if (advantage === "NORMAL" && !lucky) return { formula, facets: [] };
  let found = false;
  const shaped = formula.replace(
    TERM,
    (term, count: string, sides: string, modifiers: string) => {
      if (found || sides !== "20" || /[kd]/i.test(modifiers)) return term;
      found = true;
      const written = count === "" ? 1 : Number(count);
      const dice = advantage === "NORMAL" ? written : Math.max(written, 2);
      const reroll = lucky && !/r/i.test(modifiers) ? "r1" : "";
      const keep = advantage === "NORMAL" ? "" : KEEP[advantage];
      return `${dice}d20${modifiers}${reroll}${keep}`;
    },
  );
  if (!found) {
    if (advantage !== "NORMAL") throw new GraphQLError(NO_D20);
    return { formula, facets: [] };
  }
  const facets: string[] = advantage === "NORMAL" ? [] : [FACET[advantage]];
  if (lucky) facets.push("halfling_luck");
  return { formula: shaped, facets };
}

/** A damage roll is rolled as declared, and never with advantage. */
export function shapeDamage(formula: string, advantage: Advantage): Shaped {
  if (advantage !== "NORMAL") throw new GraphQLError(NO_DAMAGE_ADVANTAGE);
  return { formula, facets: [] };
}

/** `RollFacet` rows, for a roll's `facets` field. */
export function facetRows(
  ids: readonly string[],
): { id: string; label: string }[] {
  return ids.map((id) => ({ id, label: FACET_LABELS[id] ?? id }));
}
