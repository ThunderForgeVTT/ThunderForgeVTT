import { useEffect, useMemo, useState } from "react";

import { getWorldAbilities } from "@/api/abilities";
import { getActorAbilities } from "@/api/actorAbilities";
import { useActorSystemData } from "@/hooks/useActorSystemData";
import type { WorldAbilityRecord } from "@/types/ability";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";
import type { WorldActorRecord } from "@/types/actor";

import { abilityRolls, statRolls, type CharacterRoll } from "./characterRolls";

/**
 * A character's rolls: its numeric stats, then its abilities' formulas.
 *
 * Two reads because a formula lives on the ability, not on the actor's entry
 * for it. A failure on either leaves the ability rolls absent rather than
 * taking the sheet down: the stats still roll, which is most of why the
 * player opened it. Shared by the dock's sheet and the sheet page (spec 081).
 */
export function useCharacterRolls(
  worldId: string,
  actor: WorldActorRecord,
): CharacterRoll[] {
  const { data } = useActorSystemData(
    actor.id,
    actor.gameSystemId ?? undefined,
  );
  const [entries, setEntries] = useState<ActorAbilityEntryRecord[] | null>(
    null,
  );
  const [catalog, setCatalog] = useState<WorldAbilityRecord[] | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([getActorAbilities(actor.id), getWorldAbilities(worldId)])
      .then(([actorAbilities, worldAbilities]) => {
        if (active) {
          setEntries(actorAbilities);
          setCatalog(worldAbilities);
        }
      })
      .catch(() => {
        if (active) {
          setEntries([]);
          setCatalog([]);
        }
      });
    return () => {
      active = false;
    };
  }, [actor.id, worldId]);

  return useMemo(
    () => [...statRolls(data?.ability_data), ...abilityRolls(entries, catalog)],
    [data?.ability_data, entries, catalog],
  );
}
