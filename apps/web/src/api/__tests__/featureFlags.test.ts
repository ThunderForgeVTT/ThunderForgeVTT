import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  currentFeatureFlags,
  forgetFeatureFlags,
  loadFeatureFlags,
  refreshFeatureFlags,
  subscribeToFeatureFlags,
} from "@/api/featureFlags";

/**
 * Spec 068 FR-013: flags are asked for once per person, again on an
 * administrator's change, and read as off until answered.
 */

type Reply = { key: string; on: boolean }[];

function answerWith(...replies: (Reply | Error)[]) {
  const fetchMock = vi.fn();
  for (const reply of replies) {
    if (reply instanceof Error) {
      fetchMock.mockRejectedValueOnce(reply);
    } else {
      fetchMock.mockResolvedValueOnce(
        new Response(JSON.stringify({ data: { featureFlags: reply } }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        }),
      );
    }
  }
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

const ON: Reply = [{ key: "feature.book_import", on: true }];
const OFF: Reply = [{ key: "feature.book_import", on: false }];

describe("feature flags", () => {
  beforeEach(forgetFeatureFlags);
  afterEach(() => vi.unstubAllGlobals());

  it("asks the route a visitor can reach when nobody is signed in", async () => {
    const fetchMock = answerWith([], ON);
    await loadFeatureFlags(null);
    expect(fetchMock.mock.calls[0][0]).toBe("/api/graphql/public");
    await loadFeatureFlags("account-1");
    expect(fetchMock.mock.calls[1][0]).toBe("/api/graphql");
  });

  it("reads every flag as unknown before the server has answered", () => {
    expect(currentFeatureFlags()["feature.book_import"]).toBeUndefined();
  });

  it("asks once for the same person", async () => {
    const fetchMock = answerWith(ON);
    await loadFeatureFlags("a");
    await loadFeatureFlags("a");
    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(currentFeatureFlags()).toEqual({ "feature.book_import": true });
  });

  it("asks again when somebody else is looking", async () => {
    const fetchMock = answerWith([], ON);
    await loadFeatureFlags(null);
    expect(currentFeatureFlags()).toEqual({});
    await loadFeatureFlags("a");
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(currentFeatureFlags()).toEqual({ "feature.book_import": true });
  });

  it("asks again on a refresh and tells whoever is listening", async () => {
    answerWith(ON, OFF);
    await loadFeatureFlags("a");
    const heard = vi.fn();
    const stop = subscribeToFeatureFlags(heard);
    await refreshFeatureFlags();
    stop();
    expect(heard).toHaveBeenCalledTimes(1);
    expect(currentFeatureFlags()).toEqual({ "feature.book_import": false });
  });

  it("asks again after a read that failed", async () => {
    const fetchMock = answerWith(new TypeError("offline"), ON);
    await loadFeatureFlags("a");
    expect(currentFeatureFlags()).toEqual({});
    await loadFeatureFlags("a");
    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(currentFeatureFlags()).toEqual({ "feature.book_import": true });
  });
});
