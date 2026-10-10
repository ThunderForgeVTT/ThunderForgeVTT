/**
 * sheetImport.ts — live-sync inbound half for bringing a character in
 * (spec 048; `world_events` codes 40-42, see
 * `crates/thunderforge-server/src/world_events.rs`).
 *
 * As with `actorAccess.ts`, the events say *what* changed and the client
 * reads it again: an applied import or a rollback rewrites an actor's sheet
 * fields and links, and a decision on staged content changes the GM's queue
 * and repoints the links of every actor that carried the piece.
 */

export const SHEET_IMPORT_APPLIED_EVENT_CODE = 40;
export const STAGED_CONTENT_DECIDED_EVENT_CODE = 41;
export const ACTOR_ROLLED_BACK_EVENT_CODE = 42;

type WorldEventLike = {
  event_code?: number;
  eventCode?: number;
  token_event?: unknown;
  tokenEvent?: unknown;
};

export interface SheetImportEventHandlers {
  /** This actor's sheet and links were rewritten. Read them again. */
  onActorSheetChanged?: (actorId: string) => void;
  /** The staged queue changed. Read it again. */
  onStagedContentDecided?: (stagedId: string) => void;
}

export function applySheetImportWorldEvent(
  handlers: SheetImportEventHandlers,
  event: WorldEventLike,
): void {
  const eventCode = event.event_code ?? event.eventCode;
  const payload = (event.token_event ?? event.tokenEvent) as
    | Record<string, unknown>
    | undefined;

  if (
    eventCode === SHEET_IMPORT_APPLIED_EVENT_CODE ||
    eventCode === ACTOR_ROLLED_BACK_EVENT_CODE
  ) {
    if (typeof payload?.actorId === "string") {
      handlers.onActorSheetChanged?.(payload.actorId);
    }
    return;
  }

  if (eventCode === STAGED_CONTENT_DECIDED_EVENT_CODE) {
    if (typeof payload?.stagedId === "string") {
      handlers.onStagedContentDecided?.(payload.stagedId);
    }
    // An adoption repoints every link to the piece: each actor reads again.
    const actorIds = Array.isArray(payload?.actorIds) ? payload.actorIds : [];
    for (const actorId of actorIds) {
      if (typeof actorId === "string") {
        handlers.onActorSheetChanged?.(actorId);
      }
    }
  }
}

/**
 * Drive `applySheetImportWorldEvent` from a `worldEventsCreated`
 * subscription. Same shape as `startActorAccessEventSync`.
 */
export function startSheetImportEventSync(
  handlers: SheetImportEventHandlers,
  graphqlSubscription: AsyncIterable<WorldEventLike>,
): () => void {
  const iterator = graphqlSubscription[Symbol.asyncIterator]();
  let cancelled = false;

  void (async () => {
    try {
      while (!cancelled) {
        const { value: event, done } = await iterator.next();
        if (done || cancelled || !event) break;
        applySheetImportWorldEvent(handlers, event);
      }
    } catch (error) {
      console.error("Sheet import event sync error:", error);
    }
  })();

  return () => {
    cancelled = true;
    void iterator.return?.();
  };
}
