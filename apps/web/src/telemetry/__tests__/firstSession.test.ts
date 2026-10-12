import { afterEach, describe, expect, it } from "vitest";
import { ALLOWED_ATTRIBUTES, EVENT_NAMES } from "@thunderforge/telemetry";
import {
  changedBucket,
  expiryBucket,
  joinOutcomeOf,
  maxUsesBucket,
  recordHeroOpened,
  recordJoin,
  recordLinkCreated,
  recordLinkRevoked,
  recordMapImported,
  recordRollsCleared,
  recordSettingsSaved,
  recordUnsavedWarning,
  recordWorldCreated,
  saveOutcome,
  setFirstSessionSink,
  startingMapBucket,
  type FirstSessionAttributes,
} from "../firstSession";

type Recorded = [string, FirstSessionAttributes];

function capture(): Recorded[] {
  const seen: Recorded[] = [];
  setFirstSessionSink((name, attributes) => seen.push([name, attributes]));
  return seen;
}

afterEach(() => setFirstSessionSink(undefined));

describe("buckets", () => {
  it("never sends a link's own use limit", () => {
    expect(maxUsesBucket(1)).toBe("1");
    expect(maxUsesBucket(2)).toBe("2-5");
    expect(maxUsesBucket(5)).toBe("2-5");
    expect(maxUsesBucket(6)).toBe("6-50");
    expect(maxUsesBucket(50)).toBe("6-50");
    expect(maxUsesBucket(null)).toBe("none");
  });

  it("names an expiry from the closed set", () => {
    expect(expiryBucket("1d")).toBe("1d");
    expect(expiryBucket("30d")).toBe("30d");
    expect(expiryBucket("never")).toBe("none");
    expect(expiryBucket("2026-10-12")).toBe("none");
  });

  it("maps a refusal code to an outcome and leaves others uncounted", () => {
    expect(joinOutcomeOf("LINK_USED_UP")).toBe("used_up");
    expect(joinOutcomeOf("ALREADY_MEMBER")).toBe("already_member");
    expect(joinOutcomeOf("FORBIDDEN")).toBeUndefined();
  });

  it("says which kind of starting map, never which map", () => {
    expect(startingMapBucket(undefined, "m1")).toBe("default");
    expect(startingMapBucket("m1", "m1")).toBe("default");
    expect(startingMapBucket("m2", "m1")).toBe("other");
    expect(startingMapBucket(null, "m1")).toBe("none");
  });

  it("buckets a save", () => {
    expect(saveOutcome(3, 3)).toBe("all");
    expect(saveOutcome(1, 3)).toBe("partial");
    expect(saveOutcome(0, 3)).toBe("none");
    expect(changedBucket(1)).toBe("1");
    expect(changedBucket(3)).toBe("2-3");
    expect(changedBucket(4)).toBe("4+");
  });
});

describe("recording", () => {
  it("sends only allow-listed names and attributes", () => {
    const seen = capture();
    recordLinkCreated(10, "never");
    recordLinkRevoked();
    recordJoin("joined");
    recordWorldCreated(null, "m1", false);
    recordRollsCleared();
    recordMapImported("uvtt", true);
    recordSettingsSaved("mail", 2, 2);
    recordUnsavedWarning(false);
    recordHeroOpened("builder");

    expect(seen.map(([name]) => name)).toEqual([
      "world_link.created",
      "world_link.revoked",
      "world_link.join",
      "world.created",
      "rolls.cleared",
      "map.imported",
      "settings.saved",
      "settings.unsaved_warning",
      "hero.opened_from_players",
    ]);
    const names: readonly string[] = EVENT_NAMES;
    const keys: readonly string[] = ALLOWED_ATTRIBUTES;
    for (const [name, attributes] of seen) {
      expect(names).toContain(name);
      for (const key of Object.keys(attributes)) expect(keys).toContain(key);
    }
    expect(seen[0][1]).toEqual({ max_uses: "6-50", expiry: "none" });
    expect(seen[3][1]).toEqual({ starting_map: "none", map_error: false });
    expect(seen[6][1]).toEqual({
      section: "mail",
      outcome: "all",
      changed: "2-3",
    });
    expect(seen[7][1]).toEqual({ action: "stayed" });
  });

  it("sends nothing for a save with nothing to save", () => {
    const seen = capture();
    recordSettingsSaved("mail", 0, 0);
    expect(seen).toEqual([]);
  });
});
