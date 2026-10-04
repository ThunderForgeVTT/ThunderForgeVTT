import { describe, expect, it } from "vitest";

import type { StatBlockAttackPlan } from "@/host";
import type { WorldAbilityRecord } from "@/types/ability";

import source from "../../../../../../../packs/systems/dnd5e/web/src/StatBlocks.ts";
import { calculateProficiencyBonusForChallenge } from "../../../../../../../packs/systems/dnd5e/web/src/derived-data.ts";
import { reusableAbility } from "../applyStatBlock";
import { resolveStatBlocks, statBlockForCreature } from "../systemStatBlocks";

/**
 * Applying a stat block, as far as it can be checked without a server.
 *
 * Two halves. The host half decides which world ability a plan entry may
 * reuse; get that wrong and either every goblin makes its own scimitar or a
 * goblin swings somebody else's. The pack half turns a printed block into
 * slot values and attacks, and it is tested here, against a real pack,
 * because the host's contract is only as good as one pack honouring it.
 */

function ability(over: Partial<WorldAbilityRecord>): WorldAbilityRecord {
  return {
    id: "a1",
    name: "Scimitar (Goblin Warrior)",
    classification: "feat",
    myPermissionLevel: "EDITOR",
    effects: [
      { effectType: "DAMAGE", formula: "1d6+2", sortOrder: 1 },
      { effectType: "ATTACK_ROLL", formula: "1d20+4", sortOrder: 0 },
    ],
    ...over,
  } as unknown as WorldAbilityRecord;
}

const scimitar: StatBlockAttackPlan = {
  key: "Scimitar",
  name: "Scimitar (Goblin Warrior)",
  description: "",
  classification: "feat",
  attackRoll: "1d20+4",
  damage: "1d6+2",
  reach: 5,
  rangeNormal: null,
  rangeLong: null,
  parts: [],
};

describe("reusableAbility", () => {
  it("reuses an ability with the same name and the same arithmetic", () => {
    expect(reusableAbility([ability({})], scimitar)?.id).toBe("a1");
  });

  it("does not reuse one whose numbers were edited since", () => {
    const edited = ability({
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+6", sortOrder: 0 },
        { effectType: "DAMAGE", formula: "1d6+2", sortOrder: 1 },
      ],
    } as Partial<WorldAbilityRecord>);
    expect(reusableAbility([edited], scimitar)).toBeNull();
  });

  it("does not reuse one the viewer may only read, or one of another kind", () => {
    expect(
      reusableAbility([ability({ myPermissionLevel: "VIEWER" })], scimitar),
    ).toBeNull();
    expect(
      reusableAbility([ability({ classification: "spell" })], scimitar),
    ).toBeNull();
    expect(
      reusableAbility([ability({ name: "Scimitar (Bandit)" })], scimitar),
    ).toBeNull();
  });
});

describe("the bundled fifth edition stat blocks", () => {
  // The world's system id reaches the host as data; a test has to spell it.
  const SYSTEM = "dnd5e";

  it("is found by the host for its system, and for nobody else's", () => {
    expect(resolveStatBlocks(SYSTEM)).not.toBeNull();
    expect(resolveStatBlocks("no_such_system")).toBeNull();
    expect(statBlockForCreature(SYSTEM, "goblin")?.id).toBe("goblin-warrior");
    expect(statBlockForCreature(SYSTEM, "no-such-creature")).toBeNull();
  });

  it("writes a goblin's printed numbers into the slots", () => {
    const plan = source.plan("goblin-warrior", null)!;
    expect(plan.slots.resource_data).toMatchObject({
      max_hp: 10,
      current_hp: 10,
      temporary_hp: 0,
      hit_dice: "3d6",
    });
    expect(plan.slots.ability_data).toMatchObject({
      armor_class: 15,
      dexterity: 15,
    });
    expect(plan.slots.trait_data).toMatchObject({
      challenge: "1/4",
      size: "small",
      speed_walk: 30,
      darkvision: 60,
    });
    expect(plan.slots.proficiency_data).toMatchObject({
      skill_proficiencies: ["stealth"],
      skill_expertise: ["stealth"],
    });
  });

  it("keeps the notes and drops what only a character has", () => {
    const plan = source.plan("ogre", {
      trait_data: {
        notes: "Guards the bridge.",
        level: 3,
        class: "Fighter",
        speed_fly: 60,
      },
    })!;
    expect(plan.slots.trait_data?.notes).toBe("Guards the bridge.");
    expect(plan.slots.trait_data).not.toHaveProperty("level");
    expect(plan.slots.trait_data).not.toHaveProperty("class");
    // The last creature's fly speed is not the ogre's.
    expect(plan.slots.trait_data).not.toHaveProperty("speed_fly");
  });

  it("turns each attack into a roll and a damage formula", () => {
    const plan = source.plan("goblin-warrior", null)!;
    const blade = plan.attacks.find((attack) => attack.key === "Scimitar")!;
    expect(blade).toMatchObject({
      name: "Scimitar (Goblin Warrior)",
      attackRoll: "1d20+4",
      reach: 5,
      rangeNormal: null,
    });
    const bow = plan.attacks.find((attack) => attack.key === "Shortbow")!;
    expect(bow).toMatchObject({ reach: null, rangeNormal: 80, rangeLong: 320 });
  });

  it("names only attacks it also plans in a multiattack", () => {
    for (const summary of source.blocks) {
      const plan = source.plan(summary.id, null)!;
      const keys = new Set(plan.attacks.map((attack) => attack.key));
      for (const attack of plan.attacks) {
        for (const part of attack.parts) {
          expect(keys.has(part), `${summary.id}: ${part}`).toBe(true);
        }
      }
    }
  });

  it("reads the proficiency bonus off the challenge rating", () => {
    expect(calculateProficiencyBonusForChallenge("1/4")).toBe(2);
    expect(calculateProficiencyBonusForChallenge("0")).toBe(2);
    expect(calculateProficiencyBonusForChallenge("4")).toBe(2);
    expect(calculateProficiencyBonusForChallenge("5")).toBe(3);
    expect(calculateProficiencyBonusForChallenge("17")).toBe(6);
    expect(calculateProficiencyBonusForChallenge("30")).toBe(9);
    expect(calculateProficiencyBonusForChallenge("31")).toBeNull();
    expect(calculateProficiencyBonusForChallenge("")).toBeNull();
    expect(calculateProficiencyBonusForChallenge("3/4")).toBeNull();
  });
});
