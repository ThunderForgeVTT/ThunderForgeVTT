import { useEffect, useMemo, useState } from "react";

import { getWorldAbilities } from "@/api/abilities";
import { getActorAbilities } from "@/api/actorAbilities";
import { useActorSystemData } from "@/hooks/useActorSystemData";
import type { WorldAbilityRecord } from "@/types/ability";
import type { ActorAbilityEntryRecord } from "@/types/actorAbility";
import type { WorldActorRecord } from "@/types/actor";

import { abilityRolls, type CharacterRoll } from "./characterRolls";
import { loadStatRolls, rollsFor, type PackStatRolls } from "./systemStatRolls";

/**
 * A character's rolls: its numeric stats, then its abilities' formulas.
 *
 * Two reads because a formula lives on the ability, not on the actor's entry
 * for it. A failure on either leaves the ability rolls absent rather than
 * taking the sheet down: the stats still roll, which is most of why the
 * player opened it. Shared by the dock's sheet and the sheet page (spec 081).
 *
 * The stats come from the actor's pack where it exports its own `statRolls`
 * (`systemStatRolls.ts`), and from the generic reading where it does not.
 * Until the pack's index has loaded no stat is offered, so a pack whose
 * numbers are scores never flashes them as modifiers.
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
  const systemId = actor.gameSystemId ?? null;
  const [pack, setPack] = useState<{
    systemId: string | null;
    statRolls: PackStatRolls | null;
  } | null>(null);

  useEffect(() => {
    let active = true;
    loadStatRolls(systemId)
      .then((statRolls) => {
        if (active) setPack({ systemId, statRolls });
      })
      .catch(() => {
        // A pack index that failed to load offers no stats rather than the
        // generic reading, which may be the very misreading the pack exists
        // to prevent.
        if (active) setPack({ systemId, statRolls: () => [] });
      });
    return () => {
      active = false;
    };
  }, [systemId]);

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

  const packReady = pack !== null && pack.systemId === systemId;
  return useMemo(
    () => [
      ...(packReady
        ? rollsFor(pack.statRolls, data as Record<string, unknown> | null)
        : []),
      ...abilityRolls(entries, catalog),
    ],
    [packReady, pack, data, entries, catalog],
  );
}
