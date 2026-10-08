/**
 * Spec 084 (FR-019): 5e's roll facets, mirrored for the demo's own formulas
 * from `packs/systems/dnd5e/server/src/roll_facets.rs`. The server rewrites
 * the formula's first d20 that keeps nothing; this does the same to the
 * demo's `1d20 + MODIFIER` checks and its attacks' to-hit formulas, and the
 * dice crate built for the page rolls what comes out.
 */
import { GraphQLError } from "graphql";

import {
  calculateProficiencyBonus,
  calculateProficiencyBonusForChallenge,
} from "../../../../../packs/systems/dnd5e/web/src/derived-data";

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

/** What the pack's `NOT_A_D20_TEST` says. */
export const NOT_A_D20_TEST = "Only a d20 test can be rerolled.";

/** The roll a reroll is planned against. */
export interface RerollTarget {
  kind: unknown;
  formula: string;
  facets: readonly string[];
}

/**
 * How the pack's `RerollEdit` says to roll again: the lowest d20 once more
 * (Heroic Inspiration), or the whole roll through a new formula (a Luck
 * Point).
 */
export type RerollEdit =
  | { kind: "lowest"; sides: 20 }
  | { kind: "reshape"; formula: string };

/** A planned reroll: the sheet's trait data once spent, and the edit. */
export interface RerollPlan {
  traitData: Record<string, unknown>;
  edit: RerollEdit;
}

/** The spends the pack knows, in the order it offers them. */
const REROLLS = ["inspiration", "luck_point"] as const;

/** The kinds a spend may be offered on: the d20 tests. */
const OFFERED_ON: readonly RollKind[] = ["check", "to_hit"];

/** `proficiency_of`: from the level, or else the challenge rating. */
function proficiencyOf(traits: Record<string, unknown>): number {
  const level = traits.level;
  if (typeof level === "number" && Number.isInteger(level)) {
    return level >= 1 && level <= 20 ? calculateProficiencyBonus(level) : 0;
  }
  const challenge = traits.challenge;
  return typeof challenge === "string"
    ? (calculateProficiencyBonusForChallenge(challenge) ?? 0)
    : 0;
}

/** `spend_inspiration`. */
function spendInspiration(
  traits: Record<string, unknown>,
  target: RerollTarget,
  actorName: string,
): RerollPlan {
  if (target.kind === "damage") throw new GraphQLError(NOT_A_D20_TEST);
  if (traits.inspiration !== true) {
    throw new GraphQLError(`${actorName} has no Heroic Inspiration.`);
  }
  return {
    traitData: { ...traits, inspiration: false },
    edit: { kind: "lowest", sides: 20 },
  };
}

/**
 * `spend_luck_point`: one more d20 in the first d20 term, the highest kept,
 * and one more of the proficiency bonus's points used.
 */
function spendLuckPoint(
  traits: Record<string, unknown>,
  target: RerollTarget,
  actorName: string,
): RerollPlan {
  if (target.kind === "damage") throw new GraphQLError(NOT_A_D20_TEST);
  if (!hasFacet(traits, "lucky")) {
    throw new GraphQLError(`${actorName} does not have the Lucky feat.`);
  }
  if (target.facets.includes("disadvantage")) {
    throw new GraphQLError(
      "A Luck Point does nothing on a roll made with disadvantage.",
    );
  }
  const used =
    typeof traits.luck_points_used === "number" ? traits.luck_points_used : 0;
  if (used >= proficiencyOf(traits)) {
    throw new GraphQLError(`${actorName} has no Luck Points left.`);
  }
  let found = false;
  const formula = target.formula.replace(
    TERM,
    (term, count: string, sides: string, modifiers: string) => {
      if (found || sides !== "20") return term;
      found = true;
      const dice = (count === "" ? 1 : Number(count)) + 1;
      const keep = /[kd]/i.test(modifiers) ? "" : "kh1";
      return `${dice}d20${modifiers}${keep}`;
    },
  );
  if (!found) throw new GraphQLError(NOT_A_D20_TEST);
  return {
    traitData: { ...traits, luck_points_used: used + 1 },
    edit: { kind: "reshape", formula },
  };
}

/**
 * The 5e pack's `reroll`: the plan for spending `spend` on `target`, or the
 * pack's refusal. `spentAlready` is every facet spent along the roll's chain
 * (the server's own check, made before the pack is asked).
 */
export function rerollPlan(
  traitData: unknown,
  spend: string,
  actorName: string,
  spentAlready: readonly string[],
  target: RerollTarget,
): RerollPlan {
  const traits = { ...((traitData ?? {}) as Record<string, unknown>) };
  if (!(REROLLS as readonly string[]).includes(spend)) {
    throw new GraphQLError(`This system has no reroll called "${spend}".`);
  }
  if (spentAlready.includes(spend)) {
    const label = FACET_LABELS[spend] ?? spend;
    throw new GraphQLError(`${label} has already been spent on this roll.`);
  }
  return spend === "inspiration"
    ? spendInspiration(traits, target, actorName)
    : spendLuckPoint(traits, target, actorName);
}

/**
 * `offers_for`: every spend whose plan the pack would accept, so the button
 * the table is offered and the refusal it would get cannot disagree.
 */
export function rerollOffers(
  traitData: unknown,
  target: RerollTarget,
  spentAlready: readonly string[],
): string[] {
  if (!OFFERED_ON.includes(target.kind as RollKind)) return [];
  return REROLLS.filter((spend) => {
    try {
      rerollPlan(traitData, spend, "", spentAlready, target);
      return true;
    } catch {
      return false;
    }
  });
}
