import type { ActivationResult } from "@/api/interactives";

/**
 * What the player who asked is told (FR-028).
 *
 * The player-side counterpart of the queue. Deliberately small, and
 * deliberately not silent: someone who asked to go somewhere and heard nothing
 * back cannot tell "the GM has not looked yet" from "this feature is broken",
 * and the second reading is the one people reach for.
 *
 * Nothing here says *why* a request was turned down. That is the Game Master's
 * to explain at the table, in their own words, and a generated reason would put
 * words in their mouth about their own scene.
 */

export interface RequestOutcomeProps {
  /** What the server said when the request was raised, or when it was decided. */
  result: ActivationResult | null;
  /** Whether the GM has since decided it, and how. */
  decision?: "approved" | "refused" | null;
}

export function RequestOutcome({
  result,
  decision = null,
}: RequestOutcomeProps) {
  if (decision === "approved") {
    return <p role="status">The GM said yes.</p>;
  }
  if (decision === "refused") {
    return <p role="status">The GM said not yet.</p>;
  }
  if (result?.outcome === "requested") {
    // No countdown, because there is no timeout. Saying "waiting" and meaning
    // it is more honest than a progress bar for something that will wait
    // indefinitely.
    return <p role="status">Asked the GM. Waiting on them.</p>;
  }
  return null;
}
