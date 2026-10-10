/**
 * The rolls a 5e character's stored numbers offer, for the host's play dock
 * and sheet page.
 *
 * `ability_data` holds *scores*, and Armor Class beside them. The host's
 * generic reading treats every number there as a d20 modifier, which offered
 * "Wisdom 1d20+18" and "Armor class 1d20+18". This pack knows better, so it
 * contributes its own list through `index.ts`:
 *
 * - an ability check per score entered, at the score's modifier;
 * - a saving throw per proficient ability, at modifier plus proficiency bonus;
 * - nothing for Armor Class, hit points, level, speed or any other number
 *   that is not something you roll.
 *
 * Saves without proficiency roll the same formula as the check, so only the
 * proficient ones are listed. The proficiency bonus is found the way the
 * sheet finds it (`ActorSheet.tsx`): the level, else the challenge rating,
 * else level 1, so the dock and the sheet agree on every number.
 *
 * Formulas only: `rollDice` on the server decides every number.
 */
import {
  calculateAbilityModifier,
  calculateProficiencyBonus,
  calculateProficiencyBonusForChallenge,
  calculateSavingThrow,
} from "./derived-data.ts";
import { DND5E_ABILITIES } from "./sheet-regions.ts";

/** The host's `CharacterRoll`, by shape. */
export interface StatRoll {
  key: string;
  label: string;
  formula: string;
}

type Json = Record<string, unknown>;

function record(value: unknown): Json {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Json)
    : {};
}

function score(value: unknown): number | null {
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

/** A list of ids (the sheet) or an object of flags (a sheet import). */
function proficientIds(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.filter((item): item is string => typeof item === "string");
  }
  return Object.entries(record(value))
    .filter(([, on]) => on === true)
    .map(([id]) => id);
}

function proficiencyBonus(traitData: Json): number {
  const level = score(traitData.level);
  if (level !== null) {
    return calculateProficiencyBonus(Math.min(20, Math.max(1, level)));
  }
  const challenge =
    typeof traitData.challenge === "string" ? traitData.challenge : "";
  return (
    calculateProficiencyBonusForChallenge(challenge) ??
    calculateProficiencyBonus(1)
  );
}

/** `1d20+4`, `1d20-1`, and a bare `1d20` rather than `1d20+0`. */
function d20(modifier: number): string {
  if (modifier === 0) return "1d20";
  return modifier > 0 ? `1d20+${modifier}` : `1d20${modifier}`;
}

export function statRolls(systemData: Json): StatRoll[] {
  const abilityData = record(systemData.ability_data);
  const modifiers = DND5E_ABILITIES.flatMap((ability) => {
    const value = score(abilityData[ability.id]);
    return value === null
      ? []
      : [{ ability, modifier: calculateAbilityModifier(value) }];
  });

  const checks = modifiers.map(({ ability, modifier }) => ({
    key: `stat-${ability.id}`,
    label: ability.label,
    formula: d20(modifier),
  }));

  const proficient = proficientIds(
    record(systemData.proficiency_data).saving_throw_proficiencies,
  );
  const bonus = proficiencyBonus(record(systemData.trait_data));
  const saves = modifiers
    .filter(({ ability }) => proficient.includes(ability.id))
    .map(({ ability, modifier }) => ({
      key: `save-${ability.id}`,
      label: `${ability.label} save`,
      formula: d20(calculateSavingThrow(modifier, true, bonus)),
    }));

  return [...checks, ...saves];
}
