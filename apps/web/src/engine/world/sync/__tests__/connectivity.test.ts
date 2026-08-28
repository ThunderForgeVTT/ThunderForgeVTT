import { describe, expect, it } from "vitest";
import {
  connectivityFor,
  isDisconnected,
  isServerIsolated,
  peerAdjudicationAvailable,
} from "../subscriptionClient";

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
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 1,
        browserOffline: false,
      }),
    ).toEqual({ status: "reconnecting", attempt: 1 });
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 2,
        browserOffline: false,
      }),
    ).toEqual({ status: "reconnecting", attempt: 2 });
  });

  it("calls it a disconnection once retrying stops looking temporary", () => {
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 3,
        browserOffline: false,
      }),
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
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 0,
        browserOffline: true,
      }),
    ).toEqual({ status: "disconnected", attempt: 0, browserOffline: true });
  });

  /**
   * The converse is not symmetric: `onLine` true means an interface exists,
   * not that the server is reachable. A machine on a café wifi with no route
   * out is online by that measure and disconnected by every useful one.
   */
  it("does not treat an interface being up as the server being reachable", () => {
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 5,
        browserOffline: false,
      }),
    ).toEqual({ status: "disconnected", attempt: 5, browserOffline: false });
  });

  /**
   * A first handshake that has not landed yet is `connecting`, not a
   * reconnection — there is nothing to have been disconnected from, and an
   * attempt count shown on a page that has never loaded is noise.
   */
  it("keeps a first handshake in connecting, however many attempts it takes", () => {
    expect(
      connectivityFor({
        hasConnectedOnce: false,
        attempt: 9,
        browserOffline: false,
      }),
    ).toEqual({ status: "connecting" });
  });

  /** But a browser reporting no network at all outranks even that. */
  it("reports no-network during a first handshake too", () => {
    expect(
      connectivityFor({
        hasConnectedOnce: false,
        attempt: 0,
        browserOffline: true,
      }).status,
    ).toBe("disconnected");
  });
});

describe("isDisconnected", () => {
  it("is what decides whether an edit is queued rather than sent", () => {
    expect(isDisconnected({ status: "live" })).toBe(false);
    expect(isDisconnected({ status: "connecting" })).toBe(false);
    expect(isDisconnected({ status: "reconnecting", attempt: 2 })).toBe(false);
    expect(
      isDisconnected({
        status: "disconnected",
        attempt: 3,
        browserOffline: false,
      }),
    ).toBe(true);
  });
});

/**
 * The third state (spec 028 US7, T096, FR-055 to FR-059).
 *
 * `server-isolated` is what lets a table keep playing when the server goes
 * away and everyone can still see each other. Every test below is really
 * about the *cost* of the two rules that define it — full peer connectivity
 * rather than a quorum, and the Game Master specifically with no election —
 * because both rules stop play in cases a looser rule would allow, and a
 * future change that allows one more case is the failure they exist to catch.
 */
describe("peerAdjudicationAvailable", () => {
  it("needs every participant, not most of them", () => {
    // The split-brain rule. A quorum would let two subsets each satisfy a
    // majority and both make progress, leaving two histories that cannot be
    // merged afterwards without destroying somebody's work.
    expect(
      peerAdjudicationAvailable({
        expected: 4,
        reachable: 3,
        gmReachable: true,
      }),
    ).toBe(false);
    expect(
      peerAdjudicationAvailable({
        expected: 4,
        reachable: 4,
        gmReachable: true,
      }),
    ).toBe(true);
  });

  it("needs the Game Master among them, and promotes nobody in their place", () => {
    // FR-059. There is deliberately no election to test for: a promotion
    // would mean two adjudicators in one session and no single chain of
    // authority.
    expect(
      peerAdjudicationAvailable({
        expected: 3,
        reachable: 3,
        gmReachable: false,
      }),
    ).toBe(false);
  });

  it("treats a table of one as offline rather than isolated", () => {
    // There is nobody to adjudicate among, and the ordinary offline path —
    // queue and reconcile — is already the right answer.
    expect(
      peerAdjudicationAvailable({
        expected: 0,
        reachable: 0,
        gmReachable: true,
      }),
    ).toBe(false);
  });

  it("says no when it does not know who was at the table", () => {
    // Peer transfer turned off, or a client that never learned the roster.
    // Not knowing has to read as "no", which is what makes turning peer
    // transfer off forfeit server-isolated play rather than silently keep it.
    expect(peerAdjudicationAvailable(null)).toBe(false);
    expect(peerAdjudicationAvailable(undefined)).toBe(false);
  });
});

describe("connectivityFor, with peers", () => {
  const wholeTable = { expected: 3, reachable: 3, gmReachable: true };

  it("keeps a socket blip out of it while the server is still answering", () => {
    // The failure this catches is the important one: peer-adjudicated play
    // starting because `graphql-ws` dropped a lazy connection. The heartbeat
    // is the only signal that means the server is gone, and without it this
    // is an ordinary reconnection.
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 5,
        browserOffline: false,
        serverUnreachable: false,
        peers: wholeTable,
      }),
    ).toEqual({ status: "disconnected", attempt: 5, browserOffline: false });
  });

  it("is server-isolated when the server is gone and the whole table is here", () => {
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 3,
        browserOffline: false,
        serverUnreachable: true,
        peers: wholeTable,
      }),
    ).toEqual({ status: "server-isolated", attempt: 3, peers: 3 });
  });

  it("believes open peer connections over the browser's opinion of the network", () => {
    // `navigator.onLine` describes a route to the internet. Direct channels
    // to every person at the table are evidence of a working local network,
    // which is the only network this state needs — a household or an office
    // whose uplink died is exactly the case the feature exists for.
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 1,
        browserOffline: true,
        serverUnreachable: true,
        peers: wholeTable,
      }).status,
    ).toBe("server-isolated");
  });

  it("drops back to offline the moment one peer goes", () => {
    // FR-058. Not a degraded mode, not a smaller table: the same plain
    // offline state a client with no peers at all is in.
    expect(
      connectivityFor({
        hasConnectedOnce: true,
        attempt: 3,
        browserOffline: false,
        serverUnreachable: true,
        peers: { expected: 3, reachable: 2, gmReachable: true },
      }),
    ).toEqual({ status: "disconnected", attempt: 3, browserOffline: false });
  });

  it("leaves both halves of a partition offline, with neither winning", () => {
    // The larger half is not privileged. Both sides see a table that is not
    // all present, and both stop.
    const larger = connectivityFor({
      hasConnectedOnce: true,
      attempt: 3,
      browserOffline: false,
      serverUnreachable: true,
      peers: { expected: 3, reachable: 2, gmReachable: true },
    });
    const smaller = connectivityFor({
      hasConnectedOnce: true,
      attempt: 3,
      browserOffline: false,
      serverUnreachable: true,
      peers: { expected: 3, reachable: 1, gmReachable: false },
    });
    expect(larger.status).toBe("disconnected");
    expect(smaller.status).toBe("disconnected");
  });

  it("never reports isolation before this tab has ever been connected", () => {
    // A first handshake that has not landed cannot have a session roster
    // behind it, and calling that state "playing without the server" would be
    // claiming a table that was never assembled.
    expect(
      connectivityFor({
        hasConnectedOnce: false,
        attempt: 0,
        browserOffline: false,
        serverUnreachable: true,
        peers: wholeTable,
      }).status,
    ).toBe("connecting");
  });
});

describe("isServerIsolated", () => {
  it("is what decides whether a move is put to the table instead of the server", () => {
    expect(
      isServerIsolated({ status: "server-isolated", attempt: 3, peers: 3 }),
    ).toBe(true);
    expect(isServerIsolated({ status: "live" })).toBe(false);
    expect(
      isServerIsolated({
        status: "disconnected",
        attempt: 3,
        browserOffline: false,
      }),
    ).toBe(false);
  });

  it("still queues the edit, because adjudication is provisional", () => {
    // The failure this catches would be silent and permanent: a change the
    // table agreed, applied on every screen, that skipped the outbox and so
    // was never submitted, re-authorized, or recorded (FR-062).
    expect(
      isDisconnected({ status: "server-isolated", attempt: 3, peers: 3 }),
    ).toBe(true);
  });
});
