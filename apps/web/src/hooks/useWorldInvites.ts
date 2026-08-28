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

import { useCallback, useEffect, useState } from "react";
import {
  getWorldInvites,
  revokeInviteCode,
  rotateInviteCode,
} from "@/api/world";
import type { WorldInviteDoc } from "../db/collections/worldInvitesCollection";
import { computeInviteDerivedData } from "../db/collections/worldInvitesCollection";

export interface UseWorldInvitesResult {
  invites: WorldInviteDoc[];
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  /**
   * Spec 027 (FR-002): permanently retire a link. Irreversible.
   * Refetches on success — there is no push transport to deliver the change.
   */
  revoke: (inviteId: string) => Promise<void>;
  /**
   * Spec 027 (FR-003): retire a link and issue its replacement. The old code
   * stops working immediately. Returns the new code so the caller can show it
   * without waiting for the refetch to land.
   */
  rotate: (inviteId: string) => Promise<string>;
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
            // Spec 027 (FR-010): server-supplied. The client cannot see
            // revocation by inspecting counts and dates.
            state: record.state,
            remaining_uses: record.remainingUses ?? null,
            rotated_from: record.rotatedFrom ?? null,
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

  // Spec 027 (T026): both mutations refetch on success. There is no live push
  // transport here (see this file's header), so nothing else will reflect the
  // change — and a panel that kept showing a revoked link as active would be
  // worse than one that never showed state at all.
  const revoke = useCallback(
    async (inviteId: string) => {
      await revokeInviteCode(inviteId);
      await fetchInvites();
    },
    [fetchInvites],
  );

  const rotate = useCallback(
    async (inviteId: string) => {
      const replacement = await rotateInviteCode(inviteId);
      await fetchInvites();
      return replacement.inviteCode;
    },
    [fetchInvites],
  );

  return { invites, loading, error, refetch: fetchInvites, revoke, rotate };
}
