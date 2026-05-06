/**
 * useWorldInvites.ts
 * React hook for querying world invites from RxDB collection
 *
 * Usage:
 *   const { invites, loading, error } = useWorldInvites(worldId);
 *
 * Returns reactive updates whenever invites change in RxDB (from backend sync)
 */

import { useEffect, useState } from 'react';
import { useWorldDatabase } from './useWorldDatabase';
import type { WorldInviteDoc } from '../db/collections/worldInvitesCollection';
import { computeInviteDerivedData } from '../db/collections/worldInvitesCollection';

export interface UseWorldInvitesResult {
  invites: WorldInviteDoc[];
  loading: boolean;
  error: Error | null;
}

export function useWorldInvites(worldId: string): UseWorldInvitesResult {
  const db = useWorldDatabase();
  const [invites, setInvites] = useState<WorldInviteDoc[]>([]);
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
        // Query invites for this world, sorted by creation date (newest first)
        const query = db.collections.world_invites.find({
          selector: { world_id: worldId },
          sort: [{ created_at: 'desc' }],
        });

        // Subscribe to reactive updates (RxDB observable)
        const subscription = query.$.subscribe({
          next: (docs: WorldInviteDoc[]) => {
            if (mounted) {
              // Compute derived data for each invite
              const enrichedDocs = docs.map((doc) => {
                const derived = computeInviteDerivedData(doc);
                return { ...doc, ...derived };
              });
              setInvites(enrichedDocs);
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

  return { invites, loading, error };
}
