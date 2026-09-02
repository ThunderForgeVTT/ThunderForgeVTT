import { describe, expect, it } from "vitest";
import {
  buildRosterOffer,
  sameTokenIds,
  unattemptedIds,
} from "../combatRoster";
import type { CombatantRecord } from "@/types/combat";
import type { TokenRecord } from "@/types/token";
import type { WorldActorRecord } from "@/types/actor";

/**
 * What the combat panel offers a GM who has tokens selected (spec 031 FR-030).
 *
 * Every test here is about an *offer*. None of them add anybody: the module
 * under test cannot, which is the design. The assertions that matter most are
 * the ones proving an existing roster survives — a selection arriving must
 * never be able to take a combatant out of the order.
 *
 * This repo's vitest runs in a `node` environment and has no component tests;
 * that the button reaches the server is e2e's to prove.
 */

function token(over: Partial<TokenRecord> = {}): TokenRecord {
  return {
    tokenId: "token-1",
    sceneId: "scene-1",
    actorId: null,
    x: 0,
    y: 0,
    rotation: 0,
    scale: 1,
    metadata: null,
    createdAt: "2026-09-01T00:00:00Z",
    updatedAt: "2026-09-01T00:00:00Z",
    ownerUserId: null,
    isPrimary: false,
    photoUrl: null,
    health: null,
    maxHealth: null,
    tokenType: "character",
    ...over,
  };
}

function actor(over: Partial<WorldActorRecord> = {}): WorldActorRecord {
  return {
    id: "actor-1",
    worldId: "world-1",
    sceneId: "scene-1",
    actorType: "character",
    gameSystemId: null,
    label: "Vesper",
    description: null,
    isPublic: false,
    isNpc: false,
    createdBy: "user-1",
    ownedBy: "user-1",
    myPermissionLevel: "OWNER",
    createdAt: "2026-09-01T00:00:00Z",
    updatedAt: "2026-09-01T00:00:00Z",
    loreLinkedFrom: [],
    availableForClaim: false,
    claimedBy: null,
    ...over,
  };
}

function combatant(over: Partial<CombatantRecord> = {}): CombatantRecord {
  return {
    id: "combatant-1",
    combatId: "combat-1",
    actorId: null,
    tokenId: null,
    label: "Someone",
    initiative: 10,
    tiebreak: 0,
    isNpc: false,
    active: true,
    ...over,
  };
}

describe("buildRosterOffer", () => {
  it("offers the selected tokens in the order they were selected", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-2", "token-1"],
      tokens: [token({ tokenId: "token-1" }), token({ tokenId: "token-2" })],
      actors: [],
      combatants: [],
    });

    expect(offer.additions.map((c) => c.tokenId)).toEqual([
      "token-2",
      "token-1",
    ]);
  });

  it("names a token after the actor it stands for", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1"],
      tokens: [token({ actorId: "actor-1", metadata: { label: "Pawn" } })],
      actors: [actor()],
      combatants: [],
    });

    expect(offer.additions[0]).toMatchObject({
      label: "Vesper",
      actorId: "actor-1",
      isNpc: false,
    });
  });

  it("falls back to the token's own label, then to a placeholder", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1", "token-2", "token-3"],
      tokens: [
        token({ tokenId: "token-1", metadata: { label: "Wolf" } }),
        token({ tokenId: "token-2", metadata: { label: "   " } }),
        token({ tokenId: "token-3" }),
      ],
      actors: [],
      combatants: [],
    });

    expect(offer.additions.map((c) => c.label)).toEqual([
      "Wolf",
      "Unnamed token",
      "Unnamed token",
    ]);
  });

  it("takes NPC-ness from the actor, or from the token when there is none", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1", "token-2"],
      tokens: [
        token({ tokenId: "token-1", actorId: "actor-1" }),
        token({ tokenId: "token-2", tokenType: "npc" }),
      ],
      actors: [actor({ id: "actor-1", isNpc: true })],
      combatants: [],
    });

    expect(offer.additions.map((c) => c.isNpc)).toEqual([true, true]);
  });

  it("keeps an actor reference the world actor list does not explain", () => {
    // A GM who cannot see every actor still gets a combatant bound to the
    // right sheet; the server owns that reference, not this list.
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1"],
      tokens: [token({ actorId: "actor-9" })],
      actors: [],
      combatants: [],
    });

    expect(offer.additions[0].actorId).toBe("actor-9");
  });

  it("never removes or reorders what the combat already holds", () => {
    // The whole reason this is an offer: a roster built over a session must
    // survive an unrelated selection untouched.
    const existing = [
      combatant({ id: "c-1", tokenId: "token-9", label: "Boss" }),
      combatant({ id: "c-2", actorId: "actor-1", label: "Vesper" }),
    ];

    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1"],
      tokens: [token({ tokenId: "token-1" })],
      actors: [],
      combatants: existing,
    });

    expect(offer.additions).toHaveLength(1);
    expect(existing.map((c) => c.id)).toEqual(["c-1", "c-2"]);
  });

  it("reports a token the combat already contains instead of duplicating it", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1", "token-2"],
      tokens: [token({ tokenId: "token-1" }), token({ tokenId: "token-2" })],
      actors: [],
      combatants: [combatant({ tokenId: "token-1", label: "Wolf" })],
    });

    expect(offer.additions.map((c) => c.tokenId)).toEqual(["token-2"]);
    expect(offer.alreadyPresent.map((c) => c.tokenId)).toEqual(["token-1"]);
  });

  it("recognises a combatant added through the actor picker", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1"],
      tokens: [token({ actorId: "actor-1" })],
      actors: [actor()],
      combatants: [combatant({ actorId: "actor-1" })],
    });

    expect(offer.additions).toEqual([]);
    expect(offer.alreadyPresent).toHaveLength(1);
  });

  it("treats two tokens of one actor as two participants", () => {
    // A token-backed combatant answers for that token alone: a duplicated
    // minion gets its own turn.
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1", "token-2"],
      tokens: [
        token({ tokenId: "token-1", actorId: "actor-1" }),
        token({ tokenId: "token-2", actorId: "actor-1" }),
      ],
      actors: [actor()],
      combatants: [combatant({ tokenId: "token-1", actorId: "actor-1" })],
    });

    expect(offer.additions.map((c) => c.tokenId)).toEqual(["token-2"]);
  });

  it("reports selected ids no scene token answers to", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["demo-token", "token-1"],
      tokens: [token({ tokenId: "token-1" })],
      actors: [],
      combatants: [],
    });

    expect(offer.unresolvedTokenIds).toEqual(["demo-token"]);
    expect(offer.additions.map((c) => c.tokenId)).toEqual(["token-1"]);
  });

  it("counts a token selected twice once", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: ["token-1", "token-1"],
      tokens: [token({ tokenId: "token-1" })],
      actors: [],
      combatants: [],
    });

    expect(offer.additions).toHaveLength(1);
  });

  it("offers nothing when nothing is selected", () => {
    const offer = buildRosterOffer({
      selectedTokenIds: [],
      tokens: [token()],
      actors: [actor()],
      combatants: [],
    });

    expect(offer).toEqual({
      additions: [],
      alreadyPresent: [],
      unresolvedTokenIds: [],
    });
  });
});

describe("unattemptedIds", () => {
  it("asks about an id once and never again", () => {
    const attempted = new Set(["demo-token"]);
    expect(unattemptedIds(["demo-token", "fresh"], attempted)).toEqual([
      "fresh",
    ]);
    expect(unattemptedIds(["demo-token"], attempted)).toEqual([]);
  });
});

describe("sameTokenIds", () => {
  it("distinguishes order and length", () => {
    expect(sameTokenIds(["a", "b"], ["a", "b"])).toBe(true);
    expect(sameTokenIds(["a", "b"], ["b", "a"])).toBe(false);
    expect(sameTokenIds(["a"], ["a", "b"])).toBe(false);
    expect(sameTokenIds([], [])).toBe(true);
  });
});
