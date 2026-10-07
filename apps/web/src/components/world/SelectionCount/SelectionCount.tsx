import { Badge } from "@/components/ui/badge";

export interface SelectionCountProps {
  /** The world store's `selectedTokenIds`, topmost first. */
  selectedTokenIds: readonly string[];
}

/**
 * How many tokens a click picked up, when it picked up more than one.
 *
 * A click on a stack takes the whole stack, and the board marks every member
 * with a ring, but rings drawn on top of one another read as one. This says
 * the number out loud, so "I am about to move three tokens" is never a
 * surprise. One token, or none, needs no readout: its own ring says it.
 *
 * Read from the world store rather than the engine (Constitution I: React
 * observes, it does not ask the board).
 */
export function SelectionCount({ selectedTokenIds }: SelectionCountProps) {
  const count = selectedTokenIds.length;
  if (count < 2) return null;
  return (
    <Badge
      variant="secondary"
      role="status"
      aria-live="polite"
      data-testid="selection-count"
      className="pointer-events-none border-border shadow-md"
    >
      {count} selected
    </Badge>
  );
}
