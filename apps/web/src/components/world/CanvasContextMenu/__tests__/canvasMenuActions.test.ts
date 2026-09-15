import { describe, expect, it } from "vitest";
import type { TokenRecord } from "@/types/token";
import {
  attackerTokenOf,
  canvasMenuActions,
  type SheetAttack,
} from "../canvasMenuActions";

function token(overrides: Partial<TokenRecord>): TokenRecord {
  return {
    tokenId: "t",
    sceneId: "s",
    actorId: "a",
    x: 0,
    y: 0,
    rotation: 0,
    scale: 1,
    metadata: null,
    createdAt: "",
    updatedAt: "",
    ownerUserId: null,
    isPrimary: false,
    photoUrl: null,
    tokenType: "npc",
    linked: false,
    ...overrides,
  };
}

const LONGSWORD: SheetAttack = { abilityId: "sword", name: "Longsword" };
const aria = token({
  tokenId: "aria",
  actorId: "aria-actor",
  ownerUserId: "u-aria",
  isPrimary: true,
  tokenType: "character",
  linked: true,
});
const goblin = token({ tokenId: "goblin", actorId: "goblin-actor" });
const marker = token({ tokenId: "marker", actorId: null, tokenType: "object" });

const player = { isGameMaster: false, userId: "u-aria" };
const gm = { isGameMaster: true, userId: "u-gm" };

describe("the play field's right-click menu", () => {
  it("offers a player their sheet's attacks on another creature", () => {
    expect(
      canvasMenuActions({
        viewer: player,
        target: goblin,
        nameHidden: false,
        attacker: aria,
        attacks: [LONGSWORD],
      }),
    ).toEqual([{ kind: "attack", attack: LONGSWORD }]);
  });

  it("offers a player nothing on their own token, on bare board, or with no token here", () => {
    const base = { viewer: player, nameHidden: false, attacks: [LONGSWORD] };
    expect(
      canvasMenuActions({ ...base, target: aria, attacker: aria }),
    ).toEqual([]);
    expect(
      canvasMenuActions({ ...base, target: null, attacker: aria }),
    ).toEqual([]);
    expect(
      canvasMenuActions({ ...base, target: goblin, attacker: null }),
    ).toEqual([]);
  });

  it("never offers a player a Game Master's action", () => {
    const kinds = canvasMenuActions({
      viewer: player,
      target: goblin,
      nameHidden: false,
      attacker: aria,
      attacks: [LONGSWORD],
    }).map((action) => action.kind);
    for (const gmOnly of ["damage", "heal", "link", "name", "remove"]) {
      expect(kinds).not.toContain(gmOnly);
    }
  });

  it("offers a Game Master hit points, link, name and removal on a creature", () => {
    expect(
      canvasMenuActions({
        viewer: gm,
        target: goblin,
        nameHidden: false,
        attacker: null,
        attacks: [],
      }),
    ).toEqual([
      { kind: "damage" },
      { kind: "heal" },
      { kind: "link", linked: true },
      { kind: "name", hidden: true },
      { kind: "remove" },
    ]);
  });

  it("offers no hit points or link for a token standing for no actor", () => {
    expect(
      canvasMenuActions({
        viewer: gm,
        target: marker,
        nameHidden: true,
        attacker: null,
        attacks: [],
      }),
    ).toEqual([{ kind: "name", hidden: false }, { kind: "remove" }]);
  });

  it("offers a Game Master a token and a light on bare board", () => {
    expect(
      canvasMenuActions({
        viewer: gm,
        target: null,
        nameHidden: false,
        attacker: null,
        attacks: [],
      }),
    ).toEqual([{ kind: "place-token" }, { kind: "add-light" }]);
  });

  it("attacks from the viewer's primary token", () => {
    const spare = token({
      tokenId: "spare",
      ownerUserId: "u-aria",
      isPrimary: false,
    });
    expect(attackerTokenOf([spare, aria, goblin], "u-aria")?.tokenId).toBe(
      "aria",
    );
    expect(attackerTokenOf([goblin], "u-aria")).toBeNull();
  });
});
