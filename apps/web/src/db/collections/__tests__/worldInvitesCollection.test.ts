import { describe, expect, it } from "vitest";
import type { WorldInviteDoc } from "../worldInvitesCollection";
import { computeInviteDerivedData, inviteStateLabel } from "../worldInvitesCollection";

/**
 * Spec 027 (T033, FR-010): link-state display.
 *
 * The point of these is the regression they lock in. The previous derivation
 * recomputed validity locally from `used_count`, `max_uses` and `expires_at` —
 * none of which can express revocation. A revoked link therefore reported
 * itself perfectly healthy, which is the worst possible answer for a GM who
 * just killed it to contain a leak.
 */

function doc(overrides: Partial<WorldInviteDoc> = {}): WorldInviteDoc {
  return {
    id: "i1",
    world_id: "w1",
    invite_code: "ABCDEF0123456789ABCD",
    max_uses: 10,
    used_count: 0,
    expires_at: null,
    created_by: "u1",
    created_at: "2026-08-26T00:00:00Z",
    updated_at: "2026-08-26T00:00:00Z",
    state: "ACTIVE",
    remaining_uses: 10,
    rotated_from: null,
    ...overrides,
  };
}

describe("computeInviteDerivedData", () => {
  it("reports an active link as valid, with uses left", () => {
    const derived = computeInviteDerivedData(doc({ used_count: 4, remaining_uses: 6 }));
    expect(derived.is_valid).toBe(true);
    expect(derived.status).toContain("6");
    expect(derived.status).toContain("Active");
  });

  it("reports a revoked link as invalid — the case local derivation could not see", () => {
    // Counts and dates all look healthy. Only `state` reveals the truth.
    const derived = computeInviteDerivedData(
      doc({ state: "REVOKED", used_count: 0, remaining_uses: 10, expires_at: null }),
    );
    expect(derived.is_valid).toBe(false);
    expect(derived.status).toBe("Revoked");
  });

  it("distinguishes expired from exhausted rather than calling both expired", () => {
    // The old panel labelled anything unusable "Expired", regardless of why.
    expect(computeInviteDerivedData(doc({ state: "EXPIRED" })).status).toBe("Expired");
    expect(computeInviteDerivedData(doc({ state: "EXHAUSTED" })).status).toBe(
      "All uses claimed",
    );
  });

  it("handles an uncapped link without inventing a remaining count", () => {
    const derived = computeInviteDerivedData(
      doc({ max_uses: 0, used_count: 3, remaining_uses: null }),
    );
    expect(derived.is_valid).toBe(true);
    expect(derived.status).toContain("unlimited");
  });

  it("falls back to computing the remainder when the server omits it", () => {
    const derived = computeInviteDerivedData(
      doc({ max_uses: 10, used_count: 7, remaining_uses: undefined }),
    );
    expect(derived.status).toContain("3");
  });

  it("never reports a link valid on a state it does not recognise", () => {
    // Forward compatibility: a new server state must fail closed in the UI,
    // not render as working.
    const derived = computeInviteDerivedData(
      doc({ state: "SOMETHING_NEW" as WorldInviteDoc["state"] }),
    );
    expect(derived.is_valid).toBe(false);
    expect(derived.status).toBe("Unavailable");
  });
});

describe("inviteStateLabel", () => {
  it("labels every known state", () => {
    expect(inviteStateLabel("ACTIVE")).toBe("Active");
    expect(inviteStateLabel("EXPIRED")).toBe("Expired");
    expect(inviteStateLabel("EXHAUSTED")).toBe("All uses claimed");
    expect(inviteStateLabel("REVOKED")).toBe("Revoked");
  });

  it("does not describe the use cap as a security control", () => {
    // ADR-050: rotation resets the count, so a DM can rotate indefinitely.
    // The cap is a convenience control and GM-facing copy must not imply
    // otherwise.
    const labels = (["ACTIVE", "EXPIRED", "EXHAUSTED", "REVOKED"] as const).map(
      inviteStateLabel,
    );
    for (const label of labels) {
      expect(label.toLowerCase()).not.toMatch(/limit|secure|protect|enforce/);
    }
  });
});
