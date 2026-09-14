import { beforeEach, describe, expect, it, vi } from "vitest";
import * as graphqlClient from "@/api/graphqlClient";
import { onPlayPaused, resetPlayPausedForTests } from "@/api/playPauseSignal";
import {
  HEARTBEAT_FAILURES_BEFORE_OFFLINE,
  HEARTBEAT_INTERVAL_MS,
  beatOnce,
  getHeartbeatLatencyMs,
  isHeartbeatOffline,
  isOfflineAfter,
  resetHeartbeatForTests,
} from "../heartbeat";

/**
 * When a client decides it cannot reach the server (spec 028 US7).
 *
 * The threshold is the whole judgement, and it is asymmetric on purpose: one
 * failed request is a garbage collection pause or a train tunnel, while
 * announcing an outage that is not there interrupts play and teaches people
 * to ignore the notice. Noticing fifteen seconds late costs nothing anyone
 * can see.
 */
describe("isOfflineAfter", () => {
  it("tolerates a single failed beat", () => {
    expect(isOfflineAfter(0)).toBe(false);
    expect(isOfflineAfter(1)).toBe(false);
  });

  it("calls it offline once failures stop looking incidental", () => {
    expect(isOfflineAfter(HEARTBEAT_FAILURES_BEFORE_OFFLINE)).toBe(true);
    expect(isOfflineAfter(HEARTBEAT_FAILURES_BEFORE_OFFLINE + 5)).toBe(true);
  });

  /**
   * The client's threshold must not outlast the server's, or a Game Master
   * would be told a player dropped while that player still believed they were
   * connected and was happily sending edits into a world that had written
   * them off. `PRESENCE_TIMEOUT_SECS` is 15s server-side; the client reaches
   * its own verdict at 3 beats of 5s.
   */
  it("agrees with the server's presence timeout", () => {
    const clientVerdictMs =
      HEARTBEAT_FAILURES_BEFORE_OFFLINE * HEARTBEAT_INTERVAL_MS;
    const serverTimeoutMs = 15_000;

    expect(clientVerdictMs).toBeLessThanOrEqual(serverTimeoutMs);
  });
});

/**
 * The latency the canvas readout shows.
 *
 * Taken from the heartbeat because the heartbeat is already a round trip on
 * a fixed interval — it needs no probe of its own, and it measures the path
 * the client's own sense of being connected depends on rather than some
 * other endpoint that could be healthy while this one is not.
 */
describe("heartbeat latency", () => {
  beforeEach(() => {
    resetHeartbeatForTests();
    vi.restoreAllMocks();
  });

  it("has no answer before any beat has completed", () => {
    expect(getHeartbeatLatencyMs()).toBeNull();
  });

  it("reports the round trip of a beat that arrived", async () => {
    vi.spyOn(graphqlClient, "postGraphQL").mockResolvedValue({
      heartbeat: true,
    } as never);
    vi.useFakeTimers();
    const start = Date.now();
    vi.setSystemTime(start);

    const beat = beatOnce("world", null);
    vi.setSystemTime(start + 42);
    await beat;

    expect(getHeartbeatLatencyMs()).toBe(42);
    vi.useRealTimers();
  });

  /**
   * The one rule that matters here. Leaving the previous figure on screen
   * while nothing is getting through reads as a working connection — which
   * is the single thing a latency number must never say. Absent beats
   * stale.
   */
  it("forgets the last figure when a beat fails, rather than leaving it standing", async () => {
    const post = vi.spyOn(graphqlClient, "postGraphQL");
    post.mockResolvedValueOnce({ heartbeat: true } as never);
    await beatOnce("world", null);
    expect(getHeartbeatLatencyMs()).not.toBeNull();

    post.mockRejectedValueOnce(new Error("offline"));
    await beatOnce("world", null);

    expect(getHeartbeatLatencyMs()).toBeNull();
  });
});

/**
 * Spec 051 US2 (T037, research R5): a beat refused because an operator paused
 * the world is not a lost connection.
 *
 * Three failed beats switch the client to offline queueing, so a refusal
 * counted as a failure would quietly turn a paused world into offline play:
 * the table keeps moving tokens locally against a world that has stopped.
 */
describe("a heartbeat refused because play is paused", () => {
  const paused = () =>
    new graphqlClient.GraphQLRequestError(
      "Play in this world has been paused by an operator.",
      { operation: "Heartbeat", status: 200, codes: ["WORLD_PLAY_PAUSED"] },
    );

  beforeEach(() => {
    resetHeartbeatForTests();
    resetPlayPausedForTests();
    vi.restoreAllMocks();
  });

  it("announces the pause, and never counts toward going offline", async () => {
    const heard: string[] = [];
    onPlayPaused((worldId) => heard.push(worldId));
    vi.spyOn(graphqlClient, "postGraphQL").mockRejectedValue(paused());

    for (
      let beat = 0;
      beat < HEARTBEAT_FAILURES_BEFORE_OFFLINE * 3;
      beat += 1
    ) {
      expect(await beatOnce("world", null)).toBe(false);
    }

    expect(isHeartbeatOffline()).toBe(false);
    expect(heard).toEqual(["world"]);
  });

  /**
   * The reconnect: a browser offline when the pause landed comes back, and
   * its first beat through is refused. That is the server answering, so the
   * client is connected again, which is what lets it submit its queue and
   * learn the queue was refused too.
   */
  it("brings a client that was offline back online, and clears its failures", async () => {
    const post = vi.spyOn(graphqlClient, "postGraphQL");
    post.mockRejectedValue(new Error("offline"));
    for (let beat = 0; beat < HEARTBEAT_FAILURES_BEFORE_OFFLINE; beat += 1) {
      await beatOnce("world", null);
    }
    expect(isHeartbeatOffline()).toBe(true);

    post.mockRejectedValueOnce(paused());
    await beatOnce("world", null);
    expect(isHeartbeatOffline()).toBe(false);

    // The count started again from nothing: one more network failure is
    // incidental, not the third in a row.
    post.mockRejectedValueOnce(new Error("offline"));
    await beatOnce("world", null);
    expect(isHeartbeatOffline()).toBe(false);
  });
});
