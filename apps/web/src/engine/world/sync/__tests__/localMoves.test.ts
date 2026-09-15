import { beforeEach, describe, expect, it } from "vitest";

import {
  beginLocalMove,
  isPositionStale,
  markRead,
  resetLocalMovesForTests,
} from "../localMoves";

/**
 * When a read of the scene is older than this client's own move (spec 045
 * T065): a player stepping twice must not have the first step's echo put the
 * token back a cell after the second has left.
 */
describe("localMoves", () => {
  beforeEach(() => resetLocalMovesForTests());

  it("a read sent while a move is unanswered is stale for that token", () => {
    const settle = beginLocalMove("brom");
    const mark = markRead();
    expect(isPositionStale("brom", mark)).toBe(true);
    settle();
    expect(isPositionStale("brom", mark), "answered after the read left").toBe(
      true,
    );
  });

  it("a read sent after a move was answered is not stale", () => {
    beginLocalMove("brom")();
    expect(isPositionStale("brom", markRead())).toBe(false);
  });

  it("a read sent before a move and answered while it is unanswered is stale", () => {
    const mark = markRead();
    beginLocalMove("brom");
    expect(isPositionStale("brom", mark)).toBe(true);
  });

  it("only the moved token is kept", () => {
    const mark = markRead();
    beginLocalMove("brom")();
    expect(isPositionStale("aria", mark)).toBe(false);
  });

  it("counts overlapping moves, and settling twice counts once", () => {
    const first = beginLocalMove("brom");
    const second = beginLocalMove("brom");
    first();
    first();
    const mark = markRead();
    expect(isPositionStale("brom", mark), "the second is still out").toBe(true);
    second();
    expect(isPositionStale("brom", markRead())).toBe(false);
  });
});
