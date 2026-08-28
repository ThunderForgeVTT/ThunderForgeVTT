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
import { getWorldMembers } from "@/api/worldMembers";
import type { WorldMemberDoc } from "../db/collections/worldMembersCollection";
import { sortMembersByRole } from "../db/collections/worldMembersCollection";

export interface UseWorldMembersResult {
  members: WorldMemberDoc[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

function isMemberRole(role: string): role is "Owner" | "GM" | "Player" {
  return role === "Owner" || role === "GM" || role === "Player";
}

export function useWorldMembers(worldId: string): UseWorldMembersResult {
  const [members, setMembers] = useState<WorldMemberDoc[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchMembers = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const records = await getWorldMembers(worldId);

      const docs: WorldMemberDoc[] = records
        .filter((record) => isMemberRole(record.role))
        .map((record) => ({
          id: record.id,
          world_id: record.worldId ?? worldId,
          user_id: record.userId,
          role: record.role as "Owner" | "GM" | "Player",
          joined_at: record.joinedAt,
          created_at: record.createdAt ?? record.joinedAt,
          updated_at: record.updatedAt ?? record.joinedAt,
          claimed_actor: record.claimedActor,
        }));

      // Sort by role hierarchy: Owner, GM, Player
      setMembers(sortMembersByRole(docs));
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
      setMembers([]);
    } finally {
      setLoading(false);
    }
  }, [worldId]);

  useEffect(() => {
    void fetchMembers();
  }, [fetchMembers]);

  return { members, loading, error, refetch: fetchMembers };
}
