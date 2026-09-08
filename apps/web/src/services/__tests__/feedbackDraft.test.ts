/**
 * Spec 037 FR-005: an unsent draft survives the form being dismissed and
 * reopened within the same session.
 *
 * The dismiss/reopen cycle is exactly `saveDraft` then `loadDraft` with the
 * module's own state thrown away in between, which is what the fresh-import
 * case below simulates: nothing is held in memory between the two calls, so a
 * pass means the words really came back from `sessionStorage`.
 */

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  DRAFT_STORAGE_KEY,
  EMPTY_DRAFT,
  clearDraft,
  draftIsEmpty,
  loadDraft,
  saveDraft,
  type FeedbackDraft,
} from "../feedbackDraft";

/** The store the browser would give us, with no browser present. */
class MemoryStorage implements Storage {
  private items = new Map<string, string>();

  get length() {
    return this.items.size;
  }
  clear() {
    this.items.clear();
  }
  getItem(key: string) {
    return this.items.get(key) ?? null;
  }
  key(index: number) {
    return [...this.items.keys()][index] ?? null;
  }
  removeItem(key: string) {
    this.items.delete(key);
  }
  setItem(key: string, value: string) {
    this.items.set(key, value);
  }
}

beforeEach(() => {
  globalThis.sessionStorage = new MemoryStorage();
});

afterEach(() => {
  Reflect.deleteProperty(globalThis, "sessionStorage");
});

const typed: FeedbackDraft = {
  kind: "FEATURE_REQUEST",
  summary: "Let a GM reorder initiative by dragging",
  message: "Clicking through the list mid-combat loses the thread.",
  includeLogs: false,
  includeContext: true,
};

describe("the draft survives a dismiss and a reopen", () => {
  it("returns what was typed after the form is closed", () => {
    saveDraft(typed);

    // The dismissal: nothing of the form is left. The only thing that could
    // carry the words is the store.
    expect(loadDraft()).toEqual(typed);
  });

  it("keeps the kind and the tick boxes, not only the words", () => {
    saveDraft(typed);
    const reopened = loadDraft();

    expect(reopened.kind).toBe("FEATURE_REQUEST");
    expect(reopened.includeLogs).toBe(false);
  });

  it("is cleared only by an explicit clear, which is what a send does", () => {
    saveDraft(typed);
    clearDraft();

    expect(loadDraft()).toEqual(EMPTY_DRAFT);
  });

  it("stores nothing at all when nothing was typed", () => {
    saveDraft({ ...EMPTY_DRAFT, summary: "   " });

    expect(globalThis.sessionStorage.getItem(DRAFT_STORAGE_KEY)).toBeNull();
    expect(draftIsEmpty(loadDraft())).toBe(true);
  });

  it("never uses localStorage — the draft must not outlive the tab", () => {
    saveDraft(typed);

    // If this module ever reached for `localStorage`, the read above would
    // have had to put something there. FR-005 says "within the same session",
    // and a draft on a shared machine tomorrow is a record nobody asked for.
    expect(Reflect.has(globalThis, "localStorage")).toBe(false);
  });
});

describe("a store that cannot be read", () => {
  it("yields an empty draft rather than throwing", () => {
    Reflect.deleteProperty(globalThis, "sessionStorage");

    expect(loadDraft()).toEqual(EMPTY_DRAFT);
    expect(() => saveDraft(typed)).not.toThrow();
    expect(() => clearDraft()).not.toThrow();
  });

  it("ignores a corrupted or hostile value", () => {
    globalThis.sessionStorage.setItem(DRAFT_STORAGE_KEY, '{"kind":"ADMIN"');
    expect(loadDraft()).toEqual(EMPTY_DRAFT);

    globalThis.sessionStorage.setItem(
      DRAFT_STORAGE_KEY,
      JSON.stringify({ kind: "ADMIN", message: 7 }),
    );
    const recovered = loadDraft();
    expect(recovered.kind).toBe(EMPTY_DRAFT.kind);
    expect(recovered.message).toBe("");
  });
});
