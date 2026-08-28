/**
 * Deciding whether a roll is worth a second look (spec 028 US7, T102a).
 *
 * Kept apart from `RollResult.tsx` so the judgement can be tested without a
 * DOM — this repo's vitest runs in a `node` environment and has no component
 * tests, and this is the half that has to be right. What the GM *sees* is
 * e2e's business; what counts as a discrepancy at all is this file's.
 *
 * # Why the display re-checks something the server already decided
 *
 * FR-067a: a false positive puts an innocent player under suspicion in front
 * of the one person who can act on it, while a missed discrepancy costs
 * nothing — the GM runs their table either way. That asymmetry is worth a
 * second, independent refusal at the point of display. A half-populated
 * record, a value that arrived as a string, a comparison against a value the
 * server never actually determined: each of those is an ambiguity, and every
 * ambiguity here reads as "no discrepancy".
 */

/** Two numbers that disagree: what the client said, and what the server found. */
export interface RollDiscrepancy {
  /** The total the client reported. */
  claimedValue: number;
  /** The total the server independently determined (ADR-044). */
  determinedValue: number;
}

/**
 * What the server said about a roll it could check for itself.
 *
 * Populated by discrepancy detection in `mutations_reconcile.rs` (T099/T102c).
 * Optional on the wire and optional here: where the server has no independent
 * basis for an outcome there is nothing to compare and the field is absent
 * (FR-068), which is not the same as a comparison that came out equal.
 */
export interface RollDiscrepancyRecord {
  claimedValue?: number | null;
  determinedValue?: number | null;
  /**
   * The server's own spelling on the reconcile outcome. Accepted here so the
   * two ends cannot disagree about a name — they already did once, and the
   * result was a value that travelled the whole way and then read as absent.
   */
  reportedValue?: number | null;
  /** Snake-cased alternates, for whichever spelling the field ships under. */
  claimed_value?: number | null;
  determined_value?: number | null;
  reported_value?: number | null;
}

function finite(...candidates: (number | null | undefined)[]): number | null {
  for (const candidate of candidates) {
    if (typeof candidate === "number" && Number.isFinite(candidate)) return candidate;
  }
  return null;
}

/**
 * The discrepancy to render, or `null` for every other case.
 *
 * Two independent gates, and both are requirements rather than tidiness.
 *
 * `isGameMaster` is FR-067: the display exists for the GM and for nobody
 * else. Gating it here rather than at the call site means a component that
 * forgets to ask shows nothing, which is the safe direction — a player
 * catching sight of a mark against another player is the harm the whole
 * design avoids.
 *
 * The rest is FR-067a. A mismatch is reported only where two genuine numbers
 * genuinely differ. When in doubt, report nothing.
 */
export function discrepancyToShow(
  record: RollDiscrepancyRecord | null | undefined,
  isGameMaster: boolean,
): RollDiscrepancy | null {
  if (!isGameMaster) return null;
  if (!record) return null;

  const claimed = finite(
    record.claimedValue,
    record.claimed_value,
    record.reportedValue,
    record.reported_value,
  );
  const determined = finite(record.determinedValue, record.determined_value);
  // Either side missing means the comparison never happened — a timeout, a
  // parse failure, a version the server could not replay. Absence of evidence
  // is not a flag (FR-068).
  if (claimed === null || determined === null) return null;
  if (claimed === determined) return null;

  return { claimedValue: claimed, determinedValue: determined };
}
