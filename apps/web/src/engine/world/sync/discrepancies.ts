/**
 * Where a disclosed discrepancy waits until the GM looks at the roll
 * (spec 028 FR-064 to FR-067).
 *
 * # Why this exists at all
 *
 * The two ends were built and did not meet. The server determines the
 * mismatch during `reconcileQueuedChanges` and returns it on the outcome;
 * `RollResult` renders one from a `RollResolutionRecord`. But a reconcile
 * outcome and a roll result arrive by completely different routes and at
 * completely different times — the reconcile happens when a connection comes
 * back, and the roll is looked at whenever the GM happens to look. Nothing
 * carried the first to the second, so the display was unreachable in the
 * running product however correct each half was on its own.
 *
 * This is that carrier, and it is deliberately the smallest thing that can
 * be one: a map from the server's roll record id to the two numbers.
 *
 * # Kept in memory on purpose
 *
 * A discrepancy is disclosure, not a record. The server already holds the
 * roll and its determined value; this is only the note that says "these two
 * disagreed", and it exists so the GM can be shown it while they are here.
 * Persisting it would mean deciding when it stops being worth showing, and
 * an old one resurfacing weeks later — about a player who has since left,
 * say — is exactly the situation FR-066's "no dispute process" is trying to
 * stay out of. Losing it on reload costs a note nobody has to act on.
 */

export interface DisclosedDiscrepancy {
  /** Whose outcome it was — the originator, not whoever submitted it. */
  userId: string;
  reportedValue: number;
  determinedValue: number;
}

const byRecordId = new Map<string, DisclosedDiscrepancy>();

/**
 * Remember what the server disclosed about one roll.
 *
 * Silently ignores anything incomplete. The whole obligation on this path is
 * accuracy — a false discrepancy puts an innocent player under suspicion in
 * front of the only person who can act on it — so a half-populated record is
 * dropped rather than shown with a guess in place of the missing half.
 */
export function noteDiscrepancy(
  recordId: string | null | undefined,
  disclosed: Partial<DisclosedDiscrepancy> | null | undefined,
): void {
  if (!recordId || !disclosed) return;
  const { userId, reportedValue, determinedValue } = disclosed;
  if (typeof userId !== "string") return;
  if (!Number.isFinite(reportedValue) || !Number.isFinite(determinedValue))
    return;
  byRecordId.set(recordId, {
    userId,
    reportedValue: reportedValue as number,
    determinedValue: determinedValue as number,
  });
}

/** What the server disclosed about this roll, if anything. */
export function discrepancyFor(
  recordId: string | null | undefined,
): DisclosedDiscrepancy | null {
  if (!recordId) return null;
  return byRecordId.get(recordId) ?? null;
}

/** Reset module state. Tests only. */
export function resetDiscrepanciesForTests(): void {
  byRecordId.clear();
}
