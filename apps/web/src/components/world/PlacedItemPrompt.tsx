import { useCallback, useEffect, useState } from "react";
import { onPickUpItem, type PickUpItemEvent } from "@/engine/bevy";
import { isAlreadyTaken, pickUpPlacedItem } from "@/api/inventory";
import { Button } from "@/components/ui/button/Button";
import { Card } from "@/components/ui/card/Card";

/**
 * What happens when somebody clicks a thing lying on the floor.
 *
 * # Why a choice and not an action
 *
 * Spec 031 FR-014. Clicking a placed item cannot simply take it: a Game Master
 * checking what they authored, and a player who wants to read the description
 * before committing, both need to look without picking up. So activation
 * offers, and the person decides.
 *
 * # Why chrome and not the engine
 *
 * The engine recognises the effect and resolves which item is meant — that is
 * canvas knowledge. Everything past that point is not: asking the server,
 * knowing this application's URL structure, and knowing which character the
 * person at the keyboard is playing. Constitution Principle I puts that here,
 * and ADR-054 is explicit that the engine must not become a second authority
 * on whether a pickup was allowed.
 *
 * # Why nothing is removed optimistically
 *
 * The token stays on the map until the server's answer comes back through the
 * ordinary sync. That is what makes FR-017 free: a refused pickup leaves the
 * map and every inventory exactly as they were, because nothing was changed in
 * anticipation. A quicker-feeling optimistic delete would have to be undone on
 * refusal, and an undo that runs during a scene change or a disconnect is
 * precisely how a token goes missing for good.
 */

export interface PlacedItemPromptProps {
  worldId: string;
  /**
   * The character receiving the item, or `null` when the viewer is playing
   * nobody — a Game Master with no claim, or a spectator.
   */
  actorId: string | null;
}

type Phase =
  | { kind: "asking" }
  | { kind: "picking-up" }
  | { kind: "done"; message: string }
  | { kind: "refused"; message: string };

export function PlacedItemPrompt({ worldId, actorId }: PlacedItemPromptProps) {
  const [pending, setPending] = useState<PickUpItemEvent | null>(null);
  const [phase, setPhase] = useState<Phase>({ kind: "asking" });

  useEffect(
    () =>
      onPickUpItem((event) => {
        // A second activation replaces the first rather than queueing. Two
        // prompts stacked over the map would hide the thing they are about,
        // and the most recent click is the one the person is thinking of.
        setPending(event);
        setPhase({ kind: "asking" });
      }),
    [],
  );

  const dismiss = useCallback(() => {
    setPending(null);
    setPhase({ kind: "asking" });
  }, []);

  // Escape dismisses, as it does for every other transient surface here.
  useEffect(() => {
    if (!pending) {
      return;
    }
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        dismiss();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [pending, dismiss]);

  if (!pending) {
    return null;
  }

  const handleView = () => {
    window.open(
      `/world/${worldId}/item/${pending.itemId}/view`,
      "_blank",
      "noopener,noreferrer",
    );
    dismiss();
  };

  const handlePickUp = async () => {
    if (!actorId || !pending.subjectRef) {
      return;
    }
    setPhase({ kind: "picking-up" });
    try {
      const entry = await pickUpPlacedItem(pending.subjectRef, actorId);
      // The token disappears when the sync catches up, not here.
      setPhase({ kind: "done", message: `${entry.itemName} is yours.` });
    } catch (error) {
      setPhase({
        kind: "refused",
        message: isAlreadyTaken(error)
          ? "Somebody was quicker — it is already gone."
          : error instanceof Error
            ? error.message
            : "Could not pick that up.",
      });
    }
  };

  // Both are required, and for different reasons worth saying separately.
  const cannotPickUpBecause = !actorId
    ? "You are not playing a character, so there is nowhere to put it."
    : !pending.subjectRef
      ? "This interactive is not attached to anything on the map."
      : null;

  return (
    <Card
      className="fixed bottom-6 left-1/2 z-50 grid w-[min(22rem,90vw)] -translate-x-1/2 gap-3 p-4 shadow-lg"
      data-testid="placed-item-prompt"
      role="dialog"
      aria-label="Placed item"
    >
      {phase.kind === "done" || phase.kind === "refused" ? (
        <>
          <p
            className={
              phase.kind === "done" ? "text-sm" : "text-sm text-destructive"
            }
            data-testid={
              phase.kind === "done"
                ? "placed-item-result"
                : "placed-item-refusal"
            }
          >
            {phase.message}
          </p>
          <Button type="button" size="sm" variant="secondary" onClick={dismiss}>
            Close
          </Button>
        </>
      ) : (
        <>
          <p className="text-sm font-medium">There is something here.</p>
          {cannotPickUpBecause ? (
            <p
              className="text-xs text-muted-foreground"
              data-testid="placed-item-cannot-pick-up"
            >
              {cannotPickUpBecause}
            </p>
          ) : null}
          <div className="flex flex-wrap gap-2">
            <Button
              type="button"
              size="sm"
              icon="inventory"
              onClick={() => void handlePickUp()}
              disabled={
                cannotPickUpBecause !== null || phase.kind === "picking-up"
              }
              data-testid="placed-item-pickup"
            >
              {phase.kind === "picking-up" ? "Picking up..." : "Pick up"}
            </Button>
            <Button
              type="button"
              size="sm"
              variant="secondary"
              icon="quill"
              onClick={handleView}
              data-testid="placed-item-view"
            >
              View
            </Button>
            <Button
              type="button"
              size="sm"
              variant="ghost"
              onClick={dismiss}
              data-testid="placed-item-dismiss"
            >
              Leave it
            </Button>
          </div>
        </>
      )}
    </Card>
  );
}
