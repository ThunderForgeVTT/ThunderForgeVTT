import { describe, expect, it } from "vitest";
import type { TrackerAbilityRecord } from "@/api/attacks";
import type { CombatantRecord } from "@/types/combat";
import { abilitiesFor, combatantActKind } from "../combatantActions";

const line = { allowed: 1, spent: 0, remaining: 1 };

function combatant(over: Partial<CombatantRecord> = {}): CombatantRecord {
  return {
    id: "c",
    combatId: "combat",
    actorId: null,
    tokenId: "token",
    label: "Ogre",
    kind: "CREATURE",
    initiative: 15,
    tiebreak: 0,
    isNpc: true,
    active: true,
    downedBy: null,
    budget: {
      action: line,
      bonusAction: line,
      reaction: line,
      movement: { allowed: 30, spent: 0, remaining: 30 },
      legendary: null,
      unit: "ft",
    },
    ...over,
  };
}

function ability(
  name: string,
  actionCost: TrackerAbilityRecord["actionCost"],
  rolls = true,
): TrackerAbilityRecord {
  return {
    id: name,
    name,
    actionCost,
    legendaryCost: 1,
    effects: rolls ? [{ effectType: "ATTACK_ROLL", formula: "1d20+4" }] : [],
  };
}

describe("combatantActKind", () => {
  it("offers a legendary action only to a creature with a pool and a token", () => {
    expect(combatantActKind(combatant())).toBeNull();
    const legendary = combatant({
      budget: {
        ...combatant().budget!,
        legendary: { allowed: 3, spent: 1, remaining: 2 },
      },
    });
    expect(combatantActKind(legendary)).toBe("legendary");
    expect(combatantActKind({ ...legendary, tokenId: null })).toBeNull();
  });

  it("offers a lair its action, with no budget to read", () => {
    expect(
      combatantActKind(
        combatant({ kind: "LAIR", tokenId: null, budget: null }),
      ),
    ).toBe("lair");
  });
});

describe("abilitiesFor", () => {
  const all = [
    ability("Claw", "ACTION"),
    ability("Tail", "LEGENDARY"),
    ability("Roar", "LEGENDARY", false),
  ];

  it("offers the legendary attacks for a legendary action", () => {
    expect(abilitiesFor("legendary", all).map((a) => a.name)).toEqual(["Tail"]);
  });

  it("offers every attack when none is declared legendary, and to a lair", () => {
    const plain = [ability("Claw", "ACTION"), ability("Bite", "ACTION")];
    expect(abilitiesFor("legendary", plain)).toHaveLength(2);
    expect(abilitiesFor("lair", all).map((a) => a.name)).toEqual([
      "Claw",
      "Tail",
    ]);
  });
});
