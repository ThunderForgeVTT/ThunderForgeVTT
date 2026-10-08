import { useEffect, useState } from "react";

import { rerollRoll } from "@/api/roll";
import { Button } from "@/components/ui/button/Button";
import type { RollFacetRecord, WorldRollRecord } from "@/types/roll";

import { msUntilClosed } from "./rerollWindow";

export interface RerollButtonsViewProps {
  offers: RollFacetRecord[];
  /** Whether the reroll window is still open. */
  open: boolean;
  /** The offer being spent, while its request is in flight. */
  busy: string | null;
  /** The server's refusal, as it said it. */
  error: string | null;
  onReroll: (spend: string) => void;
}

/** Spec 084: one Reroll button per spend the server offered. */
export function RerollButtonsView({
  offers,
  open,
  busy,
  error,
  onReroll,
}: RerollButtonsViewProps) {
  if (!open || offers.length === 0) {
    return null;
  }
  return (
    <div className="mt-1 flex flex-wrap items-center gap-1">
      {offers.map((offer) => (
        <Button
          key={offer.id}
          type="button"
          size="sm"
          variant="secondary"
          disabled={busy !== null}
          onClick={() => onReroll(offer.id)}
          data-testid={`roll-reroll-${offer.id}`}
        >
          {busy === offer.id ? "Rerolling…" : `Reroll (${offer.label})`}
        </Button>
      ))}
      {error ? (
        <p
          className="w-full text-sm text-destructive"
          data-testid="roll-reroll-error"
        >
          {error}
        </p>
      ) : null}
    </div>
  );
}

export interface RerollButtonsProps {
  worldId: string;
  roll: WorldRollRecord;
  /** Takes the new roll in at once, ahead of its event. */
  onRerolled: (roll: WorldRollRecord) => void;
}

/**
 * Spec 084 US3: what the maker of a roll may spend to reroll it. The server
 * decides the offers; this shows them until the window closes, and says the
 * server's refusal when it has one.
 */
export function RerollButtons({
  worldId,
  roll,
  onRerolled,
}: RerollButtonsProps) {
  // The clock the window is read by, moved on by the timer that closes it.
  const [now, setNow] = useState(() => Date.now());
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const left = msUntilClosed(roll.rerollUntil, Date.now());
    if (left === 0) return;
    const timer = window.setTimeout(() => setNow(Date.now()), left);
    return () => window.clearTimeout(timer);
  }, [roll.rerollUntil]);

  const handleReroll = async (spend: string) => {
    setBusy(spend);
    setError(null);
    try {
      onRerolled(await rerollRoll(worldId, roll.id, spend));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to reroll");
    } finally {
      setBusy(null);
    }
  };

  return (
    <RerollButtonsView
      offers={roll.rerolledBy === null ? roll.rerollOffers : []}
      open={msUntilClosed(roll.rerollUntil, now) > 0}
      busy={busy}
      error={error}
      onReroll={(spend) => void handleReroll(spend)}
    />
  );
}
