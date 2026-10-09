import { describe, expect, it } from "vitest";

import type { WorldInviteDoc } from "@/db/collections/worldInvitesCollection";

import {
  DEFAULT_LINK_CHOICE,
  describeUses,
  linkOptionsFrom,
  splitLinks,
} from "../worldLinks";

const now = new Date("2026-10-09T12:00:00Z");

function link(overrides: Partial<WorldInviteDoc>): WorldInviteDoc {
  return {
    id: "link",
    world_id: "world",
    invite_code: "CODE",
    max_uses: null,
    used_count: 0,
    expires_at: null,
    created_by: "gm",
    created_at: "2026-10-09T10:00:00Z",
    updated_at: "2026-10-09T10:00:00Z",
    state: "ACTIVE",
    remaining_uses: null,
    ...overrides,
  };
}

describe("spec 088 FR-002: what a GM chooses for a link", () => {
  it("defaults to no limit and seven days, leaving both to the server", () => {
    expect(DEFAULT_LINK_CHOICE).toEqual({ limit: null, expiry: "7d" });
    expect(linkOptionsFrom(DEFAULT_LINK_CHOICE, now)).toEqual({});
  });

  it("sends a limit only when the GM sets one", () => {
    expect(linkOptionsFrom({ limit: 1, expiry: "7d" }, now)).toEqual({
      maxUses: 1,
    });
  });

  it("turns 1 day and 30 days into a date, and never into null", () => {
    expect(linkOptionsFrom({ limit: null, expiry: "1d" }, now)).toEqual({
      expiresAt: "2026-10-10T12:00:00.000Z",
    });
    expect(linkOptionsFrom({ limit: null, expiry: "30d" }, now)).toEqual({
      expiresAt: "2026-11-08T12:00:00.000Z",
    });
    expect(linkOptionsFrom({ limit: null, expiry: "never" }, now)).toEqual({
      expiresAt: null,
    });
  });
});

describe("spec 088 FR-004: the list", () => {
  it("puts working links first and folds the rest under past links", () => {
    const { active, past } = splitLinks([
      link({ id: "revoked", state: "REVOKED" }),
      link({ id: "open" }),
      link({ id: "expired", state: "EXPIRED" }),
      link({ id: "used", state: "EXHAUSTED" }),
    ]);
    expect(active.map((l) => l.id)).toEqual(["open"]);
    expect(past.map((l) => l.id)).toEqual(["revoked", "expired", "used"]);
  });

  it("counts joins, and the uses left only on a limited link", () => {
    expect(describeUses(link({ used_count: 0 }))).toBe("No joins yet");
    expect(describeUses(link({ used_count: 1 }))).toBe("1 join");
    expect(
      describeUses(link({ used_count: 2, max_uses: 5, remaining_uses: 3 })),
    ).toBe("2 joins · 3 of 5 uses left");
    expect(
      describeUses(
        link({
          used_count: 1,
          max_uses: 1,
          remaining_uses: 0,
          state: "EXHAUSTED",
        }),
      ),
    ).toBe("1 join · 0 of 1 uses left");
  });
});
