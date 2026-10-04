/**
 * actorAccess.ts — live-sync inbound half for who may edit a character
 * (`world_events` code 31, see `src/server/src/world_events.rs`; spec 063).
 *
 * Same shape as `appearance.ts`: the notify says *that* access to an actor
 * changed, and the client re-reads the actor. Here the payload could not say
 * more if it wanted to. It reaches every member of the world, and who holds
 * what on a character is the Game Master's to read, so it carries the actor
 * and nobody's level. Each client asks the server what it may now do, which
 * is the only answer worth showing anyway.
 *
 * This is what lets a player who has just claimed a character edit it on the
 * page they already have open (spec 063 SC-003), and what takes the controls
 * away again on a release. The server refuses a stale client's write either
 * way; this decides only how soon the page stops offering it.
 */

export const ACTOR_ACCESS_CHANGED_EVENT_CODE = 31;

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

export interface ActorAccessEventHandlers {
  /** Somebody's access to this actor changed. Re-read it to learn whose. */
  onActorAccessChanged?: (actorId: string) => void;
}

export function applyActorAccessWorldEvent(
  handlers: ActorAccessEventHandlers,
  event: WorldEventLike,
): void {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== ACTOR_ACCESS_CHANGED_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;
  const actorId = payload?.actorId;
  // An event that names no actor gives nothing to re-read. Dropped rather
  // than treated as "every actor": no caller could act on that.
  if (typeof actorId === "string") {
    handlers.onActorAccessChanged?.(actorId);
  }
}

/**
 * Drive `applyActorAccessWorldEvent` from a `worldEventsCreated` subscription.
 *
 * Holds the iterator and calls `.return()` on cleanup rather than checking a
 * flag inside `for await`, for the reason given on `startAppearanceEventSync`.
 */
export function startActorAccessEventSync(
  handlers: ActorAccessEventHandlers,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const iterator = graphqlSubscription[Symbol.asyncIterator]();
  let cancelled = false;

  void (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        applyActorAccessWorldEvent(handlers, event);
      }
    } catch (error) {
      console.error("Actor access event sync error:", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
