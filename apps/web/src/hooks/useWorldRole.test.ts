import { describe, expect, it } from "vitest";
import { roleIn } from "./useWorldRole";

const world = { createdBy: "creator" };

describe("roleIn", () => {
  it("is nobody while no members are known and the caller did not create the world", () => {
    expect(roleIn([], "player", world)).toBeNull();
  });

  it("is the creator's Owner fallback without a membership row", () => {
    expect(roleIn([], "creator", world)).toBe("Owner");
  });

  it("is the caller's membership when one is known", () => {
    const members = [{ user_id: "player", role: "Player" as const }];
    expect(roleIn(members, "player", world)).toBe("Player");
  });

  it("is nobody when signed out", () => {
    expect(roleIn([], null, world)).toBeNull();
  });
});
