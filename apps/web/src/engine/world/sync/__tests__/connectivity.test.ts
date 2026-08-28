import { describe, expect, it } from "vitest";
import { connectivityFor, isDisconnected } from "../subscriptionClient";

/**
 * When a dropped connection stops being a blip and starts being an outage
 * (spec 028 US7, T073).
 *
 * The socket wiring around this needs a real WebSocket and is exercised by
 * `world-cache-offline.spec.ts`. What is here is the judgement: the same
 * underlying situation — no connection — has to read as "reconnecting" for a
 * moment and "disconnected" after that, because the second one is when edits
 * start being queued and the user has to be told.
 */
describe("connectivityFor", () => {
  it("stays quiet through a brief drop", () => {
    expect(
      connectivityFor({ hasConnectedOnce: true, attempt: 1, browserOffline: false }),
    ).toEqual({ status: "reconnecting", attempt: 1 });
    expect(
      connectivityFor({ hasConnectedOnce: true, attempt: 2, browserOffline: false }),
    ).toEqual({ status: "reconnecting", attempt: 2 });
  });

  it("calls it a disconnection once retrying stops looking temporary", () => {
    expect(
      connectivityFor({ hasConnectedOnce: true, attempt: 3, browserOffline: false }),
    ).toEqual({ status: "disconnected", attempt: 3, browserOffline: false });
  });

  /**
   * `navigator.onLine === false` means there is no interface up, so no amount
   * of backoff will help — waiting out three attempts before saying so would
   * leave the user watching "reconnecting" for seconds while their machine
   * already knows better.
   */
  it("believes the browser immediately when it says there is no network", () => {
    expect(
      connectivityFor({ hasConnectedOnce: true, attempt: 0, browserOffline: true }),
    ).toEqual({ status: "disconnected", attempt: 0, browserOffline: true });
  });

  /**
   * The converse is not symmetric: `onLine` true means an interface exists,
   * not that the server is reachable. A machine on a café wifi with no route
   * out is online by that measure and disconnected by every useful one.
   */
  it("does not treat an interface being up as the server being reachable", () => {
    expect(
      connectivityFor({ hasConnectedOnce: true, attempt: 5, browserOffline: false }),
    ).toEqual({ status: "disconnected", attempt: 5, browserOffline: false });
  });

  /**
   * A first handshake that has not landed yet is `connecting`, not a
   * reconnection — there is nothing to have been disconnected from, and an
   * attempt count shown on a page that has never loaded is noise.
   */
  it("keeps a first handshake in connecting, however many attempts it takes", () => {
    expect(
      connectivityFor({ hasConnectedOnce: false, attempt: 9, browserOffline: false }),
    ).toEqual({ status: "connecting" });
  });

  /** But a browser reporting no network at all outranks even that. */
  it("reports no-network during a first handshake too", () => {
    expect(
      connectivityFor({ hasConnectedOnce: false, attempt: 0, browserOffline: true }).status,
    ).toBe("disconnected");
  });
});

describe("isDisconnected", () => {
  it("is what decides whether an edit is queued rather than sent", () => {
    expect(isDisconnected({ status: "live" })).toBe(false);
    expect(isDisconnected({ status: "connecting" })).toBe(false);
    expect(isDisconnected({ status: "reconnecting", attempt: 2 })).toBe(false);
    expect(
      isDisconnected({ status: "disconnected", attempt: 3, browserOffline: false }),
    ).toBe(true);
  });
});
