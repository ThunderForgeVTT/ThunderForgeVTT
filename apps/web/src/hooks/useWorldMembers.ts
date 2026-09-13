/**
 * useWorldMembers.ts
 * React hook for fetching a world's membership roster.
 *
 * Usage:
 *   const { members, loading, error, refetch } = useWorldMembers(worldId);
 *
 * RxDB has been hard cut from this layer entirely: it was fatally broken
 * app-wide (RxDB 17.2.0 rejects the inline `index: true` schema shorthand
 * used throughout, error SC26 — crashing `getWorldDatabase()` for every
 * collection), and there was never a live GraphQL subscription transport
 * wired up client-side to make its "reactive" local-cache queries actually
 * reflect server-side pushes anyway (at best they mirrored this browser
 * tab's own optimistic writes). This hook now fetches directly via
 * GraphQL on mount and whenever `worldId` changes, mirroring the
 * established fallback pattern in `useActorSystemData.ts`. There is no
 * live push transport, so members will NOT update in this view when
 * another user joins/leaves/changes role elsewhere — call `refetch()` (or
 * remount) to pick up changes.
 */

import { useCallback, useEffect, useState } from "react";
import { useResetOnChange } from "@/hooks/useResetOnChange";
import { getWorldMembers } from "@/api/worldMembers";
import type { WorldMemberDoc } from "../db/collections/worldMembersCollection";
import { sortMembersByRole } from "../db/collections/worldMembersCollection";
import { isWorldMemberRole } from "@/types/world";

export interface UseWorldMembersResult {
  members: WorldMemberDoc[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export function useWorldMembers(worldId: string): UseWorldMembersResult {
  const [members, setMembers] = useState<WorldMemberDoc[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  // Deliberately writes no state: the mount/worldId effect below has to be
  // able to call it without a synchronous setState
  // (react-hooks/set-state-in-effect), and both callers then share one
  // mapping/sorting path.
  const fetchMembers = useCallback(async (): Promise<WorldMemberDoc[]> => {
    const records = await getWorldMembers(worldId);

    // A role this build does not recognise is left out rather than guessed
    // at, as the server reads it as nobody. Narrowing in one step keeps the
    // type honest without a cast.
    const docs: WorldMemberDoc[] = records.flatMap((record) => {
      const role = record.role;
      if (!isWorldMemberRole(role)) return [];
      return [
        {
          id: record.id,
          world_id: record.worldId ?? worldId,
          user_id: record.userId,
          role,
          joined_at: record.joinedAt,
          created_at: record.createdAt ?? record.joinedAt,
          updated_at: record.updatedAt ?? record.joinedAt,
          claimed_actor: record.claimedActor,
        },
      ];
    });

    // Sort by role hierarchy, highest first
    return sortMembersByRole(docs);
  }, [worldId]);

  // A different world starts over: loading again, and no error carried over
  // from the previous one. Done during render (see useResetOnChange) rather
  // than at the top of the effect below.
  useResetOnChange(worldId, () => {
    setLoading(true);
    setError(null);
  });

  useEffect(() => {
    let active = true;
    fetchMembers()
      .then((docs) => {
        if (active) {
          setMembers(docs);
          setError(null);
        }
      })
      .catch((err) => {
        if (active) {
          setError(err instanceof Error ? err : new Error(String(err)));
          setMembers([]);
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false);
        }
      });
    // The `active` guard is new with this restructure and fixes a real race:
    // before it, a slow response for a previous `worldId` could land after a
    // faster one for the current world and overwrite it.
    return () => {
      active = false;
    };
  }, [fetchMembers]);

  const refetch = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      setMembers(await fetchMembers());
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
      setMembers([]);
    } finally {
      setLoading(false);
    }
  }, [fetchMembers]);

  return { members, loading, error, refetch };
}
