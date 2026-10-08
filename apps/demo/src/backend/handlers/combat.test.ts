/**
 * Spec 079: the demo's fight, through the same executor the page asks.
 *
 * Every answer here is the combat crate's, compiled to wasm and loaded from
 * `dist/combat` (the build step of T042). Node cannot fetch the wasm the way
 * the page does, so the test hands it the bytes first; `rules()` then finds
 * the module already started.
 */
import { readFileSync } from "node:fs";
import { beforeAll, describe, expect, it, vi } from "vitest";

vi.hoisted(() => {
  // `state.ts` keeps the world in `window.localStorage`; this one is empty,
  // but for a fixed seed for the dice.
  Object.assign(globalThis, {
    window: {
      addEventListener() {},
      dispatchEvent() {
        return true;
      },
      localStorage: {
        getItem: (key: string) =>
          key === "thunderforge-demo:dice-seed" ? "79" : null,
        setItem() {},
        removeItem() {},
      },
      location: {
        href: "http://demo.test/demo/",
        origin: "http://demo.test",
      },
    },
  });
});

import { initSync } from "@thunderforge/combat";
import { runOperation } from "../execute";
import { demoState, loadState, type Row } from "../state";
import { loadDiceForTest, seedDice } from "./dice";
import { isMelee, shapedDamage } from "./combatAttacks";
import { rules } from "./combatRules";
import type { MapListing } from "../../seed/world";

const ROOT = new URL("../../../../../", import.meta.url);
const maps = JSON.parse(
  readFileSync(new URL("apps/demo/public/maps/maps.json", ROOT), "utf8"),
) as MapListing[];
const fetchStatic = (async () =>
  new Response(JSON.stringify(maps))) as unknown as typeof fetch;

const AMBUSH = "d0000000-0000-4000-0003-000000000001";

async function ask(query: string, variables: Row = {}): Promise<Row> {
  const answer = await runOperation({ query, variables });
  if (answer.errors?.length) throw new Error(answer.errors[0].message);
  return answer.data as Row;
}

async function refusal(query: string, variables: Row = {}) {
  const answer = await runOperation({ query, variables });
  return answer.errors?.[0];
}

const COMBAT = `id round activeCombatantId endedAt effectiveAutoApply roundLabel
  combatants { id label tokenId actorId initiative active downedBy kind
    budget { action { allowed spent remaining } unit } }`;

function actorId(castKey: string): string {
  return demoState().actors.find((a) => a.castKey === castKey)?.id as string;
}

function tokenNamed(name: string): Row {
  const token = demoState().tokens.find(
    (t) =>
      t.sceneId === AMBUSH &&
      ((t.metadata as Row | null)?.label ?? t.name) === name,
  );
  if (!token) throw new Error(`no token named ${name}`);
  return token;
}

async function hitPoints(tokenId: string): Promise<Row | undefined> {
  const { tokenStatus } = await ask(
    `query($s: UUID!) { tokenStatus(sceneId: $s) { tokenId resources {
       definitionId disclosure quarter entries { current max } } } }`,
    { s: AMBUSH },
  );
  return (tokenStatus as Row[]).find((t) => t.tokenId === tokenId);
}

beforeAll(async () => {
  initSync({
    module: readFileSync(new URL("dist/combat/combat_bg.wasm", ROOT)),
  });
  loadDiceForTest(readFileSync(new URL("dist/dice/dice_bg.wasm", ROOT)));
  await loadState(fetchStatic, "/demo/");
});

describe("a fight in the demo", () => {
  let combatId = "";

  it("starts at round 1 with the initiative order rolled and sorted", async () => {
    const { startCombat } = await ask(
      `mutation($w: UUID!, $s: UUID) { startCombat(input: { worldId: $w, sceneId: $s }) { ${COMBAT} } }`,
      { w: demoState().world.id, s: AMBUSH },
    );
    combatId = (startCombat as Row).id as string;
    expect((startCombat as Row).round).toBe(1);
    expect((startCombat as Row).roundLabel).toBe("Round");

    for (const [key, label] of [
      ["fighter", "Brannoc Stoneward"],
      ["goblin", "Goblin Warrior"],
      ["goblin", "Goblin Warrior"],
    ]) {
      await ask(
        `mutation($c: UUID!, $a: UUID!, $l: String!) {
           addCombatant(input: { combatId: $c, actorId: $a, label: $l }) { id } }`,
        { c: combatId, a: actorId(key), l: label },
      );
    }
    const { activeCombat } = await ask(
      `query($w: UUID!) { activeCombat(worldId: $w) { ${COMBAT} } }`,
      { w: demoState().world.id },
    );
    const seats = (activeCombat as Row).combatants as Row[];
    // An actor-only seat stands for its first token not already seated.
    expect(seats.map((s) => s.label).sort()).toEqual([
      "Brannoc Stoneward",
      "Goblin 1",
      "Goblin 2",
    ]);
    expect(seats.every((s) => s.tokenId)).toBe(true);
    const order = seats.map((s) => s.initiative as number);
    expect(order).toEqual([...order].sort((a, b) => b - a));
    expect((seats[0].budget as Row).unit).toBe("ft");
  });

  it("goes round the order into round 2", async () => {
    const advance = `mutation($c: UUID!) { advanceTurn(combatId: $c) { round activeCombatantId } }`;
    let combat: Row = {};
    for (let i = 0; i < 4; i += 1) {
      combat = (await ask(advance, { c: combatId })).advanceTurn as Row;
    }
    expect(combat.round).toBe(2);
  });

  it("rolls an attack against the target's defence, and a hit offers damage", async () => {
    const brannoc = tokenNamed("Brannoc Stoneward");
    const goblin = tokenNamed("Goblin 1");
    const { actorAbilities } = await ask(
      `query($a: UUID!) { actorAbilities(actorId: $a) { abilityId abilityName } }`,
      { a: actorId("fighter") },
    );
    const sword = (actorAbilities as Row[])[0];
    expect(sword.abilityName).toBe("Longsword");

    const { previewAttack } = await ask(
      `query($i: AttackInput!) { previewAttack(input: $i) { flags turn { allowed } reach unit } }`,
      {
        i: {
          attackerTokenId: brannoc.tokenId,
          abilityId: sword.abilityId,
          targetTokenId: goblin.tokenId,
        },
      },
    );
    expect((previewAttack as Row).reach).toBe(5);
    expect((previewAttack as Row).unit).toBe("ft");

    const before = await hitPoints(goblin.tokenId as string);
    const { makeAttack } = await ask(
      `mutation($i: AttackInput!) { makeAttack(input: $i) {
         id outcome defence abilityName attacker { label } target { label }
         toHit { resultValue dice { numericSides rolls } }
         damage { resultValue } offer { id amount status } } }`,
      {
        i: {
          attackerTokenId: brannoc.tokenId,
          abilityId: sword.abilityId,
          targetTokenId: goblin.tokenId,
        },
      },
    );
    const [attack] = makeAttack as Row[];
    expect(attack.defence).toBe(15);
    expect(attack.target).toEqual({ label: "Goblin 1" });
    expect((attack.toHit as Row).dice).toEqual([
      expect.objectContaining({ numericSides: 20 }),
    ]);
    const total = (attack.toHit as Row).resultValue as number;
    expect(attack.outcome).toBe(total >= 15 ? "HIT" : "MISS");

    if (attack.outcome === "HIT") {
      const offer = attack.offer as Row;
      expect(offer.status).toBe("PENDING");
      await ask(
        `mutation($o: UUID!) { resolveOffer(offerId: $o, take: true) { status } }`,
        { o: offer.id },
      );
      const after = await hitPoints(goblin.tokenId as string);
      const hp = (s?: Row) =>
        ((s?.resources as Row[])[0].entries as Row[])[0].current as number;
      expect(hp(after)).toBe(
        Math.max(0, hp(before) - (offer.amount as number)),
      );
    } else {
      expect(attack.damage).toBeNull();
      expect(attack.offer).toBeNull();
    }
    const { sceneAttacks } = await ask(
      `query($s: UUID!) { sceneAttacks(sceneId: $s) { id } }`,
      { s: AMBUSH },
    );
    expect((sceneAttacks as Row[])[0].id).toBe(attack.id);
  });

  it("rolls a to-hit with disadvantage and judges it on the die kept", async () => {
    // Spec 084 (FR-019): as `record_attack` shapes the to-hit. A reaction,
    // so the turn the previous test spent is not in the way.
    const brannoc = tokenNamed("Brannoc Stoneward");
    const goblin = tokenNamed("Goblin 1");
    const { actorAbilities } = await ask(
      `query($a: UUID!) { actorAbilities(actorId: $a) { abilityId } }`,
      { a: actorId("fighter") },
    );
    const sword = (actorAbilities as Row[])[0];
    const before = demoState().rolls?.length ?? 0;
    const { makeAttack } = await ask(
      `mutation($i: AttackInput!) { makeAttack(input: $i) {
         outcome defence toHit { formula resultValue dice { numericSides kept finalValue } } } }`,
      {
        i: {
          attackerTokenId: brannoc.tokenId,
          abilityId: sword.abilityId,
          targetTokenId: goblin.tokenId,
          actionCost: "REACTION",
          advantage: "DISADVANTAGE",
        },
      },
    );
    const [attack] = makeAttack as Row[];
    const toHit = attack.toHit as Row;
    expect(toHit.formula).toContain("2d20kl1");
    const d20s = (toHit.dice as Row[]).filter((d) => d.numericSides === 20);
    expect(d20s).toHaveLength(2);
    const kept = d20s.find((d) => d.kept)!;
    expect(kept.finalValue).toBe(
      Math.min(...d20s.map((d) => d.finalValue as number)),
    );
    const total = toHit.resultValue as number;
    const natural = kept.finalValue as number;
    if (natural !== 1 && natural !== 20) {
      expect(attack.outcome).toBe(
        total >= (attack.defence as number) ? "HIT" : "MISS",
      );
    }
    const records = demoState().rolls!.slice(before);
    expect(records[0]).toMatchObject({
      rollKind: "to_hit",
      facets: ["disadvantage"],
    });
    for (const damage of records.slice(1)) {
      expect(damage).toMatchObject({ rollKind: "damage", facets: [] });
    }
  });

  it("puts a creature at 0 hit points out of the order", async () => {
    const goblin = tokenNamed("Goblin 2");
    const { changeHitPoints } = await ask(
      `mutation($t: UUID!) { changeHitPoints(tokenId: $t, kind: DAMAGE, amount: 100) { current max } }`,
      { t: goblin.tokenId },
    );
    expect((changeHitPoints as Row).current).toBe(0);
    const { activeCombat } = await ask(
      `query($w: UUID!) { activeCombat(worldId: $w) { ${COMBAT} } }`,
      { w: demoState().world.id },
    );
    const seat = ((activeCombat as Row).combatants as Row[]).find(
      (s) => s.tokenId === goblin.tokenId,
    );
    expect(seat).toMatchObject({ active: false, downedBy: "HIT_POINTS" });
  });

  it("keeps the whole fight in the saved world", () => {
    const saved = JSON.parse(JSON.stringify(demoState())) as Row;
    expect(saved.fight).toEqual(demoState().fight);
    expect((saved.fight as Row).seed).toBe(79);
  });

  it("tells a player neither a monster's hit points nor a hidden name", async () => {
    demoState().viewer = "player";
    try {
      const goblin = tokenNamed("Goblin 1");
      const brannoc = tokenNamed("Brannoc Stoneward");
      // A hidden ambusher stands on the player's board like any other
      // figure (the server sends every token); its health is a quarter.
      const hidden = (
        (await hitPoints(goblin.tokenId as string))?.resources as Row[]
      )[0];
      expect(hidden.disclosure).toBe("chunked");
      expect(hidden.entries).toBeNull();
      const hero = await hitPoints(brannoc.tokenId as string);
      expect((hero?.resources as Row[])[0].disclosure).toBe("visible");

      // Revealed, a goblin's hit points are a quarter, never a figure.
      const actor = demoState().actors.find(
        (a) => a.castKey === "goblin",
      ) as Row;
      actor.visibleToPlayers = true;
      const status = await hitPoints(goblin.tokenId as string);
      const resource = (status?.resources as Row[])[0];
      expect(resource.disclosure).toBe("chunked");
      expect(resource.entries).toBeNull();
      expect(typeof resource.quarter).toBe("number");

      const { activeCombat } = await ask(
        `query($w: UUID!) { activeCombat(worldId: $w) { combatants { label tokenId } } }`,
        { w: demoState().world.id },
      );
      const labels = ((activeCombat as Row).combatants as Row[]).map(
        (s) => s.label,
      );
      expect(labels).toContain("Brannoc Stoneward");
      expect(labels).not.toContain("Goblin 1");
      expect(labels).toContain("Unknown");

      const { sceneAttacks } = await ask(
        `query($s: UUID!) { sceneAttacks(sceneId: $s) { target { label tokenId } defence } }`,
        { s: AMBUSH },
      );
      expect((sceneAttacks as Row[])[0]).toEqual({
        target: { label: "Unknown", tokenId: null },
        defence: null,
      });
      actor.visibleToPlayers = false;
    } finally {
      demoState().viewer = "gm";
    }
  });

  it("refuses a player who acts out of turn or for someone else's creature", async () => {
    demoState().viewer = "player";
    try {
      const error = await refusal(
        `mutation($c: UUID!) { advanceTurn(combatId: $c) { round } }`,
        { c: combatId },
      );
      expect(error?.message).toBe("Only the GM may advance the turn");
    } finally {
      demoState().viewer = "gm";
    }
  });

  it("does not let an ability's attack fields be written", async () => {
    for (const field of [
      "setAbilityAttack(abilityId: $id",
      "setItemAttack(itemId: $id",
    ]) {
      const error = await refusal(
        `mutation($id: UUID!) { ${field}, attack: { needsLineOfSight: true, actionCost: ACTION, legendaryCost: 0, multiattack: [] }) }`,
        { id: AMBUSH },
      );
      expect(error?.extensions?.code).toBe("NOT_IN_DEMO");
    }
  });

  it("ends", async () => {
    await ask(`mutation($c: UUID!) { endCombat(combatId: $c) { endedAt } }`, {
      c: combatId,
    });
    const { activeCombat } = await ask(
      `query($w: UUID!) { activeCombat(worldId: $w) { id } }`,
      { w: demoState().world.id },
    );
    expect(activeCombat).toBeNull();
  });

  /** The goblins' Armor Class, as the sheet holds it. */
  function armourClass(ac: number) {
    const sheet = demoState().systemData.find(
      (row) => row.actorId === actorId("goblin"),
    )!;
    sheet.abilityData = { ...(sheet.abilityData as Row), armor_class: ac };
  }

  function inspire(key: string): Row {
    const sheet = demoState().systemData.find(
      (row) => row.actorId === actorId(key),
    )!;
    sheet.traitData = { ...(sheet.traitData as Row), inspiration: true };
    return sheet;
  }

  /** Brannoc's longsword at Goblin 2, which cannot land against AC 99. */
  async function aMiss(): Promise<Row> {
    armourClass(99);
    inspire("fighter");
    const { actorAbilities } = await ask(
      `query($a: UUID!) { actorAbilities(actorId: $a) { abilityId } }`,
      { a: actorId("fighter") },
    );
    const { makeAttack } = await ask(
      `mutation($i: AttackInput!) { makeAttack(input: $i) { id outcome defence } }`,
      {
        i: {
          attackerTokenId: tokenNamed("Brannoc Stoneward").tokenId,
          abilityId: (actorAbilities as Row[])[0].abilityId,
          targetTokenId: tokenNamed("Goblin 2").tokenId,
        },
      },
    );
    const [attack] = makeAttack as Row[];
    expect(attack).toMatchObject({ outcome: "MISS", defence: 99 });
    return demoState().fight!.attacks.find((a) => a.id === attack.id)!;
  }

  const REROLL = `mutation($w: UUID!, $r: UUID!) {
    rerollRoll(worldId: $w, rollId: $r, spend: "inspiration") { id rerollOf } }`;
  const ATTACKS = `query($s: UUID!) { sceneAttacks(sceneId: $s) {
    id outcome defence rerollOf damage { resultValue } offer { amount } } }`;

  it("rerolls a missed attack with Inspiration into a hit, judged on the defence it was made against", async () => {
    // Spec 084 T059: as `reroll_attack`. The miss was made against AC 99;
    // the row is given the defence 1 that a d20 cannot miss, and the
    // goblin keeps its 99, so only the row's defence can make it a hit.
    demoState().viewer = "gm";
    seedDice([5, 6, 7, 8]);
    const first = await aMiss();
    first.defence = 1;
    const offers = demoState().fight!.offers.length;

    const { rerollRoll } = await ask(REROLL, {
      w: demoState().world.id,
      r: first.toHitRollId,
    });
    expect((rerollRoll as Row).rerollOf).toBe(first.toHitRollId);
    const { sceneAttacks } = await ask(ATTACKS, { s: AMBUSH });
    const shown = (sceneAttacks as Row[]).find((a) => a.rerollOf === first.id)!;
    expect(shown).toMatchObject({ outcome: "HIT", defence: 1 });
    expect(shown.damage).not.toBeNull();
    expect(shown.offer).not.toBeNull();
    expect(demoState().fight!.offers.length).toBe(offers + 1);
    const sheet = demoState().systemData.find(
      (row) => row.actorId === actorId("fighter"),
    )!;
    expect((sheet.traitData as Row).inspiration).toBe(false);
    // The hit's to-hit is never rerolled, whatever is left to spend.
    const second = demoState().fight!.attacks.find(
      (a) => a.rerollOf === first.id,
    )!;
    inspire("fighter");
    const hit = await refusal(REROLL, {
      w: demoState().world.id,
      r: second.toHitRollId,
    });
    expect(hit?.message).toBe("A hit cannot be rerolled.");
  });

  it("rerolls a miss that misses again: no damage, no offer, Inspiration spent", async () => {
    demoState().viewer = "gm";
    seedDice([9, 10, 11, 12]);
    const first = await aMiss();
    const offers = demoState().fight!.offers.length;
    const rolls = demoState().rolls!.length;

    await ask(REROLL, { w: demoState().world.id, r: first.toHitRollId });
    const second = demoState().fight!.attacks.find(
      (a) => a.rerollOf === first.id,
    )!;
    expect(second.outcome).toBe("MISS");
    expect(second.damage).toBeNull();
    expect(demoState().fight!.offers.length).toBe(offers);
    expect(demoState().rolls!.length).toBe(rolls + 1);
    const sheet = demoState().systemData.find(
      (row) => row.actorId === actorId("fighter"),
    )!;
    expect((sheet.traitData as Row).inspiration).toBe(false);
    // A rerolled miss is not rerolled again, and a hit is never offered.
    const again = await refusal(REROLL, {
      w: demoState().world.id,
      r: first.toHitRollId,
    });
    expect(again?.message).toBe("This roll has already been rerolled.");
    armourClass(15);
  });

  it("rolls a seeded [1, 5] greatsword hit as [3, 5] with Great Weapon Fighting", async () => {
    // Spec 084 T067: as `roll_hit_damage`. A seed whose `2d6` rolls [1, 5]
    // is found, then the shaped damage is rolled from the same seed.
    const r = await rules();
    const roll = (seed: number, formula: string) => {
      const roller = r.Dice.seeded(seed);
      const { resolution } = JSON.parse(r.roll(roller, formula, "{}")) as {
        resolution: { dice: { rolls: number[]; final_value: number }[] };
      };
      roller.free();
      return resolution.dice;
    };
    const seed = Array.from({ length: 10_000 }, (_, i) => i).find(
      (s) => JSON.stringify(roll(s, "2d6").map((d) => d.rolls)) === "[[1],[5]]",
    )!;
    expect(seed).toBeDefined();
    const traitData = { facets: ["great_weapon_fighting"] };
    const finals = (melee: boolean, properties: string[]) => {
      const shaped = shapedDamage(["2d6"], traitData, melee, properties);
      return {
        values: roll(seed, shaped.formulas[0]).map((d) => d.final_value),
        facets: shaped.facets,
      };
    };
    const reach = {
      reach: 5,
      rangeNormal: null,
      rangeLong: null,
      needsLineOfSight: false,
    };
    expect(isMelee(5, reach)).toBe(true);
    expect(finals(isMelee(5, reach), ["two_handed"])).toEqual({
      values: [3, 5],
      facets: ["great_weapon_fighting"],
    });
    // At range, or one-handed, the one stands.
    expect(isMelee(30, reach)).toBe(false);
    expect(finals(isMelee(30, reach), ["two_handed"])).toEqual({
      values: [1, 5],
      facets: [],
    });
    expect(finals(true, ["versatile"])).toEqual({ values: [1, 5], facets: [] });
  });
});
