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

  // System-specific JSONB columns
  ability_data?: Record<string, any>;
  resource_data?: Record<string, any>;
  proficiency_data?: Record<string, any>;
  trait_data?: Record<string, any>;
  spell_data?: Record<string, any>;

  // Metadata
  created_by: string;
  updated_by: string;
  created_at: string;
  updated_at: string;

  // Optimistic update metadata (set locally by useUpdateActorData while a
  // mutation is in flight; never sent to or read back from the server)
  _optimistic?: boolean;
  _lastServerData?: Record<string, any>;
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

  const load = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const remote = await fetchActorSystemData(actorId);
      if (!remote || (gameSystemId && remote.gameSystemId !== gameSystemId)) {
        setData(null);
      } else {
        setData(fromGraphQLRecord(remote));
      }
      setLoading(false);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      setData(null);
      setLoading(false);
      console.error("[useActorSystemData] Query failed:", error);
    }
  }, [actorId, gameSystemId]);

  useEffect(() => {
    void load();
  }, [load]);

  return { data, loading, error, refetch: load };
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
): Record<string, any> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.ability_data ?? null;
}

/**
 * Convenience hook for accessing proficiency data
 */
export function useActorProficiencyData(
  actorId: string,
  gameSystemId: string,
): Record<string, any> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.proficiency_data ?? null;
}

/**
 * Convenience hook for accessing resource data
 */
export function useActorResourceData(
  actorId: string,
  gameSystemId: string,
): Record<string, any> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.resource_data ?? null;
}

/**
 * Convenience hook for accessing trait data
 */
export function useActorTraitData(
  actorId: string,
  gameSystemId: string,
): Record<string, any> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.trait_data ?? null;
}

/**
 * Convenience hook for accessing spell data
 */
export function useActorSpellData(
  actorId: string,
  gameSystemId: string,
): Record<string, any> | null {
  const { data } = useActorSystemData(actorId, gameSystemId);
  return data?.spell_data ?? null;
}
