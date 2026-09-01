/**
 * apps/web/src/hooks/useActorSystemData.ts
 * Query hook for Actor System Data
 *
 * RxDB hard-cut (unreleased project, leaning into Bevy as the real sync
 * mechanism instead — see project constitution Principle I): this used to
 * query an RxDB local collection with a live `.subscribe()`. That
 * collection never had replication wired up (see the deleted
 * db/collections/worldActorSystemDataCollection.ts replication code), so
 * the "real-time subscription" only ever reflected this tab's own
 * optimistic writes, never another client's. Now this hook fetches
 * directly via GraphQL (`@/api/actorSystemData`) on mount and on
 * actorId/gameSystemId change, and exposes `refetch` for callers (like
 * `useUpdateActorData`) to force a fresh read after a mutation.
 *
 * Usage:
 * ```tsx
 * const { data, loading, error } = useActorSystemData(actorId, 'dnd5e');
 *
 * if (loading) return <div>Loading...</div>;
 * if (error) return <div>Error: {error.message}</div>;
 *
 * return (
 *   <div>
 *     STR: {data.ability_data?.strength ?? 10}
 *   </div>
 * );
 * ```
 */

import { useEffect, useState, useCallback } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { fetchActorSystemData } from "@/api/actorSystemData";

function fromGraphQLRecord(
  record: NonNullable<Awaited<ReturnType<typeof fetchActorSystemData>>>,
): ActorSystemData {
  return {
    id: record.id,
    actor_id: record.actorId,
    game_system_id: record.gameSystemId,
    ability_data: record.abilityData ?? undefined,
    resource_data: record.resourceData ?? undefined,
    proficiency_data: record.proficiencyData ?? undefined,
    trait_data: record.traitData ?? undefined,
    spell_data: record.spellData ?? undefined,
    created_by: "",
    updated_by: "",
    created_at: record.createdAt,
    updated_at: record.updatedAt,
  };
}

/**
 * Actor system data as read from the server.
 */
export interface ActorSystemData {
  id: string;
  actor_id: string;
  game_system_id: string;

  // System-specific JSONB columns. `unknown` values, not `any`: what these
  // hold is decided by whichever game system the actor belongs to (dnd5e's
  // `strength` and Pathfinder's `strength_mod` live in the same column), so
  // a reader that wants a number has to check for one. See lib/systemData.ts.
  ability_data?: Record<string, unknown>;
  resource_data?: Record<string, unknown>;
  proficiency_data?: Record<string, unknown>;
  trait_data?: Record<string, unknown>;
  spell_data?: Record<string, unknown>;

  // Metadata
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;

  // Optimistic update metadata (set locally by useUpdateActorData while a
  // mutation is in flight; never sent to or read back from the server)
  _optimistic?: boolean;
  _lastServerData?: Record<string, unknown>;
}

/**
 * Hook return value
 */
export interface UseActorSystemDataResult {
  /** Current actor system data (null while loading, or if none exists) */
  data: ActorSystemData | null;

  /** True while a query is executing */
  loading: boolean;

  /** Error message if query failed (null if success) */
  error: Error | null;

  /** Manual refetch trigger */
  refetch: () => Promise<void>;
}

/**
 * Fetch an actor's system data directly via GraphQL.
 *
 * @param actorId - Actor ID to query (required)
 * @param gameSystemId - Game system ID to filter by (optional, e.g., "dnd5e")
 * @returns { data, loading, error, refetch }
 */
export function useActorSystemData(
  actorId: string,
  gameSystemId?: string,
): UseActorSystemDataResult {
  const [data, setData] = useState<ActorSystemData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  // Deliberately writes no state: the mount/args effect below has to be able
  // to call it without a synchronous setState
  // (react-hooks/set-state-in-effect).
  const query = useCallback(async (): Promise<ActorSystemData | null> => {
    // No id, no question to ask.
    //
    // Callers reach this hook from routes and panels where the actor id
    // arrives a render or two after mount, so `actorId` is briefly "". Asking
    // the server about an empty id is not merely wasteful: `UUID!` refuses to
    // parse it, so every one of those renders logged
    // `Failed to parse "UUID": invalid length: found 0` to the console. It was
    // constant background noise in the app and in every end-to-end run, and
    // noise is where a real error goes to hide.
    //
    // Returning null rather than throwing keeps "not asked yet" and "asked and
    // got nothing" the same shape to the caller, which is what they already
    // handle.
    if (!actorId) {
      return null;
    }

    const remote = await fetchActorSystemData(actorId);
    if (!remote || (gameSystemId && remote.gameSystemId !== gameSystemId)) {
      return null;
    }
    return fromGraphQLRecord(remote);
  }, [actorId, gameSystemId]);

  // Different actor/system: loading again, no stale error. Done during render
  // (see useResetOnChange) rather than at the top of the effect below.
  useResetOnChange(`${actorId}|${gameSystemId ?? ""}`, () => {
    setLoading(true);
    setError(null);
  });

  useEffect(() => {
    let active = true;
    query()
      .then((result) => {
        if (active) {
          setData(result);
          setError(null);
          setLoading(false);
        }
      })
      .catch((err) => {
        const error = err instanceof Error ? err : new Error(String(err));
        if (active) {
          setError(error);
          setData(null);
          setLoading(false);
        }
        console.error("[useActorSystemData] Query failed:", error);
      });
    // The `active` guard is new with this restructure and fixes a real race:
    // before it, a slow response for a previously-requested actor could land
    // after a faster one for the current actor and overwrite it.
    return () => {
      active = false;
    };
  }, [query]);

  const refetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setData(await query());
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      setData(null);
      console.error("[useActorSystemData] Query failed:", error);
    } finally {
      setLoading(false);
    }
  }, [query]);

  return { data, loading, error, refetch };
}

/**
 * Convenience hook for accessing only a specific data type
 *
 * Usage:
 * ```tsx
 * const abilities = useActorAbilityData(actorId, 'dnd5e');
 * return <div>STR: {abilities?.strength ?? 10}</div>;
 * ```
 */
export function useActorAbilityData(
  actorId: string,
  gameSystemId: string,
): Record<string, unknown> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.ability_data ?? null;
}

/**
 * Convenience hook for accessing proficiency data
 */
export function useActorProficiencyData(
  actorId: string,
  gameSystemId: string,
): Record<string, unknown> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.proficiency_data ?? null;
}

/**
 * Convenience hook for accessing resource data
 */
export function useActorResourceData(
  actorId: string,
  gameSystemId: string,
): Record<string, unknown> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.resource_data ?? null;
}

/**
 * Convenience hook for accessing trait data
 */
export function useActorTraitData(
  actorId: string,
  gameSystemId: string,
): Record<string, unknown> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.trait_data ?? null;
}

/**
 * Convenience hook for accessing spell data
 */
export function useActorSpellData(
  actorId: string,
  gameSystemId: string,
): Record<string, unknown> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.spell_data ?? null;
}
