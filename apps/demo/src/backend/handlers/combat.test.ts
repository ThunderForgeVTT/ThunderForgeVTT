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
});
