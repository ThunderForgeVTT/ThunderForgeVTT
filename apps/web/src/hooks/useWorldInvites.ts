/**
 * useWorldInvites.ts
 * React hook for fetching a world's active invite codes.
 *
 * Usage:
 *   const { invites, loading, error, refetch } = useWorldInvites(worldId);
 *
 * RxDB has been hard cut from this layer entirely: it was fatally broken
 * app-wide (RxDB 17.2.0 rejects the inline `index: true` schema shorthand
 * used throughout, error SC26 — crashing `getWorldDatabase()` for every
 * collection), and there was never a live GraphQL subscription transport
 * wired up client-side to make its "reactive" local-cache queries actually
 * reflect server-side pushes anyway. This hook now fetches directly via
 * GraphQL on mount and whenever `worldId` changes, mirroring the
 * established fallback pattern in `useActorSystemData.ts`. There is no
 * live push transport, so invites will NOT update in this view when
 * another user generates/uses one elsewhere — call `refetch()` (or
 * remount) to pick up changes.
 */

import { useCallback, useEffect, useState } from 'react';
import { getWorldInvites } from '@/api/world';
import type { WorldInviteDoc } from '../db/collections/worldInvitesCollection';
import { computeInviteDerivedData } from '../db/collections/worldInvitesCollection';

export interface UseWorldInvitesResult {
  invites: WorldInviteDoc[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
}

export function useWorldInvites(worldId: string): UseWorldInvitesResult {
  const [invites, setInvites] = useState<WorldInviteDoc[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const fetchInvites = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);

      const records = await getWorldInvites(worldId);

      // Sorted by creation date (newest first), with derived data computed
      // client-side (mirrors the old RxDB-doc enrichment step).
      const docs: WorldInviteDoc[] = [...records]
        .sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1))
        .map((record) => {
          const doc: WorldInviteDoc = {
            id: record.id,
            world_id: record.worldId ?? worldId,
            invite_code: record.inviteCode,
            max_uses: record.maxUses,
            used_count: record.usedCount,
            expires_at: record.expiresAt ?? null,
            created_by: record.createdBy,
            created_at: record.createdAt,
            updated_at: record.updatedAt,
          };
          const derived = computeInviteDerivedData(doc);
          return { ...doc, ...derived };
        });

      setInvites(docs);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
      setInvites([]);
    } finally {
      setLoading(false);
    }
  }, [worldId]);

  useEffect(() => {
    void fetchInvites();
  }, [fetchInvites]);

  return { invites, loading, error, refetch: fetchInvites };
}
