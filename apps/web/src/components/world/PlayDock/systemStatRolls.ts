import { loadPackIndex } from "@/lib/systemPackIndexes";

import { statRolls, type CharacterRoll } from "./characterRolls";

/**
 * A character's stat rolls, from its pack where the pack knows them.
 *
 * The generic `statRolls` reads every number in `ability_data` as a d20
 * modifier. That is right for a system that stores modifiers and wrong for
 * one that stores scores, or keeps a defence beside them, and only the pack
 * can tell which. So a pack may export `statRolls` from its web index; it is
 * handed every system data slot, since a save needs proficiency and level as
 * well as a score. A pack without one keeps the generic reading.
 */
export type PackStatRolls = (systemData: Record<string, unknown>) => unknown;

/** The system data slots, as `useActorSystemData` returns them. */
type SystemData = Record<string, unknown>;

function isRoll(value: unknown): value is CharacterRoll {
  if (!value || typeof value !== "object") return false;
  const roll = value as Record<string, unknown>;
  return (
    typeof roll.key === "string" &&
    typeof roll.label === "string" &&
    typeof roll.formula === "string"
  );
}

/**
 * The stat rolls for a character's data: the pack's where it has them, the
 * generic reading where it does not. A pack's answer is checked for shape,
 * and a pack that throws contributes nothing rather than a wrong guess.
 */
export function rollsFor(
  packStatRolls: PackStatRolls | null,
  systemData: SystemData | null | undefined,
): CharacterRoll[] {
  if (!packStatRolls) {
    return statRolls(
      systemData?.ability_data as Record<string, unknown> | null | undefined,
    );
  }
  try {
    const answer = packStatRolls(systemData ?? {});
    return Array.isArray(answer)
      ? answer
          .filter(isRoll)
          .map(({ key, label, formula }) => ({ key, label, formula }))
      : [];
  } catch {
    return [];
  }
}

/** A system's own stat rolls, or `null` where its pack exports none. */
export async function loadStatRolls(
  gameSystemId: string | null | undefined,
): Promise<PackStatRolls | null> {
  const index = await loadPackIndex(gameSystemId);
  return index?.statRolls ?? null;
}
