import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Offline changes met by a pause on reconnect (spec 051 US2, T038, research
 * R5, FR-023).
 *
 * A paused world refuses the whole batch with `PLAY_PAUSED`. The changes are
 * discarded, never held for later: applied days after a lift, they would be
 * edits made against a world an operator stopped. So the refusal goes through
 * the same revert every refusal does, and the notice is told how many.
 */

const submitQueuedChanges = vi.fn();
vi.mock("@/api/reconcile", () => ({
  submitQueuedChanges: (...args: unknown[]) => submitQueuedChanges(...args),
}));

const readQueuedChanges = vi.fn();
const forgetReconciledChanges = vi.fn();
vi.mock("@/engine/bevy", () => ({
  readQueuedChanges: (...args: unknown[]) => readQueuedChanges(...args),
  queueOfflineChange: vi.fn(),
  forgetReconciledChanges: (...args: unknown[]) =>
    forgetReconciledChanges(...args),
}));

vi.mock("../heartbeat", () => ({ isHeartbeatOffline: () => true }));

const { reconcileWorld } = await import("../offlineQueue");
const {
  onChangesNotKept,
  onPlayPaused,
  resetPlayPausedForTests,
  takeChangesNotKept,
} = await import("@/api/playPauseSignal");

const WORLD = "11111111-1111-4111-8111-111111111111";
const TOKEN_A = "22222222-2222-4222-8222-222222222222";
const TOKEN_B = "33333333-3333-4333-8333-333333333333";

const queued = (localId: string, tokenId: string) => ({
  localId,
  command: { type: "upsert_token", token: { id: tokenId, x: 1, y: 2 } },
});

beforeEach(() => {
  vi.clearAllMocks();
  resetPlayPausedForTests();
  forgetReconciledChanges.mockResolvedValue(0);
});

describe("reconciling against a paused world", () => {
  it("reverts every refused change, forgets it, counts it and sends the table to the notice", async () => {
    readQueuedChanges
      .mockResolvedValueOnce([queued("a", TOKEN_A), queued("b", TOKEN_B)])
      .mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([
      { localId: "a", applied: false, reason: "PLAY_PAUSED" },
      { localId: "b", applied: false, reason: "PLAY_PAUSED" },
    ]);
    const revert = vi.fn();
    const paused: string[] = [];
    const told: string[] = [];
    onPlayPaused((worldId) => paused.push(worldId));
    onChangesNotKept((worldId) => told.push(worldId));

    const report = await reconcileWorld(WORLD, { revert });

    expect(report?.applied).toEqual([]);
    expect(report?.rejected).toHaveLength(2);
    expect(report?.notKeptForPause).toBe(2);
    // Discarded, not kept for another try after a lift.
    expect(report?.stillQueued).toEqual([]);
    expect(forgetReconciledChanges).toHaveBeenCalledWith([
      { localId: "a", applied: false },
      { localId: "b", applied: false },
    ]);
    // The existing revert path, the same as for any refusal.
    expect(revert).toHaveBeenCalledWith([TOKEN_A, TOKEN_B]);
    expect(report?.reverted).toEqual([TOKEN_A, TOKEN_B]);

    expect(paused).toEqual([WORLD]);
    expect(told).toEqual([WORLD]);
    expect(takeChangesNotKept(WORLD)).toBe(2);
  });

  it("counts only the pause's refusals, and says nothing of a pause when there is none", async () => {
    readQueuedChanges
      .mockResolvedValueOnce([queued("a", TOKEN_A), queued("b", TOKEN_B)])
      .mockResolvedValue([]);
    submitQueuedChanges.mockResolvedValue([
      { localId: "a", applied: false, reason: "GONE_AWAY" },
      { localId: "b", applied: true },
    ]);
    const paused: string[] = [];
    onPlayPaused((worldId) => paused.push(worldId));

    const report = await reconcileWorld(WORLD, { revert: vi.fn() });

    expect(report?.notKeptForPause).toBe(0);
    expect(paused).toEqual([]);
    expect(takeChangesNotKept(WORLD)).toBe(0);
  });
});
