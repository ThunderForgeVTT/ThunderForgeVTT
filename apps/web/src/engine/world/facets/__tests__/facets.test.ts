import { describe, expect, it } from "vitest";

import { resolveTokenPermissions } from "../tokenControl";
import { DEFAULT_HIT_RADIUS, resolveStack } from "../selection";
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

describe("resolveStack", () => {
  it("finds every token at the point, not just the top one", () => {
    // The whole reason this exists: tokens genuinely stack.
    const stacked = [token({ id: "a" }), token({ id: "b" }), token({ id: "c" })];
    expect(resolveStack(stacked, { x: 0, y: 0 })).toHaveLength(3);
  });

  it("orders topmost first", () => {
    const stacked = [
      token({ id: "low", z: 0 }),
      token({ id: "high", z: 10 }),
      token({ id: "mid", z: 5 }),
    ];
    expect(resolveStack(stacked, { x: 0, y: 0 }).map((t) => t.id)).toEqual([
      "high",
      "mid",
      "low",
    ]);
  });

  it("breaks ties stably so a picker's entries do not reshuffle", () => {
    const stacked = [token({ id: "b" }), token({ id: "a" })];
    const once = resolveStack(stacked, { x: 0, y: 0 }).map((t) => t.id);
    const twice = resolveStack([...stacked].reverse(), { x: 0, y: 0 }).map((t) => t.id);
    expect(once).toEqual(["a", "b"]);
    expect(twice).toEqual(once);
  });

  it("excludes tokens outside the hit radius", () => {
    const spread = [token({ id: "near", x: 4 }), token({ id: "far", x: 400 })];
    expect(resolveStack(spread, { x: 0, y: 0 }).map((t) => t.id)).toEqual(["near"]);
  });

  it("includes a token exactly on the radius", () => {
    const edge = [token({ id: "edge", x: DEFAULT_HIT_RADIUS })];
    expect(resolveStack(edge, { x: 0, y: 0 })).toHaveLength(1);
  });

  it("measures distance in both axes, not just x", () => {
    const diagonal = [token({ id: "diag", x: DEFAULT_HIT_RADIUS, y: DEFAULT_HIT_RADIUS })];
    expect(resolveStack(diagonal, { x: 0, y: 0 })).toHaveLength(0);
  });

  it("returns nothing for empty canvas, which is a deselect and not an error", () => {
    expect(resolveStack([token({ x: 500 })], { x: 0, y: 0 })).toEqual([]);
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
