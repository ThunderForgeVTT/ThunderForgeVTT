import { describe, expect, it } from "vitest";

import { statRolls } from "../../../../../../../packs/systems/dnd5e/web/src/statRolls.ts";

/**
 * The 5e pack's own stat rolls for the play dock and the sheet page.
 *
 * `ability_data` holds 5e's *scores*, not modifiers, and Armor Class beside
 * them. The host's generic reading offered "Wisdom 1d20+18" and "Armor class
 * 1d20+18"; the pack turns a score into its modifier and offers nothing for
 * a number that is not a check.
 *
 * Names a system because a test has to; `check-system-registry` exempts
 * tests for that reason.
 */

const SCORES = {
  strength: 8,
  dexterity: 14,
  constitution: 10,
  intelligence: 11,
  wisdom: 18,
  charisma: 3,
};

function formulaOf(rolls: ReturnType<typeof statRolls>, key: string) {
  return rolls.find((roll) => roll.key === key)?.formula;
}

describe("dnd5e statRolls", () => {
  it("offers each ability check with the modifier its score gives", () => {
    const rolls = statRolls({ ability_data: SCORES });
    expect(rolls.slice(0, 6)).toEqual([
      { key: "stat-strength", label: "Strength", formula: "1d20-1" },
      { key: "stat-dexterity", label: "Dexterity", formula: "1d20+2" },
      { key: "stat-constitution", label: "Constitution", formula: "1d20" },
      { key: "stat-intelligence", label: "Intelligence", formula: "1d20" },
      { key: "stat-wisdom", label: "Wisdom", formula: "1d20+4" },
      { key: "stat-charisma", label: "Charisma", formula: "1d20-4" },
    ]);
  });

  it("offers nothing for Armor Class or any other number that is not a check", () => {
    const rolls = statRolls({
      ability_data: { ...SCORES, armor_class: 18, speed: 30, level: 5 },
      resource_data: { max_hp: 40, current_hp: 12 },
    });
    expect(rolls.map((roll) => roll.key)).toEqual([
      "stat-strength",
      "stat-dexterity",
      "stat-constitution",
      "stat-intelligence",
      "stat-wisdom",
      "stat-charisma",
    ]);
    expect(rolls.some((roll) => /armor/i.test(roll.label))).toBe(false);
  });

  it("leaves out a score nobody entered rather than reading it as 10", () => {
    expect(statRolls({ ability_data: { wisdom: 18 } })).toEqual([
      { key: "stat-wisdom", label: "Wisdom", formula: "1d20+4" },
    ]);
  });

  it("offers nothing without ability data", () => {
    expect(statRolls({})).toEqual([]);
    expect(statRolls({ ability_data: { wisdom: "18" } })).toEqual([]);
  });

  /** The manual sheet stores proficiencies as a list of ids. */
  it("adds a save, with the proficiency bonus, for each proficient ability", () => {
    const rolls = statRolls({
      ability_data: SCORES,
      proficiency_data: { saving_throw_proficiencies: ["wisdom", "charisma"] },
      trait_data: { level: 5 },
    });
    expect(rolls.slice(6)).toEqual([
      { key: "save-wisdom", label: "Wisdom save", formula: "1d20+7" },
      { key: "save-charisma", label: "Charisma save", formula: "1d20-1" },
    ]);
  });

  /** A sheet import stores them as an object of flags. */
  it("reads the flags a sheet import writes", () => {
    const rolls = statRolls({
      ability_data: SCORES,
      proficiency_data: {
        saving_throw_proficiencies: { strength: true, dexterity: false },
      },
      trait_data: { level: 9, classes: [{ name: "Fighter", level: 9 }] },
    });
    expect(formulaOf(rolls, "save-strength")).toBe("1d20+3");
    expect(formulaOf(rolls, "save-dexterity")).toBeUndefined();
  });

  it("takes a creature's bonus from its challenge rating when it has no level", () => {
    const rolls = statRolls({
      ability_data: SCORES,
      proficiency_data: { saving_throw_proficiencies: ["wisdom"] },
      trait_data: { challenge: "5" },
    });
    expect(formulaOf(rolls, "save-wisdom")).toBe("1d20+7");
  });

  it("reads a blank level as the sheet does, as level 1", () => {
    const rolls = statRolls({
      ability_data: SCORES,
      proficiency_data: { saving_throw_proficiencies: ["dexterity"] },
    });
    expect(formulaOf(rolls, "save-dexterity")).toBe("1d20+4");
  });

  it("offers no saves without proficiency data", () => {
    const rolls = statRolls({ ability_data: SCORES, trait_data: { level: 5 } });
    expect(rolls.some((roll) => roll.key.startsWith("save-"))).toBe(false);
  });
});
