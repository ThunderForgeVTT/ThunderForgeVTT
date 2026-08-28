/**
 * genieSession.ts
 * Spec 018 (User Story 7): the Genie session loop's live-sync inbound
 * half — Session Wish Pool, Doom Clock, Puzzle Clocks, and Session
 * Resource trades. Mirrors the exact shape of `walls.ts`/`tokens.ts`'s
 * "inbound" responsibility, but for `eventCode = 15`
 * (`src/server/src/world_events.rs::EVENT_CODE_GENIE_SESSION_STATE`)
 * rather than a scene-scoped canvas primitive.
 *
 * Per data-model.md's `world_events` section and
 * `contracts/genie-session-loop.md`/`contracts/genie-economy.md`, the
 * notify payload (`token_event` JSON column) is a discriminated union on
 * `kind`: `"wish_pool" | "doom_clock" | "puzzle_clock" | "resource_trade"
 * | "resource_grant" | "purchase" | "clock_reward"` (the last three added
 * by spec 020), each carrying `session_id` plus kind-specific fields — never the full
 * session/clock/holdings shape, the same "notify carries an id, the
 * client re-fetches" convention `tokens.ts`/`walls.ts` already use for
 * their own event codes (10/14). This module intentionally does NOT
 * re-fetch itself (there's no Genie-specific GraphQL client module in
 * apps/web yet, per research.md's scope — server-side T032-T036 own
 * that contract); instead it exposes a single `applyGenieSessionWorldEvent`
 * callback-based dispatcher so a host page can plug in its own
 * `genieSession(worldId)` / `genieResourceHoldings(...)` refetch, the
 * same "reasonable default, drop-in once a subscription transport
 * exists" shape `walls.ts`'s module doc comment documents — no
 * live GraphQL subscription transport (apollo-client/graphql-ws) exists
 * anywhere in apps/web yet, confirmed by the same search walls.ts's NOTE
 * already recorded; `startGenieSessionEventSync` is written to the same
 * `for await (const event of subscription)` shape so it drops in
 * unchanged once one does.
 */

export const GENIE_SESSION_EVENT_CODE = 15;

export type GenieSessionEventKind =
  | "wish_pool"
  | "doom_clock"
  | "puzzle_clock"
  | "resource_trade"
  | "resource_grant"
  | "purchase"
  | "clock_reward";

export interface GenieSessionWorldEventPayload {
  kind: GenieSessionEventKind;
  session_id?: string;
  sessionId?: string;
  [key: string]: unknown;
}

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

export interface GenieSessionEventHandlers {
  /** Fired for any `wish_pool`, `doom_clock`, or `puzzle_clock` payload — the host page should re-run `genieSession(worldId)`. */
  onSessionStateChanged?: (payload: GenieSessionWorldEventPayload) => void;
  /** Fired for a `resource_trade` payload — the host page should re-run `genieResourceHoldings(sessionId, actorId)`. */
  onResourceTradeChanged?: (payload: GenieSessionWorldEventPayload) => void;
}

/**
 * Apply a single `world_events` NOTIFY payload, dispatching to the
 * caller-supplied handlers when it's a Genie session event
 * (eventCode 15). No-ops for any other event code, mirroring
 * `applyTokenWorldEvent`/`applyWallWorldEvent`'s shape exactly.
 */
export function applyGenieSessionWorldEvent(
  handlers: GenieSessionEventHandlers,
  event: WorldEventLike,
): void {
  const eventCode = event.event_code ?? event.eventCode;
  if (eventCode !== GENIE_SESSION_EVENT_CODE) {
    return;
  }

  const payload = (event.token_event ?? event.tokenEvent) as
    | GenieSessionWorldEventPayload
    | undefined;
  if (!payload || !payload.kind) {
    return;
  }

  // Spec 020: resource_grant/purchase change a single actor's holdings
  // (and, for purchase, inventory) the same way an accepted trade does —
  // route them through onResourceTradeChanged too. clock_reward can
  // change BOTH the clock's segment state and a recipient's holdings, so
  // it fires both handlers rather than picking one.
  if (
    payload.kind === "resource_trade" ||
    payload.kind === "resource_grant" ||
    payload.kind === "purchase"
  ) {
    handlers.onResourceTradeChanged?.(payload);
  } else if (payload.kind === "clock_reward") {
    handlers.onSessionStateChanged?.(payload);
    handlers.onResourceTradeChanged?.(payload);
  } else {
    handlers.onSessionStateChanged?.(payload);
  }
}

/**
 * Drive applyGenieSessionWorldEvent from a `worldEventsCreated`
 * GraphQL subscription async iterable. Returns a cleanup function that
 * stops consuming the subscription — same shape as
 * `startTokenEventSync`/`startWallEventSync`.
 */
export function startGenieSessionEventSync(
  handlers: GenieSessionEventHandlers,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  // Not an AbortController + in-loop flag check: `for await` only
  // re-checks anything between events, so on a quiet world the loop
  // hangs forever awaiting the next one and the cleanup function
  // wouldn't actually close the subscription. Holding the iterator
  // directly and calling `.return()` unblocks a pending `next()`
  // immediately instead (this is what `for await...of` itself does on
  // early exit — this is that same behavior, invoked explicitly since
  // this cleanup happens from outside the loop, not inside it). Found
  // live while first actually driving this from a real subscription
  // (this function predates one existing at all).
  const iterator = graphqlSubscription[Symbol.asyncIterator]();
  let cancelled = false;

  (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        applyGenieSessionWorldEvent(handlers, event);
      }
    } catch (error) {
      console.error("Genie session event sync error:", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
