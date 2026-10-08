/** Spec 084: how long until `until` passes, from `now`; 0 once it has, or without one. */
export function msUntilClosed(until: string | null, now: number): number {
  if (until === null) return 0;
  return Math.max(0, Date.parse(until) - now);
}
