import { describe, expect, it } from "vitest";
import {
  HEARTBEAT_FAILURES_BEFORE_OFFLINE,
  HEARTBEAT_INTERVAL_MS,
  isOfflineAfter,
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
    const clientVerdictMs = HEARTBEAT_FAILURES_BEFORE_OFFLINE * HEARTBEAT_INTERVAL_MS;
    const serverTimeoutMs = 15_000;

    expect(clientVerdictMs).toBeLessThanOrEqual(serverTimeoutMs);
  });
});
