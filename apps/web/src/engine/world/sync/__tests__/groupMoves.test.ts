import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/**
 * Spec 085: a group's answers are tallied into one message. Five tokens
 * moved together and two refused is one toast, not two, and none at all
 * when every answer agreed.
 */

const warning = vi.fn();
vi.mock("sonner", () => ({
  toast: { warning: (...a: unknown[]) => warning(...a) },
}));

const { settleGroup, forgetAllGroups } = await import("../groupMoves");

beforeEach(() => {
  warning.mockClear();
  forgetAllGroups();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("settleGroup", () => {
  it("says nothing when every answer was yes", () => {
    const stamp = { id: "g-1", size: 3 };
    settleGroup(stamp, true, "moved");
    settleGroup(stamp, true, "moved");
    settleGroup(stamp, true, "moved");
    expect(warning).not.toHaveBeenCalled();
  });

  it("reports the refused ones once, when the last answer is in", () => {
    const stamp = { id: "g-2", size: 5 };
    settleGroup(stamp, false, "moved");
    settleGroup(stamp, true, "moved");
    settleGroup(stamp, false, "moved");
    settleGroup(stamp, true, "moved");
    expect(warning).not.toHaveBeenCalled();
    settleGroup(stamp, true, "moved");
    expect(warning).toHaveBeenCalledTimes(1);
    expect(warning).toHaveBeenCalledWith("2 of 5 could not be moved.");
  });

  it("counts the members the board held back with the server's refusals", () => {
    // Two tokens moved; one crossed a wall on a player's board and was never
    // sent, so one answer comes back.
    const stamp = { id: "g-4", size: 1, refused: 1 };
    settleGroup(stamp, true, "moved");
    expect(warning).toHaveBeenCalledTimes(1);
    expect(warning).toHaveBeenCalledWith("1 of 2 could not be moved.");
  });

  it("ignores an answer that carries no stamp", () => {
    settleGroup(undefined, false, "deleted");
    expect(warning).not.toHaveBeenCalled();
  });

  it("drops a tally left unanswered for 30 s, without a word", () => {
    vi.useFakeTimers();
    const stamp = { id: "g-3", size: 2 };
    settleGroup(stamp, false, "hidden");
    vi.advanceTimersByTime(30_001);
    // A late answer opens a fresh tally rather than completing the old one.
    settleGroup(stamp, false, "hidden");
    expect(warning).not.toHaveBeenCalled();
  });
});
