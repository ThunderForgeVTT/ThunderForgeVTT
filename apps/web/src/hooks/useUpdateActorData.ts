/**
 * apps/web/src/hooks/useUpdateActorData.ts
 * GraphQL Mutation Hook for Actor System Data
 *
 * RxDB hard-cut (unreleased project, leaning into Bevy as the real sync
 * mechanism instead — see project constitution Principle I): this used to
 * write optimistically to an RxDB local collection, then send the GraphQL
 * mutation, then reconcile/rollback in RxDB. That local collection was
 * never actually read back from anywhere except this hook and
 * `useActorSystemData` (which now reads via GraphQL directly), so the
 * optimistic write bought nothing beyond in-memory React state. This hook
 * now sends the mutation straight to the server and lets the caller's
 * `useActorSystemData().refetch()` (or its own local UI state) reflect the
 * result — no local persistence layer involved.
 *
 * Usage:
 * ```tsx
 * const { mutate, isPending } = useUpdateActorData(actorId, 'dnd5e');
 *
 * await mutate('ability_data', { strength: 16, dexterity: 14 });
 * ```
 */

import { useCallback, useState } from "react";
import {
  updateActorSystemData,
  type ActorSystemDataType,
} from "@/api/actorSystemData";

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
 * Update actor system data.
 *
 * @param actorId - Actor ID to update (required)
 * @param gameSystemId - Game system ID (required, e.g., "dnd5e")
 * @returns { mutate, isPending, error }
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

        await updateActorSystemData(
          actorId,
          gameSystemId,
          dataType as ActorSystemDataType,
          data,
        );

        setIsPending(false);
      } catch (err) {
        const error = err instanceof Error ? err : new Error(String(err));
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
