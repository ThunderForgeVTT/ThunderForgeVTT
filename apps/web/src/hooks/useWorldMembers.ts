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
import { useWorldChromeStore } from "@/hooks/world-chrome-context";
import type { WorldMemberDoc } from "../db/collections/worldMembersCollection";
import { sortMembersByRole } from "../db/collections/worldMembersCollection";
import { isWorldMemberRole } from "@/types/world";

export interface UseWorldMembersResult {
  members: WorldMemberDoc[];
  /** True only while no member list is known for the world (spec 101). */
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

const NO_MEMBERS: WorldMemberDoc[] = [];

/** The roster as the app uses it, sorted by role, highest first. */
async function readMembers(worldId: string): Promise<WorldMemberDoc[]> {
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

  return sortMembersByRole(docs);
}

/**
 * Spec 101 US2: the roster is held per world for the session, so a page
 * change shows the members (and the Game Master's controls that hang on
 * them) at once, and reads them again behind the page. A failed read keeps
 * the members already known rather than emptying the list.
 */
export function useWorldMembers(worldId: string): UseWorldMembersResult {
  const { chrome, refresh } = useWorldChromeStore();
  const [failure, setFailure] = useState<{
    worldId: string;
    error: Error;
  } | null>(null);

  const read = useCallback(
    () =>
      refresh(worldId, "members", readMembers).then(
        () => setFailure(null),
        (err: unknown) =>
          setFailure({
            worldId,
            error: err instanceof Error ? err : new Error(String(err)),
          }),
      ),
    [worldId, refresh],
  );

  useEffect(() => {
    void read();
  }, [read]);

  const known = chrome[worldId]?.members;
  const failed = failure?.worldId === worldId ? failure.error : null;
  return {
    members: known ?? NO_MEMBERS,
    loading: known === undefined && !failed,
    error: failed,
    refetch: read,
  };
}
