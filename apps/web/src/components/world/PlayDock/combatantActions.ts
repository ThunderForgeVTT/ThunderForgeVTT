import type { TrackerAbilityRecord } from "@/api/attacks";
import type { CombatantRecord } from "@/types/combat";

/** Which of the tracker's two between-turns actions this is. */
export type CombatantActKind = "legendary" | "lair";

/**
 * What the Game Master may act with for this row: a creature with legendary
 * actions, or a lair. `null` for anything else.
 */
export function combatantActKind(
  combatant: CombatantRecord,
): CombatantActKind | null {
  if (combatant.kind === "LAIR") return "lair";
  if (combatant.tokenId && combatant.budget?.legendary) return "legendary";
  return null;
}

/**
 * The abilities offered (pure): those with an attack roll, and for a
 * legendary action the ones declared legendary first — all of them when none
 * is, since a Game Master may act with anything in their world.
 */
export function abilitiesFor(
  kind: CombatantActKind,
  abilities: TrackerAbilityRecord[],
): TrackerAbilityRecord[] {
  const attacks = abilities.filter((ability) =>
    ability.effects.some(
      (effect) =>
        effect.effectType === "ATTACK_ROLL" && effect.formula.trim() !== "",
    ),
  );
  if (kind === "lair") return attacks;
  const legendary = attacks.filter(
    (ability) => ability.actionCost === "LEGENDARY",
  );
  return legendary.length > 0 ? legendary : attacks;
}
