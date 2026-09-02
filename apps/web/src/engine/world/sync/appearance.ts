/**
 * appearance.ts — live-sync inbound half for a world's interface pack
 * (`world_events` code 23, see `src/server/src/world_events.rs`).
 *
 * Same shape as `playPanels.ts`, and for the same reason: the notify says
 * *that* the pack changed, and the client re-resolves. The payload does carry
 * the new id, but it is treated as a hint rather than as truth — the pack still
 * has to be fetched and validated before it can be applied, and a client that
 * trusted the payload would apply a look the server might have refused.
 *
 * This is what makes SC-001's "every other participant sees the change without
 * reloading" true, and it costs nothing extra on reconnect: the spec 028
 * catch-up replays events a client missed while it was away, so a look that
 * changed during an outage arrives with everything else.
 */

export const WORLD_APPEARANCE_CHANGED_EVENT_CODE = 23;

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

export interface AppearanceEventHandlers {
  /** The world's pack changed. The id is a hint; re-resolve to be sure. */
  onAppearanceChanged?: (interfacePackId: string | null) => void;
}

export function applyAppearanceWorldEvent(
  handlers: AppearanceEventHandlers,
  event: WorldEventLike,
): void {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== WORLD_APPEARANCE_CHANGED_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;
  const id = payload?.interfacePackId;
  handlers.onAppearanceChanged?.(typeof id === "string" ? id : null);
}

/**
 * Drive `applyAppearanceWorldEvent` from a `worldEventsCreated` subscription.
 *
 * Holds the iterator and calls `.return()` on cleanup rather than checking a
 * flag inside `for await`: on a quiet world the loop would otherwise hang
 * forever awaiting the next event and never observe the flag — the live-found
 * bug documented on `startGenieSessionEventSync`.
 */
export function startAppearanceEventSync(
  handlers: AppearanceEventHandlers,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const iterator = graphqlSubscription[Symbol.asyncIterator]();
  let cancelled = false;

  void (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        applyAppearanceWorldEvent(handlers, event);
      }
    } catch (error) {
      console.error("Appearance event sync error:", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
