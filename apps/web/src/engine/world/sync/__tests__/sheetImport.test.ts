import { describe, expect, it, vi } from "vitest";

import {
  ACTOR_ROLLED_BACK_EVENT_CODE,
  SHEET_IMPORT_APPLIED_EVENT_CODE,
  STAGED_CONTENT_DECIDED_EVENT_CODE,
  applySheetImportWorldEvent,
  startSheetImportEventSync,
} from "../sheetImport";

describe("sheet import world events", () => {
  it("re-reads the actor an import or a rollback rewrote", () => {
    for (const code of [
      SHEET_IMPORT_APPLIED_EVENT_CODE,
      ACTOR_ROLLED_BACK_EVENT_CODE,
    ]) {
      const onActorSheetChanged = vi.fn();
      applySheetImportWorldEvent(
        { onActorSheetChanged },
        { event_code: code, token_event: { actorId: "actor-1" } },
      );
      expect(onActorSheetChanged).toHaveBeenCalledWith("actor-1");
    }
  });

  it("re-reads the queue and every actor linked to a decided piece", () => {
    const onActorSheetChanged = vi.fn();
    const onStagedContentDecided = vi.fn();
    applySheetImportWorldEvent(
      { onActorSheetChanged, onStagedContentDecided },
      {
        eventCode: STAGED_CONTENT_DECIDED_EVENT_CODE,
        tokenEvent: { stagedId: "staged-1", actorIds: ["a", "b", 3] },
      },
    );
    expect(onStagedContentDecided).toHaveBeenCalledWith("staged-1");
    expect(onActorSheetChanged.mock.calls).toEqual([["a"], ["b"]]);
  });

  it("ignores every other event, and events that name nothing", () => {
    const onActorSheetChanged = vi.fn();
    const onStagedContentDecided = vi.fn();
    for (const eventCode of [31, 38, 39, 43]) {
      applySheetImportWorldEvent(
        { onActorSheetChanged, onStagedContentDecided },
        { eventCode, tokenEvent: { actorId: "a", stagedId: "s" } },
      );
    }
    applySheetImportWorldEvent(
      { onActorSheetChanged },
      { eventCode: SHEET_IMPORT_APPLIED_EVENT_CODE, tokenEvent: {} },
    );
    expect(onActorSheetChanged).not.toHaveBeenCalled();
    expect(onStagedContentDecided).not.toHaveBeenCalled();
  });

  it("stops reading the subscription when stopped", async () => {
    const returned = vi.fn();
    const subscription: AsyncIterable<{ eventCode: number }> = {
      [Symbol.asyncIterator]: () => ({
        next: () => new Promise(() => undefined),
        return: () => {
          returned();
          return Promise.resolve({ value: undefined, done: true });
        },
      }),
    };
    const stop = startSheetImportEventSync({}, subscription);
    stop();
    expect(returned).toHaveBeenCalled();
  });
});
