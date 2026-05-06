/**
 * useWorldMembers.ts
 * React hook for querying world members/roster from RxDB collection
 *
 * Usage:
 *   const { members, loading, error } = useWorldMembers(worldId);
 *
 * Returns reactive updates whenever members change in RxDB (from backend sync)
 */

import { useEffect, useState } from 'react';
import { useWorldDatabase } from './useWorldDatabase';
import type { WorldMemberDoc } from '../db/collections/worldMembersCollection';
import { sortMembersByRole } from '../db/collections/worldMembersCollection';

export interface UseWorldMembersResult {
  members: WorldMemberDoc[];
  loading: boolean;
  error: Error | null;
}

export function useWorldMembers(worldId: string): UseWorldMembersResult {
  const db = useWorldDatabase();
  const [members, setMembers] = useState<WorldMemberDoc[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  useEffect(() => {
    if (!db) {
      setLoading(false);
      return;
    }

    let mounted = true;
    const unsubscribes: Array<() => void> = [];

    (async () => {
      try {
        // Query members for this world, sorted by join time (oldest first)
        const query = db.collections.world_members.find({
          selector: { world_id: worldId },
          sort: [{ joined_at: 'asc' }],
        });

        // Subscribe to reactive updates (RxDB observable)
        const subscription = query.$.subscribe({
          next: (docs: WorldMemberDoc[]) => {
            if (mounted) {
              // Sort by role hierarchy: Owner, GM, Player
              const sortedDocs = sortMembersByRole(docs);
              setMembers(sortedDocs);
              setLoading(false);
              setError(null);
            }
          },
          error: (err: Error) => {
            if (mounted) {
              setError(err);
              setLoading(false);
            }
          },
        });

        unsubscribes.push(() => subscription.unsubscribe());
      } catch (err) {
        if (mounted) {
          setError(err instanceof Error ? err : new Error(String(err)));
          setLoading(false);
        }
      }
    })();

    return () => {
      mounted = false;
      unsubscribes.forEach((unsub) => unsub());
    };
  }, [db, worldId]);

  return { members, loading, error };
}
