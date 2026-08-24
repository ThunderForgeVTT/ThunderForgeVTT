/**
 * apps/web/src/hooks/useGenieSession.ts
 * Spec 018 User Story 7: the Genie session loop (Session Wish Pool, Doom
 * Clock, Puzzle Clocks, Session Resource trades) — GM staging page host
 * hook.
 *
 * No live GraphQL subscription transport (apollo-client/graphql-ws) exists
 * anywhere in apps/web yet — same gap `useActorSystemData.ts` and
 * `engine/world/sync/genieSession.ts`'s own doc comment record — so this
 * fetches on mount/worldId change and exposes `refetch`, same pattern as
 * `useActorSystemData`. Each mutation call below already returns the
 * fresh session/clock/holdings shape from the server, so action callbacks
 * update local state directly from the response instead of forcing a
 * round-trip refetch.
 */

import { useCallback, useEffect, useState } from "react";
import {
  acceptResourceTrade as acceptResourceTradeRequest,
  advanceDoomClock as advanceDoomClockRequest,
  advancePuzzleClock as advancePuzzleClockRequest,
  createPuzzleClock as createPuzzleClockRequest,
  fetchGenieResourceHoldings,
  fetchGenieSession,
  type GenieResourceHoldingRecord,
  type GenieSessionRecord,
  proposeResourceTrade as proposeResourceTradeRequest,
  type ProposeResourceTradeInput,
  spendResourceOnPuzzleClock as spendResourceOnPuzzleClockRequest,
  spendWish as spendWishRequest,
  startGenieSession as startGenieSessionRequest,
} from "@/api/genieSession";

export interface UseGenieSessionResult {
  session: GenieSessionRecord | null;
  loading: boolean;
  error: Error | null;
  refetch: () => Promise<void>;
  startSession: (doomClockMax: number) => Promise<void>;
  spendWish: (narrativeEffect: string) => Promise<void>;
  advanceDoomClock: (delta: number) => Promise<void>;
  createPuzzleClock: (label: string, segmentsMax: number) => Promise<void>;
  advancePuzzleClock: (clockId: string, delta: number) => Promise<void>;
  proposeResourceTrade: (input: ProposeResourceTradeInput) => Promise<void>;
  acceptResourceTrade: (proposalId: string) => Promise<GenieResourceHoldingRecord[]>;
  spendResourceOnPuzzleClock: (
    clockId: string,
    actorId: string,
    resourceType: string,
    quantity: number,
  ) => Promise<void>;
}

/** worldId may be undefined while the host page's world is still loading —
 * the hook simply won't fetch until it's set. */
export function useGenieSession(worldId: string | undefined): UseGenieSessionResult {
  const [session, setSession] = useState<GenieSessionRecord | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);

  const refetch = useCallback(async () => {
    if (!worldId) {
      setLoading(false);
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const result = await fetchGenieSession(worldId);
      setSession(result);
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setLoading(false);
    }
  }, [worldId]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  const startSession = useCallback(
    async (doomClockMax: number) => {
      if (!worldId) return;
      const result = await startGenieSessionRequest(worldId, doomClockMax);
      setSession(result);
    },
    [worldId],
  );

  const spendWish = useCallback(
    async (narrativeEffect: string) => {
      if (!session) return;
      const result = await spendWishRequest(session.id, narrativeEffect);
      setSession(result);
    },
    [session],
  );

  const advanceDoomClock = useCallback(
    async (delta: number) => {
      if (!session) return;
      const result = await advanceDoomClockRequest(session.id, delta);
      setSession(result);
    },
    [session],
  );

  const createPuzzleClock = useCallback(
    async (label: string, segmentsMax: number) => {
      if (!session) return;
      await createPuzzleClockRequest(session.id, label, segmentsMax);
      await refetch();
    },
    [session, refetch],
  );

  const advancePuzzleClock = useCallback(
    async (clockId: string, delta: number) => {
      await advancePuzzleClockRequest(clockId, delta);
      await refetch();
    },
    [refetch],
  );

  const proposeResourceTrade = useCallback(async (input: ProposeResourceTradeInput) => {
    await proposeResourceTradeRequest(input);
  }, []);

  const acceptResourceTrade = useCallback(async (proposalId: string) => {
    return acceptResourceTradeRequest(proposalId);
  }, []);

  const spendResourceOnPuzzleClock = useCallback(
    async (clockId: string, actorId: string, resourceType: string, quantity: number) => {
      await spendResourceOnPuzzleClockRequest(clockId, actorId, resourceType, quantity);
      await refetch();
    },
    [refetch],
  );

  return {
    session,
    loading,
    error,
    refetch,
    startSession,
    spendWish,
    advanceDoomClock,
    createPuzzleClock,
    advancePuzzleClock,
    proposeResourceTrade,
    acceptResourceTrade,
    spendResourceOnPuzzleClock,
  };
}

export { fetchGenieResourceHoldings };
