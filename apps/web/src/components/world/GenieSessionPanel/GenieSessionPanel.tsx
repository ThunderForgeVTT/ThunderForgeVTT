import { SessionClocks, SessionWishPool } from "@thunderforge/genie";
import { Button } from "@/components/ui/button/Button";
import { useGenieSession } from "@/hooks/useGenieSession";

export interface GenieSessionPanelProps {
  worldId: string;
  isGm: boolean;
}

/**
 * Spec 018 User Story 7: the Genie GM session loop — Session Wish Pool
 * and Doom/Puzzle Clocks. `SessionResourceTrade` is deliberately not
 * wired here yet: the backend only exposes point mutations
 * (`proposeResourceTrade`/`acceptResourceTrade`), not a query to list a
 * player's pending incoming proposals, so there's no real data source for
 * that component's `incomingProposals` prop without either a new backend
 * query or a live subscription transport (neither exists yet — see
 * `apps/web/src/engine/world/sync/genieSession.ts`'s doc comment).
 */
export function GenieSessionPanel({ worldId, isGm }: GenieSessionPanelProps) {
  const { session, loading, error, startSession, spendWish, advanceDoomClock, createPuzzleClock, advancePuzzleClock } =
    useGenieSession(worldId);

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
    </div>
  );
}
