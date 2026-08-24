import { SessionClocks, SessionResourceTrade, SessionWishPool } from "@thunderforge/genie";
import { Button } from "@/components/ui/button/Button";
import { useAuth } from "@/hooks/useAuth";
import { useGenieSession } from "@/hooks/useGenieSession";

export interface GenieSessionPanelProps {
  worldId: string;
  isGm: boolean;
}

/** Genie's `sessionResources` block (`packs/systems/genie/system.json`) —
 * stable manifest content, hardcoded here rather than an extra manifest
 * fetch (`TokenPanel.tsx`'s `getGameSystemManifest` pattern would work
 * too, but is more code for 3 fixed keys/labels). */
const GENIE_SESSION_RESOURCE_TYPES = [
  { key: "insight", label: "Insight" },
  { key: "favor", label: "Favor" },
  { key: "essence", label: "Essence" },
];

/**
 * Spec 018/019 User Story 7: the Genie GM session loop — Session Wish
 * Pool, Doom/Puzzle Clocks, and (spec 019) Session Resource trading.
 */
export function GenieSessionPanel({ worldId, isGm }: GenieSessionPanelProps) {
  const { user } = useAuth();
  const {
    session,
    loading,
    error,
    startSession,
    spendWish,
    advanceDoomClock,
    createPuzzleClock,
    advancePuzzleClock,
    myActor,
    partyMembers,
    myHoldings,
    incomingProposals,
    proposeResourceTrade,
    acceptResourceTrade,
  } = useGenieSession(worldId, user?.id);

  if (loading) {
    return null;
  }

  if (error) {
    return <p className="text-sm text-destructive">Failed to load Genie session: {error.message}</p>;
  }

  if (!session) {
    return isGm ? (
      <Button
        type="button"
        variant="secondary"
        data-testid="start-genie-session-button"
        onClick={() => void startSession(6)}
      >
        Start Genie session
      </Button>
    ) : (
      <p className="text-sm text-muted-foreground">No Genie session has started yet.</p>
    );
  }

  return (
    <div className="grid gap-4" data-testid="genie-session-panel">
      <SessionWishPool
        wishesRemaining={session.wishesRemaining}
        status={session.status}
        isGm={isGm}
        onSpendWish={(narrativeEffect) => spendWish(narrativeEffect)}
      />
      <SessionClocks
        doomClockCurrent={session.doomClockCurrent}
        doomClockMax={session.doomClockMax}
        puzzleClocks={session.puzzleClocks}
        sessionStatus={session.status}
        isGm={isGm}
        onAdvanceDoomClock={(delta) => advanceDoomClock(delta)}
        onAdvancePuzzleClock={(clockId, delta) => advancePuzzleClock(clockId, delta)}
        onCreatePuzzleClock={(label, segmentsMax) => createPuzzleClock(label, segmentsMax)}
      />
      {myActor ? (
        <SessionResourceTrade
          myActorId={myActor.id}
          myHoldings={myHoldings}
          resourceTypes={GENIE_SESSION_RESOURCE_TYPES}
          partyMembers={partyMembers.map((actor) => ({ actorId: actor.id, label: actor.label }))}
          incomingProposals={incomingProposals.map((proposal) => ({
            id: proposal.id,
            fromActorId: proposal.fromActorId,
            fromActorLabel:
              partyMembers.find((actor) => actor.id === proposal.fromActorId)?.label ?? "Unknown",
            fromResourceType: proposal.fromResourceType,
            fromQuantity: proposal.fromQuantity,
            toResourceType: proposal.toResourceType,
            toQuantity: proposal.toQuantity,
          }))}
          onProposeTrade={(input) =>
            proposeResourceTrade({
              sessionId: session.id,
              fromActorId: myActor.id,
              fromResourceType: input.fromResourceType,
              fromQuantity: input.fromQuantity,
              toActorId: input.toActorId,
              toResourceType: input.toResourceType,
              toQuantity: input.toQuantity,
            })
          }
          onAcceptProposal={(proposalId) => acceptResourceTrade(proposalId)}
        />
      ) : null}
    </div>
  );
}
