import { useState } from "react";

import { revealRoll } from "@/api/roll";
import { Button } from "@/components/ui/button/Button";
import type { WorldRollEntry, WorldRollRecord } from "@/types/roll";

import { feedTime } from "./feedTime";

export interface RollEntryProps {
  worldId: string;
  entry: WorldRollEntry;
  isGm: boolean;
  /** Takes the revealed roll in at once, ahead of its event. */
  onRevealed: (roll: WorldRollRecord) => void;
}

const BADGE =
  "rounded bg-muted px-1.5 py-0.5 text-[0.65rem] font-semibold tracking-widest text-muted-foreground uppercase";

const VISIBILITY_BADGE = {
  EVERYONE: null,
  GM_EYES: "GM's eyes",
  GM_ONLY: "GM only",
} as const;

function total(roll: WorldRollRecord): string {
  const { resultKind, resultValue } = roll.resolution;
  return resultKind === "SUCCESS_COUNT"
    ? `${resultValue} ${resultValue === 1 ? "success" : "successes"}`
    : String(resultValue);
}

/**
 * Spec 081: one roll in the chat feed, exactly as the server answered it for
 * this viewer. A `MaskedRoll` has no dice to show, so `****` is all there is;
 * the marks and the Reveal button read the entry's own visibility.
 */
export function RollEntry({
  worldId,
  entry,
  isGm,
  onRevealed,
}: RollEntryProps) {
  const [revealing, setRevealing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const time = (
    <time className="ml-auto text-xs text-muted-foreground">
      {feedTime(entry.createdAt)}
    </time>
  );

  if (entry.__typename === "MaskedRoll") {
    return (
      <div
        data-testid="roll-entry"
        data-roll-id={entry.id}
        data-masked="true"
        className="flex items-baseline gap-2"
      >
        <span className="text-sm">
          <span className="font-semibold">{entry.rollerName}</span> rolled for
          the GM:{" "}
          <span data-testid="roll-total" className="font-mono">
            ****
          </span>
        </span>
        {time}
      </div>
    );
  }

  const badge = VISIBILITY_BADGE[entry.visibility];
  const mayReveal = isGm && badge !== null && entry.revealedAt === null;

  const handleReveal = async () => {
    setRevealing(true);
    setError(null);
    try {
      onRevealed(await revealRoll(worldId, entry.id));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to reveal");
    } finally {
      setRevealing(false);
    }
  };

  return (
    <div
      data-testid="roll-entry"
      data-roll-id={entry.id}
      data-visibility={entry.visibility}
    >
      <div className="flex items-baseline gap-2">
        <span className="text-sm font-semibold">{entry.rollerName}</span>
        {badge && entry.revealedAt === null ? (
          <span className={BADGE} data-testid="roll-visibility">
            {badge}
          </span>
        ) : null}
        {entry.facets.map((facet) => (
          <span
            key={facet.id}
            className={BADGE}
            data-testid="roll-facet"
            data-facet-id={facet.id}
          >
            {facet.label}
          </span>
        ))}
        {time}
      </div>
      <p className="text-sm">
        {entry.label ? (
          <span data-testid="roll-label" className="font-medium">
            {entry.label}:{" "}
          </span>
        ) : null}
        <span className="font-mono text-muted-foreground">{entry.formula}</span>{" "}
        <span className="text-muted-foreground" data-testid="roll-dice">
          [
          {entry.resolution.dice.map((die, at) => (
            <span key={at}>
              {at > 0 ? ", " : null}
              {/* A die the formula dropped, such as the low one of 2d20kh1. */}
              <span className={die.kept ? undefined : "line-through"}>
                {die.finalValue}
              </span>
            </span>
          ))}
          ]
        </span>{" "}
        = <strong data-testid="roll-total">{total(entry)}</strong>
      </p>
      {entry.revealedAt !== null ? (
        <p
          className="text-xs text-muted-foreground"
          data-testid="roll-revealed"
        >
          revealed by {entry.revealedByName ?? "the GM"}
        </p>
      ) : null}
      {mayReveal ? (
        <Button
          type="button"
          size="sm"
          variant="secondary"
          className="mt-1"
          disabled={revealing}
          onClick={() => void handleReveal()}
          data-testid="roll-reveal-button"
        >
          {revealing ? "Revealing…" : "Reveal"}
        </Button>
      ) : null}
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
    </div>
  );
}
