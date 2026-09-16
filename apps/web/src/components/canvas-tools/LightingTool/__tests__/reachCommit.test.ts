import { describe, expect, it } from "vitest";
import {
  afterCommit,
  currentReaches,
  PENDING_MS,
  reachChanges,
  stillPending,
  type PendingReaches,
  type Reaches,
} from "../reachCommit";

/**
 * Spec 045 FR-061, at the seam an e2e run reaches only by luck of timing.
 *
 * The pre-playtest run of 2026-09-15 stored a light at 40 ft dim and 5 ft
 * bright after the Game Master typed 40 and then 20: the bright commit was
 * judged against the 10 ft dim reach the server had not yet replaced, and
 * refused. These pin the rule that fixed it: a committed reach stands in for
 * the stored one until the store moves, and goes out with the other reach.
 */

// 10 ft dim, 5 ft bright, in world units at 50 per five-foot square.
const STORED: Reaches = { radius: 100, brightRadius: 50 };

describe("a reach on its way to the server", () => {
  it("is what the panel judges the other reach against", () => {
    const pending = afterCommit({ radius: 400 }, STORED, {});
    expect(currentReaches(STORED, pending)).toEqual({
      radius: 400,
      brightRadius: 50,
    });
  });

  it("goes out again with the other reach, so the server clamps against neither's old value", () => {
    const pending = afterCommit({ radius: 400 }, STORED, {});
    expect(reachChanges({ brightRadius: 200 }, STORED, pending)).toEqual({
      radius: 400,
      brightRadius: 200,
    });
  });

  it("sends nothing when nothing was typed", () => {
    const pending = afterCommit({ radius: 400 }, STORED, {});
    expect(reachChanges({}, STORED, pending)).toEqual({});
  });

  it("stops standing in once it is older than an answer could be", () => {
    const pending = afterCommit({ radius: 400 }, STORED, {}, 1_000);
    expect(currentReaches(STORED, pending, 1_000 + PENDING_MS)).toEqual(STORED);
  });

  it("is settled once the store moves, whether to it or to someone else's reach", () => {
    const pending = afterCommit({ radius: 400 }, STORED, {});
    expect(stillPending({ radius: 400, brightRadius: 50 }, pending)).toEqual(
      {},
    );
    expect(stillPending({ radius: 300, brightRadius: 50 }, pending)).toEqual(
      {},
    );
    expect(currentReaches({ radius: 300, brightRadius: 50 }, pending)).toEqual({
      radius: 300,
      brightRadius: 50,
    });
  });

  it("replaced before it lands, still waits on the reach first stored", () => {
    const first = afterCommit({ radius: 400 }, STORED, {});
    const second: PendingReaches = afterCommit({ radius: 300 }, STORED, first);
    expect(second.radius).toMatchObject({ value: 300, from: 100 });
  });

  it("keeps both when both are on their way", () => {
    const first = afterCommit({ radius: 400 }, STORED, {});
    const changes = reachChanges({ brightRadius: 200 }, STORED, first);
    const both = afterCommit(changes, STORED, first);
    expect(currentReaches(STORED, both)).toEqual({
      radius: 400,
      brightRadius: 200,
    });
    // The server's answer to the first moves only the dim reach.
    expect(currentReaches({ radius: 400, brightRadius: 50 }, both)).toEqual({
      radius: 400,
      brightRadius: 200,
    });
  });
});
