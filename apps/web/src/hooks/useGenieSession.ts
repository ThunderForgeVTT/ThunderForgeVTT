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
import { getWorldActors } from "@/api/actors";
import { startGenieSessionEventSync, subscribeToWorldEvents } from "@/engine/world/sync";
import {
  acceptResourceTrade as acceptResourceTradeRequest,
  advanceDoomClock as advanceDoomClockRequest,
  advancePuzzleClock as advancePuzzleClockRequest,
  createPuzzleClock as createPuzzleClockRequest,
  declineResourceTrade as declineResourceTradeRequest,
  fetchGenieResourceHoldings,
  fetchGenieSession,
  fetchGenieTradeProposals,
  type GenieResourceHoldingRecord,
  type GenieSessionRecord,
  type GenieTradeProposalRecord,
  proposeResourceTrade as proposeResourceTradeRequest,
  type ProposeResourceTradeInput,
  spendResourceOnPuzzleClock as spendResourceOnPuzzleClockRequest,
  spendWish as spendWishRequest,
  startGenieSession as startGenieSessionRequest,
} from "@/api/genieSession";
import type { WorldActorRecord } from "@/types/actor";

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
  declineResourceTrade: (proposalId: string) => Promise<void>;
  spendResourceOnPuzzleClock: (
    clockId: string,
    actorId: string,
    resourceType: string,
    quantity: number,
  ) => Promise<void>;
  /** Spec 019: the viewer's own PC in this world (first non-NPC actor they
   * own), and every *other* PC — the two things `SessionResourceTrade`
   * needs to offer a trade to someone. `null` while unresolved/absent. */
  myActor: WorldActorRecord | null;
  partyMembers: WorldActorRecord[];
  myHoldings: GenieResourceHoldingRecord[];
  incomingProposals: GenieTradeProposalRecord[];
}

/** worldId may be undefined while the host page's world is still loading —
 * the hook simply won't fetch until it's set. currentUserId may be null
 * while auth is still resolving — myActor/holdings/proposals stay empty
 * until both are known. */
export function useGenieSession(
  worldId: string | undefined,
  currentUserId: string | null | undefined,
): UseGenieSessionResult {
  const [session, setSession] = useState<GenieSessionRecord | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<Error | null>(null);
  const [myActor, setMyActor] = useState<WorldActorRecord | null>(null);
  const [partyMembers, setPartyMembers] = useState<WorldActorRecord[]>([]);
  const [myHoldings, setMyHoldings] = useState<GenieResourceHoldingRecord[]>([]);
  const [incomingProposals, setIncomingProposals] = useState<GenieTradeProposalRecord[]>([]);

  const refetch = useCallback(async () => {
    if (!worldId) {
      setLoading(false);
      return;
    }
    try {
      setLoading(true);
      setError(null);
      const result = await fetchGenieSession(worldId);
      setSession((prev) => {
        if (result) return result;
        // genieSession(worldId) only ever returns the *active* session by
        // design (queries/genie_session.rs filters status="active") — a
        // null result is ambiguous between "no session was ever started"
        // and "the session just concluded" (possibly on another client,
        // which is exactly what the live-sync NOTIFY handler below
        // triggers this refetch for). Don't clobber an already-known
        // concluded session with that ambiguous null — this is the same
        // bug advancePuzzleClock/spendResourceOnPuzzleClock's direct
        // mutation responses were fixed for earlier; a blind refetch()
        // reintroduces it the moment anything (like live sync) calls it
        // after a mutation that could have won/lost the session.
        if (prev && prev.status !== "ACTIVE") return prev;
        return null;
      });
    } catch (err) {
      setError(err instanceof Error ? err : new Error(String(err)));
    } finally {
      setLoading(false);
    }
  }, [worldId]);

  useEffect(() => {
    void refetch();
  }, [refetch]);

  useEffect(() => {
    if (!worldId || !currentUserId) {
      setMyActor(null);
      setPartyMembers([]);
      return;
    }
    let active = true;
    getWorldActors(worldId)
      .then((actors) => {
        if (!active) return;
        const pcs = actors.filter((a) => !a.isNpc);
        // A claimed actor's `ownedBy` stays the GM/creator who made it —
        // the current *controller* is `claimedBy.userId` instead (found
        // live while writing genie-resource-trade.spec.ts: this
        // previously matched only ownedBy, so a player who joined via
        // the real invite-and-claim flow never resolved as "my actor" at
        // all). Falls back to ownedBy for an actor nobody has claimed —
        // e.g. the GM's own PC, played directly without a claim.
        const controllerId = (actor: WorldActorRecord) => actor.claimedBy?.userId ?? actor.ownedBy;
        setMyActor(pcs.find((a) => controllerId(a) === currentUserId) ?? null);
        setPartyMembers(pcs.filter((a) => controllerId(a) !== currentUserId));
      })
      .catch((err) => {
        console.error("Failed to load world actors for resource trading:", err);
      });
    return () => {
      active = false;
    };
  }, [worldId, currentUserId]);

  const refetchTrades = useCallback(async () => {
    if (!session || !myActor) {
      setMyHoldings([]);
      setIncomingProposals([]);
      return;
    }
    const [holdings, proposals] = await Promise.all([
      fetchGenieResourceHoldings(session.id, myActor.id),
      fetchGenieTradeProposals(myActor.id),
    ]);
    setMyHoldings(holdings);
    setIncomingProposals(proposals);
  }, [session, myActor]);

  useEffect(() => {
    void refetchTrades();
  }, [refetchTrades]);

  // Live cross-client sync: a NOTIFY for this world's Genie session state
  // (wish pool/doom clock/puzzle clocks) or a resource trade re-runs the
  // same refetch()/refetchTrades() a manual action already triggers —
  // this is the piece that makes Scenarios 8-9 (spec 018 quickstart)
  // actually work across two real connected clients rather than only
  // updating the tab that performed the action.
  useEffect(() => {
    if (!worldId) return;
    const stopSync = startGenieSessionEventSync(
      {
        onSessionStateChanged: () => void refetch(),
        onResourceTradeChanged: () => void refetchTrades(),
      },
      subscribeToWorldEvents(worldId),
    );
    return stopSync;
  }, [worldId, refetch, refetchTrades]);

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
      const clock = await createPuzzleClockRequest(session.id, label, segmentsMax);
      setSession((prev) => (prev ? { ...prev, puzzleClocks: [...prev.puzzleClocks, clock] } : prev));
    },
    [session],
  );

  // Deliberately not a refetch() after the mutation: genieSession(worldId)
  // only ever returns the *active* session for a world
  // (queries/genie_session.rs filters status="active"), so a mutation
  // that resolves the last unresolved Puzzle Clock and wins the session
  // would make the very next refetch come back null and blank the whole
  // panel instead of showing the won state. Merge the mutation's
  // response locally instead, and mirror the server's own win rule
  // (mutations_genie_session.rs's all_puzzle_clocks_resolved) so the UI
  // reflects "won" immediately — the server is still the authority on
  // the persisted status, this is just keeping the client in sync with
  // what that same mutation just did, the same way advanceDoomClock/
  // spendWish already set state straight from their own responses.
  const advancePuzzleClock = useCallback(async (clockId: string, delta: number) => {
    const updatedClock = await advancePuzzleClockRequest(clockId, delta);
    setSession((prev) => {
      if (!prev) return prev;
      const puzzleClocks = prev.puzzleClocks.map((c) => (c.id === clockId ? updatedClock : c));
      const allResolved = puzzleClocks.length > 0 && puzzleClocks.every((c) => c.resolvedAt);
      return { ...prev, puzzleClocks, status: allResolved ? "WON" : prev.status };
    });
  }, []);

  const proposeResourceTrade = useCallback(
    async (input: ProposeResourceTradeInput) => {
      await proposeResourceTradeRequest(input);
      await refetchTrades();
    },
    [refetchTrades],
  );

  const acceptResourceTrade = useCallback(
    async (proposalId: string) => {
      const holdings = await acceptResourceTradeRequest(proposalId);
      await refetchTrades();
      return holdings;
    },
    [refetchTrades],
  );

  const declineResourceTrade = useCallback(
    async (proposalId: string) => {
      await declineResourceTradeRequest(proposalId);
      await refetchTrades();
    },
    [refetchTrades],
  );

  // Same rationale as advancePuzzleClock above: this can also resolve the
  // clock and win the session, so it merges the response locally instead
  // of refetching into a null-because-concluded genieSession(worldId).
  const spendResourceOnPuzzleClock = useCallback(
    async (clockId: string, actorId: string, resourceType: string, quantity: number) => {
      const updatedClock = await spendResourceOnPuzzleClockRequest(
        clockId,
        actorId,
        resourceType,
        quantity,
      );
      setSession((prev) => {
        if (!prev) return prev;
        const puzzleClocks = prev.puzzleClocks.map((c) => (c.id === clockId ? updatedClock : c));
        const allResolved = puzzleClocks.length > 0 && puzzleClocks.every((c) => c.resolvedAt);
        return { ...prev, puzzleClocks, status: allResolved ? "WON" : prev.status };
      });
    },
    [],
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
    declineResourceTrade,
    spendResourceOnPuzzleClock,
    myActor,
    partyMembers,
    myHoldings,
    incomingProposals,
  };
}

export { fetchGenieResourceHoldings };
