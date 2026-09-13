import { describe, expect, it } from "vitest";
import {
  assignableRoles,
  canManageRole,
  sortMembersByRole,
  type WorldMemberDoc,
} from "../worldMembersCollection";
import {
  WORLD_ROLES,
  isWorldMemberRole,
  managesContent,
  roleRank,
  runsTheWorld,
} from "@/types/world";

/**
 * ADR-099, on the client: the fourth role is ranked where the server ranks
 * it, and every question a component asks about roles gives the server's
 * answer. The server decides; these keep the page from offering a control
 * the server would refuse, or hiding one it would allow.
 */

describe("the world role ranking", () => {
  it("runs Player < TrustedPlayer < GM < Owner, as thunderforge_authz does", () => {
    expect(WORLD_ROLES).toEqual(["Player", "TrustedPlayer", "GM", "Owner"]);
    expect(roleRank("TrustedPlayer")).toBeGreaterThan(roleRank("Player"));
    expect(roleRank("GM")).toBeGreaterThan(roleRank("TrustedPlayer"));
    expect(roleRank(null)).toBeLessThan(roleRank("Player"));
  });

  it("reads a spelling it does not know as no role, not as a Player", () => {
    for (const wrong of [
      "trustedplayer",
      "Trusted Player",
      "gm",
      "",
      "Admin",
    ]) {
      expect(isWorldMemberRole(wrong)).toBe(false);
    }
    for (const role of WORLD_ROLES) {
      expect(isWorldMemberRole(role)).toBe(true);
    }
  });

  it("puts a Trusted Player on the Player side of running the world", () => {
    expect(runsTheWorld("Owner")).toBe(true);
    expect(runsTheWorld("GM")).toBe(true);
    expect(runsTheWorld("TrustedPlayer")).toBe(false);
    expect(runsTheWorld("Player")).toBe(false);
    expect(runsTheWorld(null)).toBe(false);
  });

  it("trusts a Trusted Player with content and a Player with none", () => {
    expect(managesContent("Owner")).toBe(true);
    expect(managesContent("GM")).toBe(true);
    expect(managesContent("TrustedPlayer")).toBe(true);
    expect(managesContent("Player")).toBe(false);
    expect(managesContent(null)).toBe(false);
  });
});

describe("managing members", () => {
  it("lets a Game Master manage a Trusted Player, and neither kind of player manage anyone", () => {
    expect(canManageRole("Owner", "GM")).toBe(true);
    expect(canManageRole("GM", "TrustedPlayer")).toBe(true);
    expect(canManageRole("GM", "GM")).toBe(false);
    expect(canManageRole("GM", "Owner")).toBe(false);
    for (const target of WORLD_ROLES) {
      expect(canManageRole("TrustedPlayer", target)).toBe(false);
      expect(canManageRole("Player", target)).toBe(false);
    }
  });

  it("offers only roles at or below the caller's own", () => {
    expect(assignableRoles("Owner")).toEqual([
      "Owner",
      "GM",
      "TrustedPlayer",
      "Player",
    ]);
    expect(assignableRoles("GM")).toEqual(["GM", "TrustedPlayer", "Player"]);
    expect(assignableRoles("TrustedPlayer")).toEqual([]);
    expect(assignableRoles("Player")).toEqual([]);
  });

  it("sorts a roster highest role first", () => {
    const member = (role: WorldMemberDoc["role"]): WorldMemberDoc => ({
      id: role,
      world_id: "w",
      user_id: role,
      role,
      joined_at: "",
      created_at: "",
      updated_at: "",
    });
    const sorted = sortMembersByRole([
      member("Player"),
      member("Owner"),
      member("TrustedPlayer"),
      member("GM"),
    ]).map((m) => m.role);
    expect(sorted).toEqual(["Owner", "GM", "TrustedPlayer", "Player"]);
  });
});
