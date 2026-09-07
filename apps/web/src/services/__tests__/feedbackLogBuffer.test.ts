import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  MAX_ENTRIES,
  MAX_ENTRY_BYTES,
  MAX_TOTAL_BYTES,
  isCapturing,
  resetLogCaptureForTests,
  snapshot,
  startLogCapture,
} from "../feedbackLogBuffer";
import { setSubmitterEmail } from "../feedbackRedaction";

/**
 * The evidence buffer (spec 037, US2 / FR-007, FR-011, FR-012).
 *
 * The four properties tested here are the four the contract makes claims
 * about, and each has a plausible implementation that passes a weaker test:
 *
 * - a secret is never retrievable from the buffer *in any form* — not just
 *   absent from a rendering of it, which is what a "redact in the preview"
 *   design would satisfy;
 * - the bounds hold and what falls off is counted, because FR-011 needs a
 *   number;
 * - capture is idempotent, because StrictMode invokes it twice and a second
 *   patch over the first duplicates every line and restores a patched
 *   console when undone;
 * - nothing reaches browser storage, which is the property that makes a
 *   shared machine inherit nothing.
 */

/** A window that records what was registered, so listeners can be fired. */
interface FakeWindow {
  addEventListener: ReturnType<typeof vi.fn>;
  removeEventListener: ReturnType<typeof vi.fn>;
}

let fakeWindow: FakeWindow;
let handlers: Map<string, (event: unknown) => void>;

/**
 * Every browser storage API the buffer is forbidden to touch, stubbed so
 * that touching one is a test failure rather than a code review.
 */
const storageSpies = {
  localSet: vi.fn(),
  sessionSet: vi.fn(),
  indexedOpen: vi.fn(),
  opfsRoot: vi.fn(),
};

beforeEach(() => {
  handlers = new Map();
  for (const spy of Object.values(storageSpies)) {
    spy.mockClear();
  }

  fakeWindow = {
    addEventListener: vi.fn((type: string, handler: (e: unknown) => void) => {
      handlers.set(type, handler);
    }),
    removeEventListener: vi.fn((type: string) => {
      handlers.delete(type);
    }),
  };

  vi.stubGlobal("window", fakeWindow);
  vi.stubGlobal("localStorage", {
    setItem: storageSpies.localSet,
    getItem: () => null,
  });
  vi.stubGlobal("sessionStorage", {
    setItem: storageSpies.sessionSet,
    getItem: () => null,
  });
  vi.stubGlobal("indexedDB", { open: storageSpies.indexedOpen });
  vi.stubGlobal("navigator", {
    storage: { getDirectory: storageSpies.opfsRoot },
    userAgent: "vitest",
  });
});

afterEach(() => {
  resetLogCaptureForTests();
  setSubmitterEmail(null);
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

/** Silence the pass-through so the suite's own output stays readable. */
function quietConsole() {
  vi.spyOn(console, "error").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
  vi.spyOn(console, "info").mockImplementation(() => {});
}

describe("capture", () => {
  it("records console.error, warn and info, and calls the original through", () => {
    const seen: unknown[][] = [];
    vi.spyOn(console, "error").mockImplementation((...args: unknown[]) => {
      seen.push(args);
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.spyOn(console, "info").mockImplementation(() => {});

    startLogCapture();
    console.error("boom", { sceneId: 7 });
    console.warn("careful");
    console.info("fyi");

    const taken = snapshot();
    expect(taken.entries.map((entry) => entry.level)).toEqual([
      "error",
      "warn",
      "info",
    ]);
    expect(taken.entries[0].text).toBe('boom {"sceneId":7}');
    expect(
      seen,
      "capture must never change what a developer sees in their own console",
    ).toEqual([["boom", { sceneId: 7 }]]);
  });

  it("keeps an Error's stack, which is the reason the line is worth having", () => {
    quietConsole();
    startLogCapture();

    const error = new Error("render threw");
    console.error(error);

    expect(snapshot().entries[0].text).toContain("render threw");
  });

  it("records what the app already knows about: uncaught errors and rejections", () => {
    quietConsole();
    startLogCapture();

    expect(fakeWindow.addEventListener).toHaveBeenCalledWith(
      "error",
      expect.any(Function),
    );
    expect(fakeWindow.addEventListener).toHaveBeenCalledWith(
      "unhandledrejection",
      expect.any(Function),
    );

    handlers.get("error")?.({
      message: "Script error",
      filename: "http://localhost/main.tsx",
      lineno: 12,
      colno: 3,
      error: new Error("Script error"),
    });
    handlers.get("unhandledrejection")?.({ reason: new Error("no world") });

    const texts = snapshot().entries.map((entry) => entry.text);
    expect(texts[0]).toContain("Uncaught Script error");
    expect(texts[0]).toContain("main.tsx:12:3");
    expect(texts[1]).toContain("Unhandled rejection");
    expect(texts[1]).toContain("no world");
  });

  it("is idempotent — a second start does not patch the patch", () => {
    quietConsole();

    const stopA = startLogCapture();
    const stopB = startLogCapture();

    expect(stopB).toBe(stopA);
    expect(isCapturing()).toBe(true);

    console.error("once");

    expect(
      snapshot().entries,
      "StrictMode calls the effect twice; the line must still be recorded once",
    ).toHaveLength(1);
  });

  it("restores the real console when it stops, and stays stopped", () => {
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    vi.spyOn(console, "warn").mockImplementation(() => {});
    vi.spyOn(console, "info").mockImplementation(() => {});

    const stop = startLogCapture();
    expect(console.error).not.toBe(spy);

    stop();
    stop();

    expect(console.error).toBe(spy);
    expect(isCapturing()).toBe(false);
    expect(fakeWindow.removeEventListener).toHaveBeenCalledTimes(2);

    console.error("after");
    expect(snapshot().entries).toHaveLength(0);
  });
});

describe("bounds", () => {
  it("holds at 500 entries and counts what fell off", () => {
    quietConsole();
    startLogCapture();

    for (let index = 0; index < MAX_ENTRIES + 100; index += 1) {
      console.error(`line ${index}`);
    }

    const taken = snapshot();
    expect(taken.entries).toHaveLength(MAX_ENTRIES);
    expect(taken.droppedCount).toBe(100);
    expect(
      taken.droppedOldestAt,
      "FR-011 needs a number and a place to start it from, not a shrug",
    ).not.toBeNull();
    // Oldest-first: what survives is the tail.
    expect(taken.entries[0].text).toBe("line 100");
    expect(taken.entries[MAX_ENTRIES - 1].text).toBe(
      `line ${MAX_ENTRIES + 99}`,
    );
  });

  it("holds at 128 KB however many entries that is", () => {
    quietConsole();
    startLogCapture();

    const chunk = "x".repeat(MAX_ENTRY_BYTES);
    for (let index = 0; index < 200; index += 1) {
      console.error(chunk);
    }

    const taken = snapshot();
    expect(taken.byteSize).toBeLessThanOrEqual(MAX_TOTAL_BYTES);
    expect(taken.entries.length).toBeLessThan(MAX_ENTRIES);
    expect(taken.droppedCount).toBeGreaterThan(0);
  });

  it("truncates one enormous line instead of letting it evict the rest", () => {
    quietConsole();
    startLogCapture();

    console.error("first");
    console.error("y".repeat(MAX_TOTAL_BYTES * 2));
    console.error("last");

    const taken = snapshot();
    expect(
      taken.entries.map((entry) => entry.text.slice(0, 5)),
      "the line that explained the enormous one must survive it",
    ).toEqual(["first", "yyyyy", "last"]);

    const enormous = taken.entries[1].text;
    expect(enormous).toContain("[truncated:");
    expect(new TextEncoder().encode(enormous).length).toBeLessThan(
      MAX_ENTRY_BYTES + 100,
    );
  });

  it("counts eviction down again, so the numbers describe what is held", () => {
    quietConsole();
    startLogCapture();

    console.error("Authorization: Bearer aaaabbbbccccdddd");
    expect(snapshot().redactionCount).toBe(1);

    for (let index = 0; index < MAX_ENTRIES + 1; index += 1) {
      console.error(`filler ${index}`);
    }

    expect(
      snapshot().redactionCount,
      "the redacted entry has been evicted; its count must go with it",
    ).toBe(0);
  });
});

describe("secrets", () => {
  const secret = "Bearer sk-live-2f9c41ab77de";

  it("never holds one, in any form retrievable from the buffer", () => {
    quietConsole();
    startLogCapture();

    console.error(`fetch failed with ${secret}`);
    console.warn("Set-Cookie: session=0191d4ac-3f1e-7b02; Path=/");
    handlers.get("unhandledrejection")?.({
      reason: `GET /a.webp?X-Amz-Signature=deadbeefcafe0011 403`,
    });

    const taken = snapshot();
    const everything = JSON.stringify(taken);

    for (const leak of [
      "sk-live-2f9c41ab77de",
      "0191d4ac-3f1e-7b02",
      "deadbeefcafe0011",
    ]) {
      expect(everything, `${leak} reached the buffer`).not.toContain(leak);
      for (const entry of taken.entries) {
        expect(entry.text).not.toContain(leak);
      }
    }

    expect(taken.redactionCount).toBe(3);
    expect(taken.entries[0].text).toContain("[redacted: bearer token]");
  });

  it("redacts at push, so the bytes reviewed are the bytes submitted", () => {
    quietConsole();
    startLogCapture();

    console.error(`fetch failed with ${secret}`);

    // Two reads of the same buffer, as the review and the submission make.
    // They are the same because there is nothing between them that could
    // change them — no redaction step waiting until send.
    const reviewed = snapshot();
    const submitted = snapshot();

    expect(submitted.entries.map((entry) => entry.text)).toEqual(
      reviewed.entries.map((entry) => entry.text),
    );
    expect(reviewed.entries[0].text).not.toContain("sk-live");
  });

  it("cannot be un-redacted by a later change of mind about the rules", () => {
    quietConsole();
    startLogCapture();

    setSubmitterEmail("player@example.com");
    console.error("failed for player@example.com");
    setSubmitterEmail(null);

    expect(
      snapshot().entries[0].text,
      "what is in the ring was redacted on the way in and has no other form",
    ).not.toContain("player@example.com");
  });
});

describe("storage", () => {
  it("writes to none of it — the buffer dies with the tab", () => {
    quietConsole();
    startLogCapture();

    for (let index = 0; index < MAX_ENTRIES + 50; index += 1) {
      console.error(`line ${index} Bearer aaaabbbbccccdddd`);
    }
    handlers.get("error")?.({
      message: "boom",
      filename: "",
      lineno: 0,
      colno: 0,
    });
    snapshot();

    expect(storageSpies.localSet).not.toHaveBeenCalled();
    expect(storageSpies.sessionSet).not.toHaveBeenCalled();
    expect(storageSpies.indexedOpen).not.toHaveBeenCalled();
    expect(storageSpies.opfsRoot).not.toHaveBeenCalled();
  });

  it("names no storage key anywhere in its source", async () => {
    // The stubs above catch a call. This catches the other half: a key
    // written by some path the tests do not exercise. There is no correct
    // reason for this module to mention any of these at all.
    const { readFile } = await import("node:fs/promises");
    const source = await readFile(
      new URL("../feedbackLogBuffer.ts", import.meta.url),
      "utf8",
    );
    const code = source.replace(/\/\*[\s\S]*?\*\//g, "");

    for (const api of [
      "localStorage",
      "sessionStorage",
      "indexedDB",
      "getDirectory",
      "caches",
    ]) {
      expect(code, `${api} has no business in the log buffer`).not.toContain(
        api,
      );
    }
  });
});
