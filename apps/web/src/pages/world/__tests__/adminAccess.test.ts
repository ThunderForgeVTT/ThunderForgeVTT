import { describe, expect, it } from "vitest";
import { reachedThroughAdminAccess } from "@/pages/world/adminAccess";

describe("reachedThroughAdminAccess", () => {
  it("is false for a player, a trusted player and a Game Master who did not create the world", () => {
    for (const role of ["Player", "TrustedPlayer", "GM"] as const) {
      expect(
        reachedThroughAdminAccess({ isAdmin: false, role, roleLoading: false }),
      ).toBe(false);
    }
  });

  it("is false for a site administrator who is a member of the world", () => {
    expect(
      reachedThroughAdminAccess({
        isAdmin: true,
        role: "Player",
        roleLoading: false,
      }),
    ).toBe(false);
  });

  it("is true for a site administrator who holds no role in the world", () => {
    expect(
      reachedThroughAdminAccess({
        isAdmin: true,
        role: null,
        roleLoading: false,
      }),
    ).toBe(true);
  });

  it("says nothing while the roster is still loading", () => {
    expect(
      reachedThroughAdminAccess({
        isAdmin: true,
        role: null,
        roleLoading: true,
      }),
    ).toBe(false);
  });

  it("is false for someone who is neither an administrator nor a member", () => {
    expect(
      reachedThroughAdminAccess({
        isAdmin: false,
        role: null,
        roleLoading: false,
      }),
    ).toBe(false);
  });
});
