import type { WorldAbilityRecord } from "@/types/ability";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";

/**
 * What a character can roll, derived from what the character already has.
 *
 * # Why this is a list of formulas and not a roller
 *
 * Spec 031 FR-003 asks that a roll from the in-pane view reach the table
 * identically to one made anywhere else. The only way to keep that promise is
 * for this module to decide *what string to send* and for `rollDice` to decide
 * everything else — the numbers, the keeps, the total. A helper here that
 * added dice together would be a second roll path producing the same answers
 * on a good day and a divergence on a bad one, which is exactly the failure
 * FR-003 exists to prevent.
 *
 * # Why the stat list is read generically rather than per system
 *
 * `ability_data` is a system-owned JSONB blob: genie keeps might/cunning/
 * spirit there, another pack keeps whatever it keeps. Naming genie's three
 * here would mean this file needs editing every time a pack ships, which is
 * the coupling `systemActorSheets.ts` was written to avoid. So every numeric
 * member is offered and non-numeric members (genie's `trained_skills`, say)
 * are ignored — a system that stores something unrollable simply contributes
 * no button.
 */
export interface CharacterRoll {
  /** Stable within a list; used for React keys and `data-testid`s. */
  key: string;
  label: string;
  /** Sent verbatim to `rollDice`. Never evaluated here. */
  formula: string;
}

/** `might` -> `Might`, `wisdom_save` -> `Wisdom save`. */
function humanise(key: string): string {
  const spaced = key.replace(/[_-]+/g, " ").trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/**
 * The d20-plus-modifier check every system in this app expresses the same
 * way. `1d20` is the base because that is what the existing dice roller
 * defaults to; a system that checks differently will contribute its own roll
 * surface when packs can (spec 032), and until then this is the honest floor
 * rather than a guess dressed up as a rule.
 */
export function statRolls(
  abilityData: Record<string, unknown> | null | undefined,
): CharacterRoll[] {
  if (!abilityData) {
    return [];
  }
  return Object.entries(abilityData)
    .filter(
      (entry): entry is [string, number] =>
        typeof entry[1] === "number" && Number.isFinite(entry[1]),
    )
    .map(([name, score]) => ({
      key: `stat-${name}`,
      label: humanise(name),
      // A zero modifier is written as a bare `1d20` rather than `1d20+0`:
      // the formula is shown to the player, and `+0` reads as a bug.
      formula:
        score === 0 ? "1d20" : score > 0 ? `1d20+${score}` : `1d20${score}`,
    }));
}

const EFFECT_LABELS: Record<string, string> = {
  ATTACK_ROLL: "attack",
  DAMAGE: "damage",
  HEAL: "healing",
  MODIFIER: "modifier",
};

/**
 * One roll per effect of each ability the character knows, resolved against
 * the world's ability catalogue.
 *
 * The catalogue lookup is what makes GM-only abilities a non-question here:
 * the server already omits them from both the actor's entries and the
 * catalogue for a non-DM (spec 025 FR-024b), so an entry with no matching
 * record contributes nothing and this file does no visibility filtering of
 * its own — it must not start doing any.
 *
 * A tombstoned entry (`abilityId === null`, the ability was deleted) is
 * skipped for the same reason: there is no formula left to send.
 */
export function abilityRolls(
  entries: ActorAbilityEntryRecord[] | null | undefined,
  catalog: WorldAbilityRecord[] | null | undefined,
): CharacterRoll[] {
  if (!entries || !catalog) {
    return [];
  }
  const byId = new Map(catalog.map((ability) => [ability.id, ability]));
  return entries.flatMap((entry) => {
    const ability = entry.abilityId ? byId.get(entry.abilityId) : undefined;
    if (!ability) {
      return [];
    }
    return ability.effects
      .filter((effect) => effect.formula.trim() !== "")
      .map((effect) => ({
        key: `ability-${entry.id}-${effect.id}`,
        label: `${entry.abilityName} (${
          EFFECT_LABELS[effect.effectType] ?? effect.effectType.toLowerCase()
        })`,
        formula: effect.formula,
      }));
  });
}
