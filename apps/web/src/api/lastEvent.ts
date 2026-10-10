/**
 * Spec 048 FR-038b: the newest world event this tab has applied, sent with
 * every GraphQL request as `x-tf-last-event`.
 *
 * When a refused use names content the GM has just declined, the server
 * reports it only if this tab had already been told of the decision; a tab
 * that may simply not have caught up is never reported. The sync records what
 * it has applied here, per world, and the header carries the lowest of them:
 * a tab watching two worlds is only as current as the one it is furthest
 * behind in, so it errs towards not reporting.
 */
export const LAST_EVENT_HEADER = "x-tf-last-event";

const applied = new Map<string, number>();

/** Record that events up to `id` have been applied for `worldId`. */
export function noteLastEvent(worldId: string, id: number): void {
  if (!Number.isSafeInteger(id) || id < 0) return;
  if (id > (applied.get(worldId) ?? 0)) applied.set(worldId, id);
}

/** The header to send, or nothing before any world has synced. */
export function withLastEvent(headers: HeadersInit = {}): HeadersInit {
  if (applied.size === 0) return headers;
  const lowest = Math.min(...applied.values());
  return { ...headers, [LAST_EVENT_HEADER]: String(lowest) };
}

/** Forget every world; for tests and sign-out. */
export function resetLastEvent(): void {
  applied.clear();
}
