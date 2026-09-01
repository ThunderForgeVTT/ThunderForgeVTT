import { beforeEach, describe, expect, it } from "vitest";
import {
  getLiveSyncState,
  noteSocketClosed,
  noteSocketConnected,
  noteSocketRetry,
  resetSocketStateForTests,
  subscribeToLiveSyncState,
} from "../subscriptionClient";

/**
 * What the socket's events do to the reported connection state.
 *
 * `connectivity.test.ts` covers the *judgement* — given a situation, which
 * status is it. This covers the half that decides when to ask: the transitions
 * `graphql-ws` drives through `connected`, `closed` and `retryWait`.
 *
 * That half had no coverage, and its absence was expensive rather than
 * theoretical. The reconnect window is about a second wide, so the only tests
 * watching this seam were browser tests racing a banner — and the question
 * "does closing the socket leave `live`?" cost a thirty-second run to answer
 * and was answered wrongly twice before a probe settled it. Everything below
 * runs in milliseconds and cannot race anything.
 */
describe("socket events reaching the reported state", () => {
  beforeEach(() => {
    resetSocketStateForTests();
  });

  it("starts out connecting, having seen no socket at all", () => {
    expect(getLiveSyncState()).toEqual({ status: "connecting" });
  });

  it("is live once the handshake completes", () => {
    noteSocketConnected();
    expect(getLiveSyncState()).toEqual({ status: "live" });
  });

  it("leaves live the moment the socket closes", () => {
    // The property the connection indicator rests on, and the one a probe had
    // to be written to establish. A tab reporting a healthy connection it does
    // not have is worse than one reporting the outage: it makes an offline
    // edit indistinguishable from an online one.
    noteSocketConnected();
    noteSocketClosed();
    expect(getLiveSyncState().status).not.toBe("live");
  });

  it("calls a first drop reconnecting, not disconnected", () => {
    // One drop with no failed attempt behind it is not yet an outage, and
    // saying so would spend the loud state on the common case.
    noteSocketConnected();
    noteSocketClosed();
    expect(getLiveSyncState()).toEqual({ status: "reconnecting", attempt: 0 });
  });

  it("stays in connecting when a socket closes before ever connecting", () => {
    // A first handshake that fails has never been live, so there is nothing to
    // reconnect to — reporting "reconnecting" here would invent a connection
    // the user never had.
    noteSocketClosed();
    expect(getLiveSyncState()).toEqual({ status: "connecting" });
  });

  it("counts retries from one, not zero", () => {
    // `graphql-ws` counts retries zero-based; a user reading "attempt 0" would
    // reasonably conclude nothing was being tried.
    noteSocketConnected();
    noteSocketClosed();
    noteSocketRetry(0);
    expect(getLiveSyncState()).toMatchObject({ attempt: 1 });
  });

  it("keeps a first handshake's retries out of the reconnecting count", () => {
    // Never live, so retrying is still the first connection being made — not a
    // reconnection with a confusing attempt count against it.
    noteSocketRetry(4);
    expect(getLiveSyncState()).toEqual({ status: "connecting" });
  });

  it("returns to live on recovery, with the attempt count cleared", () => {
    noteSocketConnected();
    noteSocketClosed();
    noteSocketRetry(3);
    noteSocketConnected();
    expect(getLiveSyncState()).toEqual({ status: "live" });
  });

  it("tells subscribers about the drop, rather than waiting to be asked", () => {
    // `ConnectionStatus` reads this through `useSyncExternalStore`, so a
    // transition nobody is told about is a banner that never appears — which
    // is exactly how every player and GM came to see a permanent
    // "Connecting…" while live sync worked perfectly.
    noteSocketConnected();
    const seen: string[] = [];
    const unsubscribe = subscribeToLiveSyncState((state) =>
      seen.push(state.status),
    );
    noteSocketClosed();
    noteSocketConnected();
    unsubscribe();
    expect(seen).toEqual(["reconnecting", "live"]);
  });

  it("stops telling a subscriber that has unsubscribed", () => {
    noteSocketConnected();
    const seen: string[] = [];
    const unsubscribe = subscribeToLiveSyncState((state) =>
      seen.push(state.status),
    );
    unsubscribe();
    noteSocketClosed();
    expect(seen).toEqual([]);
  });
});
