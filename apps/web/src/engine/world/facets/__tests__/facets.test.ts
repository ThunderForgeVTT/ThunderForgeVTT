import { describe, expect, it } from "vitest";

import { resolveTokenPermissions } from "../tokenControl";
import { didApply } from "../types";
import type { WorldToken } from "../../types";
import type { FacetPrincipal } from "../types";

const token = (overrides: Partial<WorldToken> = {}): WorldToken => ({
  id: "t1",
  x: 0,
  y: 0,
  z: 0,
  ...overrides,
});

const gm: FacetPrincipal = { userId: "u-gm", authority: "gm" };
const player: FacetPrincipal = { userId: "u-p", authority: "player" };
const observer: FacetPrincipal = { userId: "u-o", authority: "observer" };

describe("resolveTokenPermissions", () => {
  it("gives a GM every control on any token", () => {
    const permissions = resolveTokenPermissions(token({ ownerUserId: "someone-else" }), gm);
    expect(permissions).toMatchObject({
      canMove: true,
      canRotate: true,
      canResize: true,
      canSetArt: true,
      canDelete: true,
    });
  });

  it("lets a player move a token they own", () => {
    const permissions = resolveTokenPermissions(token({ ownerUserId: "u-p" }), player);
    expect(permissions.canMove).toBe(true);
  });

  it("does not let a player move someone else's token", () => {
    const permissions = resolveTokenPermissions(token({ ownerUserId: "u-other" }), player);
    expect(permissions.canMove).toBe(false);
  });

  it("keeps size and facing GM-only even on a player's own token", () => {
    // Spec 004 FR-010. A client that allowed these would produce an action
    // the server refuses, which is worse than a disabled button.
    const permissions = resolveTokenPermissions(token({ ownerUserId: "u-p" }), player);
    expect(permissions.canRotate).toBe(false);
    expect(permissions.canResize).toBe(false);
  });

  it("lets a player set art only on their own primary token", () => {
    // `setOwnPrimaryTokenPhoto` is scoped exactly this narrowly server-side.
    const own = token({ ownerUserId: "u-p", isPrimary: true });
    const secondary = token({ ownerUserId: "u-p", isPrimary: false });
    expect(resolveTokenPermissions(own, player).canSetArt).toBe(true);
    expect(resolveTokenPermissions(secondary, player).canSetArt).toBe(false);
  });

  it("gives an observer nothing, even on a token carrying their id", () => {
    // The case a boolean `isGm` collapses into "player".
    const permissions = resolveTokenPermissions(token({ ownerUserId: "u-o" }), observer);
    expect(permissions).toMatchObject({
      canMove: false,
      canRotate: false,
      canResize: false,
      canSetArt: false,
      canDelete: false,
    });
  });

  it("treats an unauthenticated principal as owning nothing", () => {
    const permissions = resolveTokenPermissions(
      // A null owner and a null viewer must not compare equal into access.
      token({ ownerUserId: null }),
      { userId: null, authority: "player" },
    );
    expect(permissions.canMove).toBe(false);
  });
});

describe("didApply", () => {
  it("is true for accepted and adjusted, false for refusals and rejections", () => {
    // `adjusted` counts: the server changed the value but something did
    // happen, and a caller that treats it as failure will not render the
    // correction.
    expect(didApply({ status: "accepted", value: 1 })).toBe(true);
    expect(didApply({ status: "adjusted", value: 2, requested: 1 })).toBe(true);
    expect(didApply({ status: "rejected", reason: "no" })).toBe(false);
    expect(didApply({ status: "refused", reason: "not-yours" })).toBe(false);
  });
});
