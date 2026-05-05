/**
 * apps/web/src/hooks/useUpdateActorData.ts
 * GraphQL Mutation Hook for Actor System Data
 *
 * Phase 4.8.1: E2.2 - Optimistic Updates & Mutations
 *
 * Hook to update actor system data with optimistic updates and rollback.
 * Handles the full mutation lifecycle:
 * 1. Optimistic: Update RxDB immediately
 * 2. Save original: Store pre-mutation values
 * 3. Send: Send GraphQL mutation to server
 * 4. Confirm: Server validates and persists
 * 5. Rollback: Restore original values on rejection
 *
 * Usage:
 * ```tsx
 * const { mutate, isPending } = useUpdateActorData(actorId, 'dnd5e');
 *
 * await mutate('ability_data', { strength: 16, dexterity: 14 });
 * ```
 */

import { useCallback, useState } from "react";
import { getWorldDatabase } from "@/engine/world/sync/database";
import { ActorSystemData } from "./useActorSystemData";

/**
 * Mutation hook result
 */
export interface UseUpdateActorDataResult {
  /** Trigger mutation. Throws on validation error. */
  mutate: (dataType: string, data: Record<string, any>) => Promise<void>;

  /** True while mutation is in flight */
  isPending: boolean;

  /** Last error from mutation */
  error: Error | null;
}

/**
 * GraphQL mutation query to update actor system data
 * This will be sent to the server for validation and persistence
 */
const UPDATE_ACTOR_DATA_MUTATION = `
  mutation UpdateActorSystemData($input: UpdateActorSystemDataInput!) {
    updateActorSystemData(input: $input) {
      success
      message
      data {
        id
        actor_id
        game_system_id
        ability_data
        resource_data
        proficiency_data
        trait_data
        spell_data
        updated_by
        updated_at
      }
    }
  }
`;

/**
 * Update actor system data with optimistic updates and rollback
 *
 * @param actorId - Actor ID to update (required)
 * @param gameSystemId - Game system ID (required, e.g., "dnd5e")
 * @returns { mutate, isPending, error }
 *
 * Handles:
 * ✅ Optimistic update to RxDB
 * ✅ Save pre-mutation state for rollback
 * ✅ Send GraphQL mutation to server
 * ✅ Rollback on server rejection
 * ✅ Loading state during mutation
 * ✅ Error handling and logging
 */
export function useUpdateActorData(
  actorId: string,
  gameSystemId: string,
): UseUpdateActorDataResult {
  const [isPending, setIsPending] = useState(false);
  const [error, setError] = useState<Error | null>(null);

  const mutate = useCallback(
    async (dataType: string, data: Record<string, any>) => {
      try {
        setIsPending(true);
        setError(null);

        const db = await getWorldDatabase();

        // 1️⃣ Get current data for rollback
        const currentQuery = db.collections.world_actor_system_data
          .find()
          .where("actor_id")
          .eq(actorId);

        const currentResults = await currentQuery.exec();
        const currentDoc = currentResults.find((d) => d.game_system_id === gameSystemId);

        if (!currentDoc) {
          throw new Error(`No actor system data found for ${actorId} in ${gameSystemId}`);
        }

        const original = { ...currentDoc } as ActorSystemData;
        const originalDataType = currentDoc[dataType as keyof ActorSystemData];

        // 2️⃣ Optimistic update: Update RxDB immediately
        const optimisticUpdate: Partial<ActorSystemData> = {
          [dataType]: data,
          _optimistic: true,
          _lastServerData: {
            [dataType]: originalDataType,
          },
        };

        await db.collections.world_actor_system_data.upsert({
          ...currentDoc,
          ...optimisticUpdate,
        });

        // 3️⃣ Send GraphQL mutation to server
        const response = await fetch("/graphql", {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            query: UPDATE_ACTOR_DATA_MUTATION,
            variables: {
              input: {
                actor_id: actorId,
                game_system_id: gameSystemId,
                [dataType]: data,
              },
            },
          }),
        });

        const responseJson = await response.json();

        if (!response.ok || responseJson.errors) {
          const errorMessage =
            responseJson.errors?.[0]?.message || `HTTP ${response.status}`;
          throw new Error(`Server rejected mutation: ${errorMessage}`);
        }

        // 4️⃣ Server confirmed! Remove optimistic flag
        // RxDB subscription will receive the canonical server data via pg_notify
        const finalUpdate: Partial<ActorSystemData> = {
          _optimistic: false,
          _lastServerData: undefined,
        };

        await db.collections.world_actor_system_data.upsert({
          ...currentDoc,
          ...optimisticUpdate,
          ...finalUpdate,
        });

        setIsPending(false);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));

        // 5️⃣ Rollback: Restore original values
        try {
          const db = await getWorldDatabase();

          // Query current doc to get latest
          const currentQuery = db.collections.world_actor_system_data
            .find()
            .where("actor_id")
            .eq(actorId);

          const currentResults = await currentQuery.exec();
          const currentDoc = currentResults.find((d) => d.game_system_id === gameSystemId);

          if (currentDoc) {
            // Restore original data type, remove optimistic flags
            await db.collections.world_actor_system_data.upsert({
              ...currentDoc,
              [dataType]: originalDataType,
              _optimistic: false,
              _lastServerData: undefined,
            });
          }
        } catch (rollbackErr) {
          console.error("[useUpdateActorData] Rollback failed:", rollbackErr);
        }

        setError(error);
        setIsPending(false);
        throw error;
      }
    },
    [actorId, gameSystemId],
  );

  return { mutate, isPending, error };
}

/**
 * Convenience hook for updating ability data specifically
 */
export function useUpdateAbilityData(actorId: string, gameSystemId: string) {
  const { mutate, isPending, error } = useUpdateActorData(actorId, gameSystemId);

  return {
    updateAbilities: async (abilities: Record<string, number>) =>
      mutate("ability_data", abilities),
    isPending,
    error,
  };
}

/**
 * Convenience hook for updating proficiency data specifically
 */
export function useUpdateProficiencyData(actorId: string, gameSystemId: string) {
  const { mutate, isPending, error } = useUpdateActorData(actorId, gameSystemId);

  return {
    updateProficiencies: async (proficiencies: Record<string, boolean>) =>
      mutate("proficiency_data", proficiencies),
    isPending,
    error,
  };
}

/**
 * Convenience hook for updating resource data specifically
 */
export function useUpdateResourceData(actorId: string, gameSystemId: string) {
  const { mutate, isPending, error } = useUpdateActorData(actorId, gameSystemId);

  return {
    updateResources: async (resources: Record<string, any>) =>
      mutate("resource_data", resources),
    isPending,
    error,
  };
}

/**
 * Convenience hook for updating trait data specifically
 */
export function useUpdateTraitData(actorId: string, gameSystemId: string) {
  const { mutate, isPending, error } = useUpdateActorData(actorId, gameSystemId);

  return {
    updateTraits: async (traits: Record<string, any>) =>
      mutate("trait_data", traits),
    isPending,
    error,
  };
}

/**
 * Convenience hook for updating spell data specifically
 */
export function useUpdateSpellData(actorId: string, gameSystemId: string) {
  const { mutate, isPending, error } = useUpdateActorData(actorId, gameSystemId);

  return {
    updateSpells: async (spells: Record<string, any>) =>
      mutate("spell_data", spells),
    isPending,
    error,
  };
}
