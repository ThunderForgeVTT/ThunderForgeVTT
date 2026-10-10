import { describe, expect, it } from "vitest";

import { dropCleared, upsertRoll } from "@/hooks/useWorldRolls";
import type { MaskedRollRecord, WorldRollRecord } from "@/types/roll";

const masked = (id: string, createdAt: string): MaskedRollRecord => ({
  __typename: "MaskedRoll",
  id,
  rollerName: "Ana",
  createdAt,
  visibility: "GM_EYES",
});

describe("upsertRoll", () => {
  it("keeps the feed oldest first whatever order rolls arrive in", () => {
    const later = masked("b", "2026-10-07T12:01:00Z");
    const earlier = masked("a", "2026-10-07T12:00:00Z");
    expect(upsertRoll([later], earlier).map((r) => r.id)).toEqual(["a", "b"]);
  });

  it("replaces a revealed roll where it stood, rather than adding it", () => {
    const first = masked("a", "2026-10-07T12:00:00Z");
    const second = masked("b", "2026-10-07T12:01:00Z");
    const revealed: WorldRollRecord = {
      __typename: "WorldRoll",
      id: "a",
      rollerId: "u",
      rollerName: "Ana",
      label: null,
      formula: "1d20",
      bindings: [],
      resolution: {
        formula: "1d20",
        dice: [],
        resultKind: "TOTAL",
        resultValue: 9,
      },
      visibility: "GM_EYES",
      createdAt: first.createdAt,
      revealedAt: "2026-10-07T12:05:00Z",
      revealedByName: "GM",
      facets: [],
      rerollOf: null,
      rerolledBy: null,
      spent: null,
      rerollOffers: [],
      rerollUntil: null,
    };
    const feed = upsertRoll([first, second], revealed);
    expect(feed).toHaveLength(2);
    expect(feed[0]).toBe(revealed);
  });
});

/** Spec 088 FR-044: a clear drops every roll made at or before it. */
describe("dropCleared", () => {
  const feed = [
    masked("a", "2026-10-07T12:00:00.123455Z"),
    masked("b", "2026-10-07T12:00:00.123456Z"),
    masked("c", "2026-10-07T12:00:00.123457Z"),
  ];

  it("drops at and before the clear, to the microsecond", () => {
    const left = dropCleared(feed, "2026-10-07T12:00:00.123456Z");
    expect(left.map((r) => r.id)).toEqual(["c"]);
  });

  it("reads the server's whole seconds and the demo's zoneless times alike", () => {
    expect(dropCleared(feed, "2026-10-07T12:00:00Z").map((r) => r.id)).toEqual([
      "a",
      "b",
      "c",
    ]);
    expect(
      dropCleared(
        [masked("d", "2026-10-07T12:00:00.5Z")],
        "2026-10-07T12:00:00.500000Z",
      ),
    ).toEqual([]);
  });

  it("leaves the feed as it is when nothing has been cleared", () => {
    expect(dropCleared(feed, null)).toBe(feed);
  });
});
