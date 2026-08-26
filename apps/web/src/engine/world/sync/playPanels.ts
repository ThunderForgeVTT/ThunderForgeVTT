/**
 * playPanels.ts — live-sync inbound half for the Play view's Chat and
 * Combat panels (`world_events` codes 17 and 18, see
 * `src/server/src/world_events.rs`).
 *
 * Both event codes follow this codebase's established "notify carries an
 * id, the client re-fetches" convention (`tokens.ts`/`walls.ts`), and for
 * chat that convention is a security property rather than a style choice:
 * every world member's subscription receives every event on the world
 * channel, so a GM-only message body riding the payload would be delivered
 * to exactly the clients it is hidden from. The payload is an id, and the
 * refetch re-applies the server's `gm_only` filter.
 */

export const CHAT_MESSAGE_EVENT_CODE = 17;
export const COMBAT_CHANGED_EVENT_CODE = 18;

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

export interface PlayPanelEventHandlers {
  /** A message was posted — refetch `worldChatMessages(worldId)`. */
  onChatMessage?: (messageId: string | undefined) => void;
  /** The tracker changed — refetch `activeCombat(worldId)`. */
  onCombatChanged?: (combatId: string | undefined) => void;
}

/**
 * Apply one `world_events` payload, dispatching to the caller's handlers
 * for codes 17/18 and no-op'ing for anything else — same shape as
 * `applyTokenWorldEvent`/`applyGenieSessionWorldEvent`.
 */
export function applyPlayPanelWorldEvent(
  handlers: PlayPanelEventHandlers,
  event: WorldEventLike,
): void {
  const eventCode = event.event_code ?? event.eventCode;
  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;

  if (eventCode === CHAT_MESSAGE_EVENT_CODE) {
    handlers.onChatMessage?.(payload?.messageId as string | undefined);
    return;
  }

  if (eventCode === COMBAT_CHANGED_EVENT_CODE) {
    handlers.onCombatChanged?.(payload?.combatId as string | undefined);
  }
}

/**
 * Drive `applyPlayPanelWorldEvent` from a `worldEventsCreated`
 * subscription. Returns a cleanup function.
 *
 * Holds the iterator directly and calls `.return()` on cleanup rather than
 * checking a flag inside `for await`: on a quiet world the loop would
 * otherwise hang forever awaiting the next event and never observe the
 * flag — the same reasoning (and the same live-found bug) documented on
 * `startGenieSessionEventSync`.
 */
export function startPlayPanelEventSync(
  handlers: PlayPanelEventHandlers,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const iterator = graphqlSubscription[Symbol.asyncIterator]();
  let cancelled = false;

  void (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        applyPlayPanelWorldEvent(handlers, event);
      }
    } catch (error) {
      console.error("Play panel event sync error:", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
