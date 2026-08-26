import { describe, expect, it } from "vitest";

import { createTokenControlFacet, resolveTokenPermissions } from "../tokenControl";
import { createLocalAdjudicator } from "../adjudicator";
import { createWorldStore } from "../../store";
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

describe("createTokenControlFacet", () => {
  const scene = (token: Partial<WorldToken>) =>
    createWorldStore({
      worldId: "w1",
      initialTokens: [{ id: "t1", x: 0, y: 0, z: 0, ...token }],
    });

  const facetFor = (store: ReturnType<typeof createWorldStore>, principal: FacetPrincipal) =>
    createTokenControlFacet(store, {
      worldId: "w1",
      sceneId: "s1",
      principal,
      adjudicator: createLocalAdjudicator(),
    });

  it("applies a manipulate the principal is permitted", async () => {
    const store = scene({ ownerUserId: "u-gm" });
    const result = await facetFor(store, gm).manipulate({ tokenId: "t1", scale: 3 });

    expect(result.status).toBe("accepted");
    expect(store.getState().tokens.t1.scale).toBe(3);
  });

  it("refuses size and facing for a player, and changes nothing", async () => {
    // The guarantee the TokenTool migration rests on: the panel disables
    // these from the same permission the facet refuses them by, so an
    // enabled control and a refused intent cannot disagree.
    const store = scene({ ownerUserId: "u-p", scale: 1 });
    const result = await facetFor(store, player).manipulate({ tokenId: "t1", scale: 4 });

    expect(result).toEqual({ status: "refused", reason: "gm-only" });
    expect(store.getState().tokens.t1.scale).toBe(1);
  });

  it("refuses art on a token the player does not own", async () => {
    const store = scene({ ownerUserId: "someone-else", isPrimary: true });
    const result = await facetFor(store, player).manipulate({
      tokenId: "t1",
      photoUrl: "/api/canvas-assets/x.webp",
    });

    expect(result.status).toBe("refused");
    expect(store.getState().tokens.t1.photoUrl).toBeUndefined();
  });

  it("lets a player set art on their own primary token", async () => {
    const store = scene({ ownerUserId: "u-p", isPrimary: true });
    const result = await facetFor(store, player).manipulate({
      tokenId: "t1",
      photoUrl: "/api/canvas-assets/x.webp",
    });

    expect(result.status).toBe("accepted");
    expect(store.getState().tokens.t1.photoUrl).toBe("/api/canvas-assets/x.webp");
  });

  it("carries a null photoUrl through, because clearing art is not the same as leaving it", async () => {
    const store = scene({ ownerUserId: "u-gm", photoUrl: "/api/canvas-assets/x.webp" });
    const result = await facetFor(store, gm).manipulate({ tokenId: "t1", photoUrl: null });

    expect(result.status).toBe("accepted");
    expect(store.getState().tokens.t1.photoUrl).toBeNull();
  });

  it("refuses an unknown token rather than dispatching a phantom", async () => {
    const store = scene({});
    const result = await facetFor(store, gm).manipulate({ tokenId: "ghost", scale: 2 });

    expect(result).toEqual({ status: "refused", reason: "unknown-subject" });
  });

  it("applies the authoritative position on a move, not the requested one", async () => {
    // With an adjudicator that corrects the move, the board must show
    // where the token actually ended up.
    const store = scene({ ownerUserId: "u-p" });
    const facet = createTokenControlFacet(store, {
      worldId: "w1",
      sceneId: "s1",
      principal: player,
      adjudicator: {
        async resolve(proposal) {
          const requested = proposal.payload as { tokenId: string; x: number; y: number };
          return {
            status: "adjusted",
            value: { ...requested, x: 10 } as typeof proposal.payload,
            requested: proposal.payload,
          };
        },
      },
    });

    const result = await facet.move({ tokenId: "t1", x: 999, y: 5 });

    expect(result.status).toBe("adjusted");
    expect(store.getState().tokens.t1.x).toBe(10);
    expect(store.getState().tokens.t1.y).toBe(5);
  });

  it("lists only the tokens this principal may move", async () => {
    const store = createWorldStore({
      worldId: "w1",
      initialTokens: [
        { id: "mine", x: 0, y: 0, z: 0, ownerUserId: "u-p" },
        { id: "theirs", x: 0, y: 0, z: 0, ownerUserId: "u-other" },
      ],
    });
    const facet = facetFor(store, player);

    expect(facet.tokens()).toHaveLength(2);
    expect(facet.controllable().map((entry) => entry.token.id)).toEqual(["mine"]);
  });
});
