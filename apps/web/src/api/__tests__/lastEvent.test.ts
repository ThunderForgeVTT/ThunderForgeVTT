import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { postGraphQL } from "@/api/graphqlClient";
import {
  LAST_EVENT_HEADER,
  noteLastEvent,
  resetLastEvent,
  withLastEvent,
} from "@/api/lastEvent";

/**
 * Spec 048 T059: every request says how far this tab has caught up, so the
 * server never reports a refused use from a tab that may only be behind.
 */

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  resetLastEvent();
  fetchMock = vi.fn().mockResolvedValue(
    new Response(JSON.stringify({ data: { ok: true } }), {
      headers: { "content-type": "application/json" },
    }),
  );
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  vi.unstubAllGlobals();
  resetLastEvent();
});

const sent = (): Record<string, string> =>
  Object.fromEntries(new Headers(fetchMock.mock.calls[0][1].headers));

describe("the last-event header", () => {
  it("is absent before any world has synced", async () => {
    await postGraphQL("query Ok { ok }");
    expect(sent()[LAST_EVENT_HEADER]).toBeUndefined();
  });

  it("carries the newest event applied, and never goes backwards", async () => {
    noteLastEvent("w1", 40);
    noteLastEvent("w1", 42);
    noteLastEvent("w1", 41);
    await postGraphQL("query Ok { ok }");
    expect(sent()[LAST_EVENT_HEADER]).toBe("42");
  });

  it("is the furthest-behind world's when a tab watches two", () => {
    noteLastEvent("w1", 90);
    noteLastEvent("w2", 12);
    expect(withLastEvent({})).toEqual({ [LAST_EVENT_HEADER]: "12" });
  });

  it("ignores ids that are not event ids", () => {
    noteLastEvent("w1", -1);
    noteLastEvent("w1", Number.NaN);
    expect(withLastEvent({})).toEqual({});
  });
});
