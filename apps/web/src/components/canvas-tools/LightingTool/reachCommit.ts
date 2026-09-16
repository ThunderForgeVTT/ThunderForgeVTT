/**
 * The Lights panel's bookkeeping for a reach it has committed and not yet seen
 * come back from the server (spec 045 FR-061).
 *
 * A committed reach reaches the store only when the server answers. Until it
 * does, the panel still holds the old stored figure, and a Game Master who
 * sets the dim reach and then, at once, the bright one would have the bright
 * one judged against the dim reach they just replaced: "20 ft bright" refused
 * as further out than a 10 ft dim reach that is already on its way to being
 * 40. And when both do go out, as two requests, the server may take them in
 * either order, clamping a bright reach to whichever dim reach it has stored.
 * So a reach on its way stands in for the stored one, and goes out again with
 * the other reach, so the server always judges the pair the Game Master saw.
 * It stands in only until the stored reach moves, and in any case no longer
 * than `PENDING_MS`.
 */

/** Both of a light's reaches, in world units. */
export interface Reaches {
  radius: number;
  brightRadius: number;
}

/** A committed reach, with the stored reach it replaced and when it was sent. */
export interface PendingReach {
  value: number;
  from: number;
  at: number;
}

/**
 * How long a committed reach is allowed to stand in for the stored one. Long
 * enough for any answer worth waiting for, short enough that a light changed
 * back to the reach it replaced — by the wheel, by undo, by another seat —
 * later on is not read as the answer finally arriving.
 */
export const PENDING_MS = 10_000;

export type PendingReaches = Partial<Record<keyof Reaches, PendingReach>>;

/**
 * Which committed reaches are still on their way: those whose stored reach has
 * not moved since. Once it moves, the server has answered, with this reach or
 * with someone else's since, and the stored figure is the truth again.
 */
export function stillPending(
  stored: Reaches,
  pending: PendingReaches,
  now: number = Date.now(),
): PendingReaches {
  const out: PendingReaches = {};
  for (const key of ["radius", "brightRadius"] as const) {
    const reach = pending[key];
    if (reach && stored[key] === reach.from && now - reach.at < PENDING_MS) {
      out[key] = reach;
    }
  }
  return out;
}

/** The reaches the panel shows and judges by. */
export function currentReaches(
  stored: Reaches,
  pending: PendingReaches,
  now: number = Date.now(),
): Reaches {
  const live = stillPending(stored, pending, now);
  return {
    radius: live.radius?.value ?? stored.radius,
    brightRadius: live.brightRadius?.value ?? stored.brightRadius,
  };
}

/**
 * What to send for a commit of `typed`: the reaches typed, and any reach still
 * on its way that was not, so the server never clamps one reach against the
 * other's old value.
 */
export function reachChanges(
  typed: Partial<Reaches>,
  stored: Reaches,
  pending: PendingReaches,
  now: number = Date.now(),
): Partial<Reaches> {
  const live = stillPending(stored, pending, now);
  const changes: Partial<Reaches> = { ...typed };
  if (Object.keys(typed).length === 0) return changes;
  for (const key of ["radius", "brightRadius"] as const) {
    if (changes[key] === undefined && live[key]) {
      changes[key] = live[key].value;
    }
  }
  return changes;
}

/** The reaches still on their way once `changes` has been sent. */
export function afterCommit(
  changes: Partial<Reaches>,
  stored: Reaches,
  pending: PendingReaches,
  now: number = Date.now(),
): PendingReaches {
  const next = stillPending(stored, pending, now);
  for (const key of ["radius", "brightRadius"] as const) {
    const value = changes[key];
    if (value === undefined) continue;
    // Replacing a reach still on its way: it has still not moved the store.
    next[key] = { value, from: next[key]?.from ?? stored[key], at: now };
  }
  return next;
}
