import { useEffect, useState } from "react";
import { SessionClocks, SessionResourceTrade, SessionWishPool } from "@thunderforge/genie";
import { getWorldItems } from "@/api/items";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";
import { useAuth } from "@/hooks/useAuth";
import { useGenieSession } from "@/hooks/useGenieSession";
import type { WorldItemRecord } from "@/types/item";

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
    declineResourceTrade,
    grantSessionResource,
    configurePuzzleClockReward,
  } = useGenieSession(worldId, user?.id);

  const [grantActorId, setGrantActorId] = useState("");
  const [grantResourceType, setGrantResourceType] = useState("insight");
  const [grantAmount, setGrantAmount] = useState("1");
  const [isGranting, setIsGranting] = useState(false);
  const [grantError, setGrantError] = useState<string | null>(null);

  // Spec 020 (User Story 3): Puzzle Clock reward configuration + an
  // actor-attributed advance, supplementary to SessionClocks' own
  // delta-only "Advance" buttons (FR-006a).
  const [worldItems, setWorldItems] = useState<WorldItemRecord[] | null>(null);
  const [rewardClockId, setRewardClockId] = useState("");
  const [rewardTriggerSegment, setRewardTriggerSegment] = useState("1");
  const [rewardKind, setRewardKind] = useState<"resource" | "item">("resource");
  const [rewardResourceType, setRewardResourceType] = useState("insight");
  const [rewardResourceAmount, setRewardResourceAmount] = useState("1");
  const [rewardItemId, setRewardItemId] = useState("");
  const [rewardItemQuantity, setRewardItemQuantity] = useState("1");
  const [rewardRecipientMode, setRewardRecipientMode] = useState<"TRIGGERING_ACTOR" | "WHOLE_PARTY">(
    "TRIGGERING_ACTOR",
  );
  const [isConfiguringReward, setIsConfiguringReward] = useState(false);
  const [rewardError, setRewardError] = useState<string | null>(null);

  const [advanceClockId, setAdvanceClockId] = useState("");
  const [advanceActorId, setAdvanceActorId] = useState("");
  const [advanceDelta, setAdvanceDelta] = useState("1");
  const [isAdvancing, setIsAdvancing] = useState(false);

  useEffect(() => {
    if (!isGm) return;
    getWorldItems(worldId).then(setWorldItems).catch(() => setWorldItems([]));
  }, [isGm, worldId]);

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

  const grantableActors = myActor ? [myActor, ...partyMembers] : partyMembers;

  const handleGrant = async () => {
    if (!grantActorId || !grantResourceType) return;
    const amount = Number.parseInt(grantAmount, 10);
    if (!Number.isFinite(amount) || amount < 1) return;
    setIsGranting(true);
    setGrantError(null);
    try {
      await grantSessionResource(grantActorId, grantResourceType, amount);
      setGrantAmount("1");
    } catch (err) {
      setGrantError(err instanceof Error ? err.message : "Failed to grant resource");
    } finally {
      setIsGranting(false);
    }
  };

  const handleConfigureReward = async () => {
    if (!rewardClockId) return;
    const triggerSegment = Number.parseInt(rewardTriggerSegment, 10);
    if (!Number.isFinite(triggerSegment) || triggerSegment < 1) return;
    setIsConfiguringReward(true);
    setRewardError(null);
    try {
      await configurePuzzleClockReward({
        clockId: rewardClockId,
        triggerSegment,
        rewardResourceType: rewardKind === "resource" ? rewardResourceType : undefined,
        rewardResourceAmount: rewardKind === "resource" ? Number.parseInt(rewardResourceAmount, 10) : undefined,
        rewardItemId: rewardKind === "item" ? rewardItemId : undefined,
        rewardItemQuantity: rewardKind === "item" ? Number.parseInt(rewardItemQuantity, 10) : undefined,
        recipientMode: rewardRecipientMode,
      });
      setRewardTriggerSegment("1");
    } catch (err) {
      setRewardError(err instanceof Error ? err.message : "Failed to configure reward");
    } finally {
      setIsConfiguringReward(false);
    }
  };

  const handleAdvanceWithActor = async () => {
    if (!advanceClockId) return;
    const delta = Number.parseInt(advanceDelta, 10);
    if (!Number.isFinite(delta) || delta === 0) return;
    setIsAdvancing(true);
    try {
      await advancePuzzleClock(advanceClockId, delta, advanceActorId || undefined);
    } finally {
      setIsAdvancing(false);
    }
  };

  return (
    <div className="grid gap-4" data-testid="genie-session-panel">
      {isGm ? (
        <Card className="grid gap-2 p-4" data-testid="genie-grant-resource-panel">
          <h4 className="text-sm font-semibold tracking-tight">Grant Session Resource</h4>
          {grantError ? <p className="text-sm text-destructive">{grantError}</p> : null}
          <div className="grid grid-cols-3 gap-2">
            <select
              value={grantActorId}
              onChange={(event) => setGrantActorId(event.target.value)}
              className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
              data-testid="grant-resource-actor-select"
              aria-label="Character to grant to"
            >
              <option value="">Select a character…</option>
              {grantableActors.map((actor) => (
                <option key={actor.id} value={actor.id}>
                  {actor.label}
                </option>
              ))}
            </select>
            <select
              value={grantResourceType}
              onChange={(event) => setGrantResourceType(event.target.value)}
              className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
              aria-label="Resource type to grant"
            >
              {GENIE_SESSION_RESOURCE_TYPES.map((r) => (
                <option key={r.key} value={r.key}>
                  {r.label}
                </option>
              ))}
            </select>
            <input
              type="number"
              min={1}
              value={grantAmount}
              onChange={(event) => setGrantAmount(event.target.value)}
              className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
              aria-label="Amount to grant"
            />
          </div>
          <Button
            type="button"
            size="sm"
            disabled={isGranting || !grantActorId}
            onClick={() => void handleGrant()}
            data-testid="grant-resource-button"
          >
            Grant
          </Button>
        </Card>
      ) : null}
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
      {isGm && session.puzzleClocks.length > 0 ? (
        <Card className="grid gap-3 p-4" data-testid="genie-puzzle-clock-rewards-panel">
          <h4 className="text-sm font-semibold tracking-tight">Puzzle Clock Rewards</h4>
          {rewardError ? <p className="text-sm text-destructive">{rewardError}</p> : null}
          <div className="grid gap-2">
            <select
              value={rewardClockId}
              onChange={(event) => setRewardClockId(event.target.value)}
              className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
              data-testid="reward-clock-select"
              aria-label="Puzzle Clock to configure"
            >
              <option value="">Select a Puzzle Clock…</option>
              {session.puzzleClocks.map((clock) => (
                <option key={clock.id} value={clock.id}>
                  {clock.label}
                </option>
              ))}
            </select>
            <div className="grid grid-cols-2 gap-2">
              <input
                type="number"
                min={1}
                value={rewardTriggerSegment}
                onChange={(event) => setRewardTriggerSegment(event.target.value)}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                data-testid="reward-trigger-segment-input"
                aria-label="Segment that triggers this reward"
              />
              <select
                value={rewardRecipientMode}
                onChange={(event) => setRewardRecipientMode(event.target.value as "TRIGGERING_ACTOR" | "WHOLE_PARTY")}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                data-testid="reward-recipient-mode-select"
                aria-label="Who receives this reward"
              >
                <option value="TRIGGERING_ACTOR">Triggering actor</option>
                <option value="WHOLE_PARTY">Whole party</option>
              </select>
            </div>
            <div className="flex items-center gap-2 text-sm">
              <label className="flex items-center gap-1">
                <input type="radio" checked={rewardKind === "resource"} onChange={() => setRewardKind("resource")} />
                Resource
              </label>
              <label className="flex items-center gap-1">
                <input type="radio" checked={rewardKind === "item"} onChange={() => setRewardKind("item")} />
                Item
              </label>
            </div>
            {rewardKind === "resource" ? (
              <div className="grid grid-cols-2 gap-2">
                <select
                  value={rewardResourceType}
                  onChange={(event) => setRewardResourceType(event.target.value)}
                  className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                  data-testid="reward-resource-type-select"
                  aria-label="Reward resource type"
                >
                  {GENIE_SESSION_RESOURCE_TYPES.map((r) => (
                    <option key={r.key} value={r.key}>
                      {r.label}
                    </option>
                  ))}
                </select>
                <input
                  type="number"
                  min={1}
                  value={rewardResourceAmount}
                  onChange={(event) => setRewardResourceAmount(event.target.value)}
                  className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                  data-testid="reward-resource-amount-input"
                  aria-label="Reward resource amount"
                />
              </div>
            ) : (
              <div className="grid grid-cols-2 gap-2">
                <select
                  value={rewardItemId}
                  onChange={(event) => setRewardItemId(event.target.value)}
                  disabled={worldItems === null}
                  className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                  data-testid="reward-item-select"
                  aria-label="Reward item"
                >
                  <option value="">{worldItems === null ? "Loading items…" : "Select an item…"}</option>
                  {(worldItems ?? []).map((item) => (
                    <option key={item.id} value={item.id}>
                      {item.name}
                    </option>
                  ))}
                </select>
                <input
                  type="number"
                  min={1}
                  value={rewardItemQuantity}
                  onChange={(event) => setRewardItemQuantity(event.target.value)}
                  className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                  data-testid="reward-item-quantity-input"
                  aria-label="Reward item quantity"
                />
              </div>
            )}
            <Button
              type="button"
              size="sm"
              disabled={isConfiguringReward || !rewardClockId || (rewardKind === "item" && !rewardItemId)}
              onClick={() => void handleConfigureReward()}
              data-testid="reward-configure-button"
            >
              Add reward
            </Button>
          </div>

          <div className="grid gap-2 border-t border-border pt-3">
            <h5 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
              Advance with attribution
            </h5>
            <div className="grid grid-cols-3 gap-2">
              <select
                value={advanceClockId}
                onChange={(event) => setAdvanceClockId(event.target.value)}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                data-testid="advance-with-actor-clock-select"
                aria-label="Puzzle Clock to advance"
              >
                <option value="">Select a clock…</option>
                {session.puzzleClocks.map((clock) => (
                  <option key={clock.id} value={clock.id}>
                    {clock.label}
                  </option>
                ))}
              </select>
              <select
                value={advanceActorId}
                onChange={(event) => setAdvanceActorId(event.target.value)}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                data-testid="advance-with-actor-select"
                aria-label="Actor to attribute this advance to"
              >
                <option value="">(none — whole party)</option>
                {grantableActors.map((actor) => (
                  <option key={actor.id} value={actor.id}>
                    {actor.label}
                  </option>
                ))}
              </select>
              <input
                type="number"
                value={advanceDelta}
                onChange={(event) => setAdvanceDelta(event.target.value)}
                className="h-9 rounded-lg border border-input bg-transparent px-2.5 text-sm outline-none"
                data-testid="advance-with-actor-delta-input"
                aria-label="Segments to advance"
              />
            </div>
            <Button
              type="button"
              size="sm"
              disabled={isAdvancing || !advanceClockId}
              onClick={() => void handleAdvanceWithActor()}
              data-testid="advance-with-actor-button"
            >
              Advance
            </Button>
          </div>
        </Card>
      ) : null}
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
          onDeclineProposal={(proposalId) => declineResourceTrade(proposalId)}
        />
      ) : null}
    </div>
  );
}
