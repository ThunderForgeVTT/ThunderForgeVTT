import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  getPhoneSnapshot,
  PHONE_QUERY,
  subscribePhone,
} from "./usePhoneLayout";

/**
 * A stand-in for `matchMedia` that understands the one query the hook asks,
 * so the test exercises the query itself rather than a hard-coded answer.
 */
function stubViewport(width: number, height: number) {
  const viewport = { width, height };
  const listeners = new Set<() => void>();
  const evaluate = (query: string) =>
    query
      .split(",")
      .map((part) => part.trim())
      .some((part) => {
        const m = /^\(max-(width|height): (\d+)px\)$/.exec(part);
        if (!m) throw new Error(`unexpected media query part: ${part}`);
        return viewport[m[1] as "width" | "height"] <= Number(m[2]);
      });
  (globalThis as { matchMedia?: unknown }).matchMedia = (query: string) => ({
    get matches() {
      return evaluate(query);
    },
    media: query,
    addEventListener: (_: "change", fn: () => void) => listeners.add(fn),
    removeEventListener: (_: "change", fn: () => void) => listeners.delete(fn),
  });
  return {
    resize(w: number, h: number) {
      viewport.width = w;
      viewport.height = h;
      for (const fn of listeners) fn();
    },
    listeners,
  };
}

describe("usePhoneLayout", () => {
  let saved: unknown;
  beforeEach(() => {
    saved = (globalThis as { matchMedia?: unknown }).matchMedia;
  });
  afterEach(() => {
    (globalThis as { matchMedia?: unknown }).matchMedia = saved;
  });

  it("asks for a shorter side under 600px", () => {
    expect(PHONE_QUERY).toBe("(max-width: 599px), (max-height: 599px)");
  });

  it.each([
    [1280, 800, false],
    [820, 1180, false],
    [412, 915, true],
    [915, 412, true],
  ])("at %ix%i the phone layout is %s", (w, h, phone) => {
    stubViewport(w, h);
    expect(getPhoneSnapshot()).toBe(phone);
  });

  it("tells subscribers when the viewport crosses the line", () => {
    const viewport = stubViewport(1280, 800);
    let calls = 0;
    const unsubscribe = subscribePhone(() => calls++);
    viewport.resize(412, 915);
    expect(calls).toBe(1);
    expect(getPhoneSnapshot()).toBe(true);
    unsubscribe();
    expect(viewport.listeners.size).toBe(0);
  });

  it("is never a phone without matchMedia", () => {
    delete (globalThis as { matchMedia?: unknown }).matchMedia;
    expect(getPhoneSnapshot()).toBe(false);
    expect(() => subscribePhone(() => {})()).not.toThrow();
  });
});
