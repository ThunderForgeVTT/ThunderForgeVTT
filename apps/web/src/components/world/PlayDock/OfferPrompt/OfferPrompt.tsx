import { useCallback, useEffect, useState } from "react";
import { getPendingOffers, resolveOffer } from "@/api/attacks";
import { Button } from "@/components/ui/button/Button";
import {
  startPlayPanelEventSync,
  subscribeToWorldEvents,
} from "@/engine/world/sync";
import type { OfferRecord } from "@/types/attack";

export interface OfferPromptProps {
  worldId: string;
  isGm: boolean;
}

/**
 * "Take 5 damage?" (spec 046 decision 1, FR-005, FR-008, FR-009).
 *
 * Nobody is told by the software that they have been hit: a hit arrives here
 * as an offer, for whoever controls the creature, and they take it or they do
 * not. An offer waits, so this reads `pendingOffers` on load as well as on
 * event 30: a player who was away comes back to what is waiting for them.
 *
 * A Game Master is sent every pending offer in the world and may resolve any
 * of them for the player it belongs to. Whether that was on the player's
 * behalf is the server's to decide and to record; the table sees it in the
 * attack log.
 */
export function OfferPrompt({ worldId, isGm }: OfferPromptProps) {
  const [offers, setOffers] = useState<OfferRecord[]>([]);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    getPendingOffers(worldId)
      .then((pending) => setOffers(pending.filter((o) => o.mayResolve)))
      .catch(() => undefined);
  }, [worldId]);

  useEffect(() => {
    reload();
  }, [reload]);

  useEffect(() => {
    return startPlayPanelEventSync(
      { onOfferChanged: () => reload() },
      subscribeToWorldEvents(worldId),
    );
  }, [worldId, reload]);

  const resolve = async (offer: OfferRecord, take: boolean) => {
    setBusy(offer.id);
    setError(null);
    try {
      await resolveOffer(offer.id, take);
      setOffers((current) => current.filter((o) => o.id !== offer.id));
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Resolving the offer failed",
      );
      reload();
    } finally {
      setBusy(null);
    }
  };

  if (offers.length === 0 && !error) return null;

  return (
    <section
      aria-label={isGm ? "Pending offers" : "Offers for you"}
      data-testid="offer-prompt"
      className="pointer-events-auto grid gap-2 rounded-lg border border-amber-500/60 bg-background/95 p-2 text-sm shadow-lg backdrop-blur"
    >
      <h2 className="text-xs font-semibold tracking-widest text-muted-foreground uppercase">
        {isGm ? "Pending offers" : "Offers for you"}
      </h2>
      <ul className="grid gap-2">
        {offers.map((offer) => {
          const change = offer.kind === "HEALING" ? "healing" : "damage";
          const question = `${offer.target.label}: take ${offer.amount} ${change}?`;
          return (
            <li
              key={offer.id}
              data-testid="offer-row"
              data-offer-id={offer.id}
              className="grid gap-1"
            >
              <span data-testid="offer-question">{question}</span>
              <div className="flex gap-2">
                <Button
                  type="button"
                  size="sm"
                  disabled={busy !== null}
                  onClick={() => void resolve(offer, true)}
                  data-testid="offer-take"
                  aria-label={`Take ${offer.amount} ${change} for ${offer.target.label}`}
                >
                  Take
                </Button>
                <Button
                  type="button"
                  size="sm"
                  variant="secondary"
                  disabled={busy !== null}
                  onClick={() => void resolve(offer, false)}
                  data-testid="offer-decline"
                  aria-label={`Decline ${offer.amount} ${change} for ${offer.target.label}`}
                >
                  Decline
                </Button>
              </div>
            </li>
          );
        })}
      </ul>
      {error ? (
        <p className="text-xs text-destructive" data-testid="offer-error">
          {error}
        </p>
      ) : null}
    </section>
  );
}
