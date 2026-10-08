/**
 * The demo's dice are the server's dice (spec 074, spec 014): the formula is
 * parsed and resolved by `crates/thunderforge-dice` built for the page, and
 * the roll is recorded for the DM's history as `roll_dice_impl` records it.
 */
import { readFileSync } from "node:fs";
import { beforeAll, beforeEach, describe, expect, it } from "vitest";

import { DEMO_USER } from "../../seed/world";
import { runOperation } from "../execute";
import { demoState, type Row } from "../state";
import { eventsSince } from "../events";
import { freshWorld, heard, refusal, releaseEvents } from "../testing/world";
import { loadDiceForTest, resolutionRow, seedDice } from "./dice";

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
  await freshWorld();
});

beforeEach(() => {
  const state = demoState();
  state.viewer = "gm";
  state.rolls = [];
  seedDice([1, 2, 3, 4]);
  // What earlier tests recorded is not this test's to hear.
  releaseEvents();
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
    expect(roll.formula).toBe("1d20 + MODIFIER");
    expect(state.rolls?.at(-1)?.triggeredBy).toBeDefined();
    expect(state.rolls).toHaveLength(1);
  });
});

const CHECK_WITH = `mutation ($w: UUID!, $a: UUID!, $adv: Advantage) {
  rollCheck(worldId: $w, actorId: $a, checkId: "stealth", advantage: $adv) {
    formula resultValue dice { numericSides kept finalValue }
  }
}`;
const FACETS_OF = `query ($worldId: UUID!, $rollId: UUID!) {
  worldRoll(worldId: $worldId, rollId: $rollId) {
    ... on WorldRoll { formula facets { id label } }
  }
}`;

/** Spec 084 (FR-019): a check rolled with advantage, as the server rolls it. */
describe("rollCheck with advantage", () => {
  async function checkWith(advantage?: string) {
    const state = demoState();
    const actor = state.actors.find((a) => !a.isNpc)!;
    const answer = await ask(CHECK_WITH, {
      w: state.world.id,
      a: actor.id,
      adv: advantage,
    });
    expect(answer.errors).toBeUndefined();
    const record = state.rolls!.at(-1)!;
    const seen = (
      await ask(FACETS_OF, { worldId: state.world.id, rollId: record.id })
    ).data?.worldRoll as Row;
    return { roll: answer.data?.rollCheck as Row, record, seen };
  }

  it("rolls two d20s and keeps the higher, and says so", async () => {
    const { roll, record, seen } = await checkWith("ADVANTAGE");
    expect(roll.formula).toBe("2d20kh1 + MODIFIER");
    const d20s = (roll.dice as Row[]).filter((d) => d.numericSides === 20);
    expect(d20s).toHaveLength(2);
    const kept = d20s.filter((d) => d.kept);
    expect(kept).toHaveLength(1);
    expect(kept[0].finalValue).toBe(
      Math.max(...d20s.map((d) => d.finalValue as number)),
    );
    expect(record).toMatchObject({
      rollKind: "check",
      checkId: "stealth",
      facets: ["advantage"],
    });
    expect(record.actorId).toBeDefined();
    expect(seen.facets).toEqual([{ id: "advantage", label: "Advantage" }]);
  });

  it("keeps the lower for disadvantage", async () => {
    const { roll, seen } = await checkWith("DISADVANTAGE");
    expect(roll.formula).toBe("2d20kl1 + MODIFIER");
    const d20s = (roll.dice as Row[]).filter((d) => d.numericSides === 20);
    expect(d20s.find((d) => d.kept)?.finalValue).toBe(
      Math.min(...d20s.map((d) => d.finalValue as number)),
    );
    expect(seen.facets).toEqual([
      { id: "disadvantage", label: "Disadvantage" },
    ]);
  });

  it("rolls the declared formula when nothing is chosen", async () => {
    const { roll, record, seen } = await checkWith();
    expect(roll.formula).toBe("1d20 + MODIFIER");
    expect(record.facets).toEqual([]);
    expect(seen.facets).toEqual([]);
  });
});

/** A roll by a member the demo does not seat, as another player's would be. */
const OTHER = "00000000-0000-7000-8000-0000000000aa";

const ROLL_AS = `mutation ($input: RollDiceInput!) {
  rollDice(input: $input) { resultValue }
}`;
const ROLL_ONE = `query ($worldId: UUID!, $rollId: UUID!) {
  worldRoll(worldId: $worldId, rollId: $rollId) {
    __typename
    ... on WorldRoll { id rollerName label formula visibility revealedAt revealedByName resolution { resultValue } }
    ... on MaskedRoll { id rollerName visibility }
  }
}`;
const FEED = `query ($worldId: UUID!, $before: String, $limit: Int) {
  worldRolls(worldId: $worldId, before: $before, limit: $limit) {
    __typename
    ... on WorldRoll { id }
    ... on MaskedRoll { id }
  }
}`;
const REVEAL = `mutation ($worldId: UUID!, $rollId: UUID!) {
  revealRoll(worldId: $worldId, rollId: $rollId) { id revealedAt revealedByName visibility }
}`;

/** Rolls as the current viewer, and the id it was recorded under. */
async function rollAs(input: Row): Promise<{ id: string; errors?: string }> {
  const worldId = demoState().world.id;
  const answer = await ask(ROLL_AS, {
    input: { worldId, formula: "1d20", ...input },
  });
  return {
    id: String(demoState().rolls?.at(-1)?.id ?? ""),
    errors: answer.errors?.[0]?.message,
  };
}

async function entryOf(rollId: string) {
  const worldId = demoState().world.id;
  return (await ask(ROLL_ONE, { worldId, rollId })).data?.worldRoll as
    | (Row & { resolution?: Row })
    | null;
}

async function feed(variables: Row = {}) {
  const worldId = demoState().world.id;
  return (await ask(FEED, { worldId, ...variables })).data?.worldRolls as Row[];
}

/** Spec 081: the server's visibility rule, as the demo answers it. */
describe("rolls at the table", () => {
  it("lets a player roll for the GM's eyes and the GM roll GM only, never the other way", async () => {
    demoState().viewer = "player";
    expect((await rollAs({ visibility: "GM_ONLY" })).errors).toBe(
      "Only the GM can roll for their eyes only",
    );
    expect((await rollAs({ visibility: "GM_EYES" })).errors).toBeUndefined();
    demoState().viewer = "gm";
    expect((await rollAs({ visibility: "GM_EYES" })).errors).toBe(
      "The GM rolls GM only, not for the GM's eyes",
    );
    expect((await rollAs({ visibility: "GM_ONLY" })).errors).toBeUndefined();
    expect(demoState().rolls).toHaveLength(2);
  });

  it("keeps a label trimmed and refuses one over 80 characters", async () => {
    expect((await rollAs({ label: "x".repeat(81) })).errors).toBe(
      "A roll's label is at most 80 characters",
    );
    const { id } = await rollAs({ label: "  Stealth  " });
    expect((await entryOf(id))?.label).toBe("Stealth");
    const blank = await rollAs({ label: "   " });
    expect((await entryOf(blank.id))?.label).toBeNull();
  });

  it("tells the table of a roll by id and visibility only", async () => {
    const events = heard();
    const { id } = await rollAs({ visibility: "GM_ONLY", label: "Ambush" });
    releaseEvents();
    expect(events.map((e) => [e.eventCode, e.tokenEvent])).toEqual([
      [36, { rollId: id, visibility: "gm_only" }],
    ]);
  });

  it("shows a GM only roll to the GM and to no player, in the fetch, the feed and the catch-up", async () => {
    const before = demoState().nextEventId - 1;
    const { id } = await rollAs({ visibility: "GM_ONLY" });
    expect((await entryOf(id))?.__typename).toBe("WorldRoll");
    expect(eventsSince(before).events).toHaveLength(1);

    demoState().viewer = "player";
    expect(await entryOf(id)).toBeNull();
    expect(await feed()).toEqual([]);
    expect(eventsSince(before).events).toEqual([]);
  });

  it("masks another player's GM's eyes roll and shows the roller and the GM all of it", async () => {
    demoState().viewer = "player";
    const own = await rollAs({ visibility: "GM_EYES", label: "Insight" });
    expect((await entryOf(own.id))?.__typename).toBe("WorldRoll");
    const other = {
      ...demoState().rolls!.at(-1)!,
      id: OTHER,
      triggeredBy: "someone",
    };
    demoState().rolls!.push(other);
    expect(await entryOf(OTHER)).toEqual({
      __typename: "MaskedRoll",
      id: OTHER,
      rollerName: "",
      visibility: "GM_EYES",
    });
    demoState().viewer = "gm";
    expect((await entryOf(OTHER))?.__typename).toBe("WorldRoll");
  });

  it("reads a roll from before spec 081 as one in the open", async () => {
    await rollOf("1d6");
    const record = demoState().rolls!.at(-1)!;
    delete record.visibility;
    demoState().viewer = "player";
    expect((await entryOf(String(record.id)))?.visibility).toBe("EVERYONE");
  });

  it("pages the feed newest first, before a time, at most 100", async () => {
    const ids: string[] = [];
    for (let n = 0; n < 3; n += 1) {
      const { id } = await rollAs({});
      demoState().rolls!.at(-1)!.createdAt = new Date(
        Date.UTC(2026, 0, 1, 0, n),
      ).toISOString();
      ids.push(id);
    }
    expect((await feed()).map((e) => e.id)).toEqual([...ids].reverse());
    expect(
      (
        await feed({
          before: new Date(Date.UTC(2026, 0, 1, 0, 2)).toISOString(),
          limit: 1,
        })
      ).map((e) => e.id),
    ).toEqual([ids[1]]);
    expect(
      (await ask(FEED, { worldId: demoState().world.id, before: "soon" }))
        .errors?.[0]?.message,
    ).toBe("`before` is not a time");
  });

  it("reveals once, by the GM, to the whole table", async () => {
    const { id } = await rollAs({ visibility: "GM_ONLY" });
    const worldId = demoState().world.id;
    demoState().viewer = "player";
    expect(await refusal(REVEAL, { worldId, rollId: id })).toBe(
      "Only the GM can reveal a roll",
    );
    demoState().viewer = "gm";
    expect(
      await refusal(REVEAL, { worldId, rollId: crypto.randomUUID() }),
    ).toBe("Roll not found");

    releaseEvents();
    const events = heard();
    const revealed = (await ask(REVEAL, { worldId, rollId: id })).data
      ?.revealRoll as Row;
    expect(revealed.revealedByName).toBe(DEMO_USER.username);
    expect(revealed.revealedAt).not.toBeNull();
    // Again: answered as it is, and nothing recorded.
    await ask(REVEAL, { worldId, rollId: id });
    releaseEvents();
    expect(events.map((e) => [e.eventCode, e.tokenEvent])).toEqual([
      [37, { rollId: id, visibility: "gm_only" }],
    ]);

    demoState().viewer = "player";
    const seen = await entryOf(id);
    expect(seen?.__typename).toBe("WorldRoll");
    expect(seen?.revealedByName).toBe(DEMO_USER.username);
  });

  it("answers a reveal of a roll in the open as it is, and records nothing", async () => {
    const { id } = await rollAs({});
    releaseEvents();
    const events = heard();
    const answer = await ask(REVEAL, {
      worldId: demoState().world.id,
      rollId: id,
    });
    releaseEvents();
    expect((answer.data?.revealRoll as Row).revealedAt).toBeNull();
    expect(events).toEqual([]);
  });

  it("names a check's roll for the check", async () => {
    const actor = demoState().actors.find((a) => !a.isNpc)!;
    await ask(
      `mutation ($worldId: UUID!, $actorId: UUID!, $checkId: String!) {
        rollCheck(worldId: $worldId, actorId: $actorId, checkId: $checkId) { formula }
      }`,
      { worldId: demoState().world.id, actorId: actor.id, checkId: "stealth" },
    );
    expect(demoState().rolls?.at(-1)?.label).toBe("Stealth");
  });
});

const ROLL_WHOLE = `query ($worldId: UUID!, $rollId: UUID!) {
  worldRoll(worldId: $worldId, rollId: $rollId) {
    __typename
    ... on WorldRoll {
      id bindings { placeholder value }
      resolution { dice { rolls steps finalValue } }
    }
    ... on MaskedRoll { id }
  }
}`;

async function wholeOf(rollId: string) {
  const worldId = demoState().world.id;
  return (await ask(ROLL_WHOLE, { worldId, rollId })).data?.worldRoll as Row;
}

/** Spec 083: what a board needs to throw a roll as the server rolled it. */
describe("dice on the screen", () => {
  it("answers a check's bindings, sorted, and a plain roll's as none", async () => {
    const actor = demoState().actors.find((a) => !a.isNpc)!;
    await ask(
      `mutation ($worldId: UUID!, $actorId: UUID!, $checkId: String!) {
        rollCheck(worldId: $worldId, actorId: $actorId, checkId: $checkId) { formula }
      }`,
      { worldId: demoState().world.id, actorId: actor.id, checkId: "stealth" },
    );
    const check = await wholeOf(String(demoState().rolls!.at(-1)!.id));
    expect(check.bindings).toEqual([
      { placeholder: "MODIFIER", value: expect.any(Number) },
    ]);

    await rollOf("1d20 + B + A", [
      { name: "B", value: 2 },
      { name: "A", value: 1 },
    ]);
    const bound = await wholeOf(String(demoState().rolls!.at(-1)!.id));
    expect(bound.bindings).toEqual([
      { placeholder: "A", value: 1 },
      { placeholder: "B", value: 2 },
    ]);

    const { id } = await rollAs({});
    expect((await wholeOf(id)).bindings).toEqual([]);
  });

  it("never answers a masked roll's bindings", async () => {
    demoState().viewer = "player";
    await rollAs({ visibility: "GM_EYES" });
    demoState().rolls!.push({
      ...demoState().rolls!.at(-1)!,
      id: OTHER,
      triggeredBy: "someone",
      bindings: { MODIFIER: 3 },
    });
    const masked = await wholeOf(OTHER);
    expect(masked.__typename).toBe("MaskedRoll");
    expect(masked).not.toHaveProperty("bindings");
  });

  it("says why each die's chain grew, and reads a roll from before as none", async () => {
    const { id } = await rollAs({ formula: "1d6xo>0" });
    const [die] = ((await wholeOf(id)).resolution as Row).dice as Row[];
    expect(die.rolls).toHaveLength(2);
    expect(die.steps).toEqual(["EXPLODE"]);

    const rerolled = await rollAs({ formula: "1d6r<7" });
    const [again] = ((await wholeOf(rerolled.id)).resolution as Row)
      .dice as Row[];
    expect(again.steps).toEqual(["REROLL"]);

    const before = resolutionRow(
      {
        formula: "1d6",
        dice: [
          { sides: { Numeric: 6 }, rolls: [2, 5], kept: true, final_value: 5 },
        ],
        kind: { Total: 5 },
      },
      null,
    );
    expect((before.dice as Row[])[0].steps).toEqual([]);
  });
});
