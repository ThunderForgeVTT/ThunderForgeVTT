import { FantasyIcon } from "@/components/ui/fantasy-icon/FantasyIcon";
import { lookAtToken } from "@/engine/lookAt";

export interface LookAtButtonProps {
  /** The token to put on screen. */
  tokenId: string;
  /** Whose it is, for the control's name — "Look at Boblin". */
  label: string;
  /** For the test id, so a roster row and a tracker row are distinguishable. */
  testIdPrefix: string;
}

/**
 * The little target that scrolls the viewport to a creature (owner decision
 * 2026-09-15).
 *
 * # It is a real control
 *
 * The owner asked for "a little target icon — not a button specifically".
 * Drawn, that is what it is: a reticle, no chrome, sitting quietly in the row
 * until pointed at. Built, it has to be a `<button>` anyway — it does
 * something, so it must be reachable by Tab, operable by Enter and Space, and
 * announced. An icon that is only a `<div>` with a click handler is invisible
 * to anyone not using a mouse, which on a roster of fifty goblins is the
 * difference between a usable tracker and none.
 *
 * The accessible name is the creature's, not the control's: "Look at Boblin",
 * fifty times over, each one distinct in a screen reader's element list. A
 * row of fifty controls all announcing "Look" would be useless.
 *
 * # Why it takes a token, not an actor
 *
 * A creature is somewhere only by standing on the board. An actor with no
 * token on this scene has no position to scroll to, so the caller resolves the
 * token and omits the control when there is none — rather than offering
 * something that would quietly do nothing.
 */
export function LookAtButton({
  tokenId,
  label,
  testIdPrefix,
}: LookAtButtonProps) {
  return (
    <button
      type="button"
      aria-label={`Look at ${label}`}
      title={`Look at ${label}`}
      data-testid={`${testIdPrefix}-${tokenId}`}
      className="rounded p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-none"
      onClick={() => lookAtToken(tokenId)}
    >
      <FantasyIcon name="locate" size={14} aria-hidden="true" />
    </button>
  );
}
