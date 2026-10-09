import { describe, expect, it } from "vitest";

import { joinRefusalOf } from "../joinRefusal";

describe("spec 088 FR-008: the join page says why a link admits no one", () => {
  it("gives each dead link its own heading and the server's message", () => {
    const revoked = joinRefusalOf(
      ["LINK_REVOKED"],
      "The GM has withdrawn this link. Ask them for a new one.",
    );
    expect(revoked).toEqual({
      kind: "LINK_REVOKED",
      heading: "This link was withdrawn",
      message: "The GM has withdrawn this link. Ask them for a new one.",
    });
    expect(joinRefusalOf(["LINK_EXPIRED"], "x")?.heading).toBe(
      "This link has expired",
    );
    expect(joinRefusalOf(["LINK_USED_UP"], "x")?.heading).toBe(
      "This link has been used",
    );
    expect(joinRefusalOf(["LINK_UNKNOWN"], "x")?.heading).toBe(
      "No world behind this link",
    );
  });

  it("reads a member's join as already in, not as a refusal", () => {
    expect(joinRefusalOf(["ALREADY_MEMBER"], "x")?.kind).toBe("ALREADY_MEMBER");
  });

  it("leaves anything else to the caller", () => {
    expect(joinRefusalOf([], "Network down")).toBeNull();
    expect(joinRefusalOf(["RATE_LIMITED"], "Slow down")).toBeNull();
  });
});
