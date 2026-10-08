/**
 * Spec 084 (FR-019): the demo's mirror of 5e's roll facets, held to the
 * cases `packs/systems/dnd5e/server/src/roll_facets_tests.rs` holds the
 * server to.
 */
import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it } from "vitest";

import { runOperation } from "../execute";
import { demoState, type Row } from "../state";
import { freshWorld } from "../testing/world";
import { loadDiceForTest, seedDice } from "./dice";
import {
  NO_D20,
  NO_DAMAGE_ADVANTAGE,
  facetRows,
  shapeD20,
  shapeDamage,
} from "./facets";

describe("shapeD20", () => {
  it("rolls a check's d20 twice and keeps the higher for advantage", () => {
    expect(shapeD20("1d20 + MODIFIER", "ADVANTAGE")).toEqual({
      formula: "2d20kh1 + MODIFIER",
      facets: ["advantage"],
    });
  });

  it("keeps the lower for disadvantage", () => {
    expect(shapeD20("1d20+5", "DISADVANTAGE")).toEqual({
      formula: "2d20kl1+5",
      facets: ["disadvantage"],
    });
  });

  it("leaves a normal roll as declared", () => {
    expect(shapeD20("1d20 + MODIFIER", "NORMAL")).toEqual({
      formula: "1d20 + MODIFIER",
      facets: [],
    });
  });

  it("takes the first d20 that keeps nothing, and leaves the rest", () => {
    expect(shapeD20("2d20kh1 + 1d20 + d4", "ADVANTAGE").formula).toBe(
      "2d20kh1 + 2d20kh1 + d4",
    );
    expect(shapeD20("d20 + 3d20", "ADVANTAGE").formula).toBe("2d20kh1 + 3d20");
  });

  it("refuses a formula with no d20 to roll twice", () => {
    expect(() => shapeD20("1d6 + 2", "ADVANTAGE")).toThrow(NO_D20);
    expect(() => shapeD20("1d200", "ADVANTAGE")).toThrow(NO_D20);
  });
});

describe("shapeD20 with Halfling Luck", () => {
  const halfling = { facets: ["halfling_luck"] };

  it("rerolls a natural 1 once on a halfling's d20 test", () => {
    expect(shapeD20("1d20 + MODIFIER", "NORMAL", halfling)).toEqual({
      formula: "1d20r1 + MODIFIER",
      facets: ["halfling_luck"],
    });
  });

  it("rerolls before it keeps, with advantage", () => {
    expect(shapeD20("1d20 + MODIFIER", "ADVANTAGE", halfling)).toEqual({
      formula: "2d20r1kh1 + MODIFIER",
      facets: ["advantage", "halfling_luck"],
    });
  });

  it("leaves a term that already rerolls, and a sheet without the facet", () => {
    expect(shapeD20("1d20r2 + 1", "NORMAL", halfling).formula).toBe(
      "1d20r2 + 1",
    );
    expect(shapeD20("1d20 + 1", "NORMAL", { facets: ["lucky"] })).toEqual({
      formula: "1d20 + 1",
      facets: [],
    });
  });

  it("asks nothing of a halfling's roll with no d20", () => {
    expect(shapeD20("1d6 + 2", "NORMAL", halfling)).toEqual({
      formula: "1d6 + 2",
      facets: [],
    });
  });
});

const ROLL = `mutation ($input: RollDiceInput!) {
  rollDice(input: $input) { dice { rolls steps finalValue } }
}`;

async function firstDie(formula: string): Promise<Row> {
  const worldId = demoState().world.id;
  const answer = await runOperation({
    query: ROLL,
    variables: { input: { worldId, formula } },
  });
  expect(answer.errors).toBeUndefined();
  return (answer.data?.rollDice as { dice: Row[] }).dice[0];
}

/** A seed whose every word differs, so the first draw moves with it. */
function spread(seed: number): number[] {
  return [1, 2, 3, 4].map((k) => Math.imul(seed + k, 0x9e3779b1) >>> 0);
}

describe("a halfling's natural 1, rolled by the page's dice", () => {
  beforeAll(async () => {
    loadDiceForTest(
      readFileSync(
        new URL("../../../../../dist/dice/dice_bg.wasm", import.meta.url),
      ),
    );
    await freshWorld();
    demoState().viewer = "gm";
  });

  it("is rolled again, and the new roll counts", async () => {
    let seed = 0;
    for (; seed < 500; seed += 1) {
      seedDice(spread(seed));
      if (((await firstDie("1d20")).rolls as number[])[0] === 1) break;
    }
    expect(seed).toBeLessThan(500);
    seedDice(spread(seed));
    const die = await firstDie("1d20r1");
    const rolls = die.rolls as number[];
    expect(rolls).toHaveLength(2);
    expect(rolls[0]).toBe(1);
    expect(die.steps).toEqual(["REROLL"]);
    expect(die.finalValue).toBe(rolls[1]);
  });
});

describe("shapeDamage", () => {
  it("rolls damage as declared, and never with advantage", () => {
    expect(shapeDamage("1d8 + 3", "NORMAL")).toEqual({
      formula: "1d8 + 3",
      facets: [],
    });
    expect(() => shapeDamage("1d8 + 3", "ADVANTAGE")).toThrow(
      NO_DAMAGE_ADVANTAGE,
    );
  });

  it("raises a two-handed melee weapon's low dice with Great Weapon Fighting", () => {
    const traitData = { facets: ["great_weapon_fighting"] };
    const greatsword = { traitData, melee: true, properties: ["two_handed"] };
    expect(shapeDamage("2d6 + 1d8 + 3", "NORMAL", greatsword)).toEqual({
      formula: "2d6min3 + 1d8min3 + 3",
      facets: ["great_weapon_fighting"],
    });
    // A term that already clamps is left alone.
    expect(shapeDamage("2d6min2", "NORMAL", greatsword)).toEqual({
      formula: "2d6min2",
      facets: [],
    });
  });

  it("needs melee, a two-handed weapon and the fighting style", () => {
    const traitData = { facets: ["great_weapon_fighting"] };
    const untouched = { formula: "2d6 + 3", facets: [] };
    for (const weapon of [
      { traitData, melee: false, properties: ["two_handed"] },
      { traitData, melee: true, properties: ["versatile"] },
      { traitData: { facets: [] }, melee: true, properties: ["two_handed"] },
    ]) {
      expect(shapeDamage("2d6 + 3", "NORMAL", weapon)).toEqual(untouched);
    }
  });
});

describe("facetRows", () => {
  it("names each facet as 5e names it", () => {
    expect(facetRows(["advantage", "disadvantage"])).toEqual([
      { id: "advantage", label: "Advantage" },
      { id: "disadvantage", label: "Disadvantage" },
    ]);
  });
});
