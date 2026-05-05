/**
 * apps/web/src/hooks/useActorSystemData.ts
 * RxDB Query Hook for Actor System Data
 *
 * Phase 4.8.1: E2.1 - RxDB Integration
 *
 * Hook to query and subscribe to actor system data from RxDB.
 * Automatically handles subscription lifecycle and provides:
 * - Real-time updates as data changes
 * - Loading state during initial query
 * - Error handling for failed queries
 * - Auto-cleanup on unmount
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
import { getWorldDatabase } from "@/engine/world/sync/database";

/**
 * Actor system data as stored in RxDB
 * Matches worldActorSystemDataSchema from database.ts
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

  // Optimistic update metadata
  _optimistic?: boolean;
  _lastServerData?: Record<string, any>;
}

/**
 * Hook return value
 */
export interface UseActorSystemDataResult {
  /** Current actor system data (null while loading) */
  data: ActorSystemData | null;

  /** True while initial query is executing */
  loading: boolean;

  /** Error message if query failed (null if success) */
  error: Error | null;

  /** Manual refetch trigger */
  refetch: () => Promise<void>;
}

/**
 * Query RxDB for actor system data with automatic subscription
 *
 * @param actorId - Actor ID to query (required)
 * @param gameSystemId - Game system ID to filter by (optional, e.g., "dnd5e")
 * @returns { data, loading, error, refetch }
 *
 * Handles:
 * ✅ Initial query from RxDB
 * ✅ Real-time subscription to changes
 * ✅ Automatic cleanup on unmount
 * ✅ Error handling and retry
 * ✅ Loading state management
 */
export function useActorSystemData(
  actorId: string,
  gameSystemId?: string,
): UseActorSystemDataResult {
  const [data, setData] = useState<ActorSystemData | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const refetch = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const db = await getWorldDatabase();

      // Build query: find by actor_id and optionally game_system_id
      let query = db.collections.world_actor_system_data.find().where("actor_id").eq(actorId);

      // Execute query
      const results = await query.exec();

      if (results.length === 0) {
        // No data found, clear state
        setData(null);
        setLoading(false);
        return;
      }

      // If gameSystemId is specified, filter results
      const filteredResult = gameSystemId
        ? results.find((r) => r.game_system_id === gameSystemId)
        : results[0];

      if (!filteredResult) {
        setData(null);
        setLoading(false);
        return;
      }

      setData(filteredResult as ActorSystemData);
      setLoading(false);
    } catch (err) {
      const error = err instanceof Error ? err : new Error(String(err));
      setError(error);
      setData(null);
      setLoading(false);
      console.error("[useActorSystemData] Query failed:", error);
    }
  }, [actorId, gameSystemId]);

  // Initial fetch and subscription setup
  useEffect(() => {
    let unsubscribe: (() => void) | null = null;

    (async () => {
      try {
        setLoading(true);
        setError(null);

        const db = await getWorldDatabase();

        // Initial fetch
        let query = db.collections.world_actor_system_data.find().where("actor_id").eq(actorId);

        const results = await query.exec();

        if (results.length === 0) {
          setData(null);
          setLoading(false);
          return;
        }

        // Filter by gameSystemId if provided
        const initialData = gameSystemId
          ? results.find((r) => r.game_system_id === gameSystemId)
          : results[0];

        if (!initialData) {
          setData(null);
          setLoading(false);
          return;
        }

        setData(initialData as ActorSystemData);
        setLoading(false);

        // Subscribe to changes
        // Build subscription query: watch for changes to this actor's data
        const subscription = db.collections.world_actor_system_data
          .find()
          .where("actor_id")
          .eq(actorId)
          .$.subscribe((docs: any[]) => {
            if (docs.length === 0) {
              setData(null);
              return;
            }

            // Update to the first matching doc, or filter by gameSystemId
            const updatedData = gameSystemId
              ? docs.find((d) => d.game_system_id === gameSystemId)
              : docs[0];

            if (updatedData) {
              setData(updatedData as ActorSystemData);
            }
          });

        unsubscribe = () => subscription.unsubscribe?.();
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
        setError(error);
        setData(null);
        setLoading(false);
        console.error("[useActorSystemData] Setup failed:", error);
      }
    })();

    // Cleanup subscription on unmount
    return () => {
      if (unsubscribe) {
        unsubscribe();
      }
    };
  }, [actorId, gameSystemId]);

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
