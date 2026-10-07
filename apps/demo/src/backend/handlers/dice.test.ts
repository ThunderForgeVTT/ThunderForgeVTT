/**
 * The demo's dice are the server's dice (spec 074, spec 014): the formula is
 * parsed and resolved by `crates/thunderforge-dice` built for the page, and
 * the roll is recorded for the DM's history as `roll_dice_impl` records it.
 */
import { readFileSync } from "node:fs";
import { beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

vi.hoisted(() =>
  Object.assign(globalThis, {
    window: {
      addEventListener() {},
      localStorage: { getItem: () => null, setItem() {}, removeItem() {} },
      location: { href: "http://demo.test/demo/", origin: "http://demo.test" },
    },
  }),
);

import { DEMO_USER } from "../../seed/world";
import { runOperation } from "../execute";
import { demoState, loadState, type Row } from "../state";
import { loadDiceForTest, seedDice } from "./dice";

const BASE = "/demo/";
const fetchStatic = (async () =>
  new Response(
    readFileSync(new URL("../../../public/maps/maps.json", import.meta.url)),
  )) as unknown as typeof fetch;

const ROLL = `mutation ($input: RollDiceInput!) {
  rollDice(input: $input) {
    formula resultKind resultValue
    dice { sidesKind numericSides rolls kept finalValue }
  }
}`;
const HISTORY = `query ($worldId: UUID!, $limit: Int) {
  worldRollRecords(worldId: $worldId, limit: $limit) {
    id triggeredBy resolution { formula resultValue }
  }
}`;

async function ask(query: string, variables: Record<string, unknown>) {
  return runOperation({ query, variables });
}

async function rollOf(formula: string, bindings?: Row[]) {
  const worldId = demoState().world.id;
  return ask(ROLL, { input: { worldId, formula, bindings } });
}

beforeAll(async () => {
  loadDiceForTest(
    readFileSync(
      new URL("../../../../../dist/dice/dice_bg.wasm", import.meta.url),
    ),
  );
  await loadState(fetchStatic, BASE);
});

beforeEach(() => {
  const state = demoState();
  state.viewer = "gm";
  state.rolls = [];
  seedDice([1, 2, 3, 4]);
});

describe("rollDice", () => {
  it("resolves a formula with the server's dice and keeps every die", async () => {
    const answer = await rollOf("4d6kh3 + 2");
    expect(answer.errors).toBeUndefined();
    const roll = answer.data?.rollDice as Row & { dice: Row[] };
    expect(roll.formula).toBe("4d6kh3 + 2");
    expect(roll.resultKind).toBe("TOTAL");
    expect(roll.dice).toHaveLength(4);
    expect(roll.dice.filter((die) => die.kept)).toHaveLength(3);
    for (const die of roll.dice) {
      expect(die.sidesKind).toBe("NUMERIC");
      expect(die.numericSides).toBe(6);
      expect(die.finalValue).toBeGreaterThanOrEqual(1);
      expect(die.finalValue).toBeLessThanOrEqual(6);
    }
    const kept = roll.dice
      .filter((die) => die.kept)
      .reduce((sum, die) => sum + (die.finalValue as number), 0);
    expect(roll.resultValue).toBe(kept + 2);
  });

  it("is decided by the seed in a test, and differs roll to roll", async () => {
    const first = await rollOf("20d20");
    const second = await rollOf("20d20");
    seedDice([1, 2, 3, 4]);
    const again = await rollOf("20d20");
    expect(again.data).toEqual(first.data);
    expect(second.data).not.toEqual(first.data);
  });

  it("binds a named placeholder, as the sheet's formulas do", async () => {
    const answer = await rollOf("1d1 + STR", [{ name: "STR", value: 3 }]);
    expect(answer.errors).toBeUndefined();
    expect((answer.data?.rollDice as Row).resultValue).toBe(4);
  });

  it("refuses what the server refuses, and records nothing", async () => {
    const answer = await rollOf("1d20 +");
    expect(answer.errors?.[0]?.message).toMatch(/^Roll rejected: /);
    expect(demoState().rolls).toHaveLength(0);
  });

  it("rolls Fate dice as the server names them", async () => {
    const answer = await rollOf("4dF");
    const roll = answer.data?.rollDice as Row & { dice: Row[] };
    expect(roll.dice.every((die) => die.sidesKind === "FATE")).toBe(true);
    expect(roll.dice.every((die) => die.numericSides === null)).toBe(true);
  });
});

describe("validateDiceFormula", () => {
  it("answers as the server's parser does", async () => {
    const check = async (formula: string) =>
      (
        await ask(`query ($f: String!) { validateDiceFormula(formula: $f) }`, {
          f: formula,
        })
      ).data?.validateDiceFormula;
    expect(await check("1d20 + STAT + MODIFIERS")).toBe(true);
    expect(await check("1d6x")).toBe(true);
    expect(await check("1d20 +")).toBe(false);
    expect(await check("roll a d20")).toBe(false);
    expect(await check("8d10cs>=7")).toBe(true);
  });
});

describe("worldRollRecords", () => {
  it("gives the GM every roll, newest first, by who rolled it", async () => {
    await rollOf("1d4");
    await rollOf("1d6");
    await rollOf("1d8");
    const answer = await ask(HISTORY, {
      worldId: demoState().world.id,
      limit: 2,
    });
    const records = answer.data?.worldRollRecords as Array<
      Row & { resolution: Row }
    >;
    expect(records.map((r) => r.resolution.formula)).toEqual(["1d8", "1d6"]);
    expect(records[0].triggeredBy).toBe(DEMO_USER.id);
  });

  it("refuses a player, as the server does", async () => {
    await rollOf("1d4");
    demoState().viewer = "player";
    const answer = await ask(HISTORY, { worldId: demoState().world.id });
    expect(answer.errors?.[0]?.message).toBe(
      "Only the DM (Owner or GM) may view roll history",
    );
  });

  it("records a check from the sheet as it records a free roll", async () => {
    const state = demoState();
    const fighter = state.actors.find((a) => a.castKey === "fighter") as Row;
    const answer = await ask(
      `mutation ($w: UUID!, $a: UUID!) {
        rollCheck(worldId: $w, actorId: $a, checkId: "athletics") {
          formula resultValue
        }
      }`,
      { w: state.world.id, a: fighter.id },
    );
    expect(answer.errors).toBeUndefined();
    const roll = answer.data?.rollCheck as Row;
    expect(roll.formula).toMatch(/^1d20 [+-] \d+$/);
    expect(state.rolls?.at(-1)?.triggeredBy).toBeDefined();
    expect(state.rolls).toHaveLength(1);
  });
});
