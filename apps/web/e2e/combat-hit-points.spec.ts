import { expect, test, type Page } from "./fixtures/test";
import {
  addCombatant,
  advanceTurn,
  barCurrentOn,
  combatSeenBy,
  openCombatPanel,
  refusalOfChangeHitPoints,
  setAbilityScores,
  setDisclosure,
  setHitPoints,
  startCombat,
  type Combat,
} from "../playtest/combat";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";
import { expectOnEverySeatWithinOneSecond } from "./fixtures/seatTiming";

/**
 * Spec 046 tasks Phase 3 (plan phase 1): damage lands, bars move, zero is out.
 *
 * The independent test, played across three browsers: the goblin has 7; the
 * Game Master applies 5 from the tracker; both players' boards draw 2 without
 * a reload. Two more and it is out of the fight, marked so, and the turn
 * passes over it. Healed 3, it is back. Taken out by the Game Master's own
 * Down, healing leaves it out.
 *
 * Driven through the tracker's buttons on the Game Master's board, because
 * the claim is that a Game Master *can* do this, and read from each client's
 * engine, because the claim is that every board *shows* it. The server's
 * arithmetic is `combat::hit_points`' tests' to pin.
 *
 * Built on the playtest's table helpers (`apps/web/playtest/`), which already
 * seat a Game Master and players the way a session does.
 */

const GOBLIN_HP = 7;
const SCORES = {
  strength: 8,
  dexterity: 14,
  constitution: 10,
  intelligence: 10,
  wisdom: 8,
  charisma: 8,
};

/** The goblin's row on the Game Master's tracker. */
function goblinRow(page: Page) {
  return page
    .getByTestId("combatant-row")
    .filter({ hasText: "Goblin" })
    .first();
}

async function applyFromTracker(
  page: Page,
  kind: "Damage" | "Heal",
  amount: number,
): Promise<void> {
  const row = goblinRow(page);
  await row.getByTestId("combatant-hp-amount").fill(String(amount));
  await row.getByRole("button", { name: `${kind} Goblin` }).click();
}

test("a Game Master's damage moves every board's bars, and zero takes a creature out", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(6 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria", "Brom"],
    sceneName: "The Goblin Hole",
  });
  const [aria, brom] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: -200, y: 0 },
      seat: aria,
    });
    const caster = await placeCast(table, {
      label: "Brom",
      at: { x: -200, y: -150 },
      seat: brom,
    });
    // An NPC's token is a copy of its sheet as placed (spec 046 ADR-102), so
    // the goblin is written before it is put on the board.
    const goblin = await placeCast(table, {
      label: "Goblin",
      at: { x: 200, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: SCORES,
        hitPoints: { current: GOBLIN_HP, max: GOBLIN_HP },
      },
    });
    for (const cast of [hero, caster]) {
      await setAbilityScores(table, cast.actorId, SCORES);
    }
    await setHitPoints(table, hero.actorId, { current: 12, max: 12 });
    await setHitPoints(table, caster.actorId, { current: 9, max: 9 });
    // The players are shown the goblin's own figures, so "their bar reads 2"
    // is a number on their board and not an estimate.
    await setDisclosure(table, goblin.tokenId, "hitPoints", "VISIBLE");

    let combat: Combat = await startCombat(table);
    combat = await addCombatant(table, combat.id, {
      label: "Aria",
      actorId: hero.actorId,
      tokenId: hero.tokenId,
      initiative: 18,
    });
    combat = await addCombatant(table, combat.id, {
      label: "Goblin",
      actorId: goblin.actorId,
      tokenId: goblin.tokenId,
      initiative: 12,
      isNpc: true,
    });
    combat = await addCombatant(table, combat.id, {
      label: "Brom",
      actorId: caster.actorId,
      tokenId: caster.tokenId,
      initiative: 6,
    });
    combat = await advanceTurn(table, combat.id);

    for (const client of [table.gm, aria.page, brom.page]) {
      await sitDown(table, client);
    }
    await openCombatPanel(table.gm);
    const everyone: [string, Page][] = [
      ["the Game Master", table.gm],
      ["Aria", aria.page],
      ["Brom", brom.page],
    ];
    for (const [who, client] of everyone) {
      await expect
        .poll(() => barCurrentOn(client, goblin.tokenId), {
          timeout: 30_000,
          message: `${who}'s board draws the goblin at ${GOBLIN_HP}`,
        })
        .toBe(GOBLIN_HP);
    }

    await test.step("a player may not change hit points by hand", async () => {
      const refusal = await refusalOfChangeHitPoints(aria.page, goblin.tokenId);
      expect(refusal.join(" ")).toContain("Only the Game Master");
    });

    await test.step("5 damage: every board draws 2 within one second, with no reload", async () => {
      // SC-002 and FR-013, asserted: from the Game Master's click (the amount
      // typed beforehand, off the clock) to the
      // slowest of the three boards. Over a second is measured once more,
      // healing the 5 back first so the same blow can land again.
      await expectOnEverySeatWithinOneSecond(testInfo, {
        what: "bars moved",
        seats: everyone,
        clock: "before-act",
        prepare: () =>
          goblinRow(table.gm).getByTestId("combatant-hp-amount").fill("5"),
        act: () =>
          goblinRow(table.gm)
            .getByRole("button", { name: "Damage Goblin" })
            .click(),
        shown: async (client) =>
          (await barCurrentOn(client, goblin.tokenId)) === 2,
        describe: async (client) =>
          `bar reads ${await barCurrentOn(client, goblin.tokenId)}`,
        again: async () => {
          await applyFromTracker(table.gm, "Heal", 5);
          for (const [who, client] of everyone) {
            await expect
              .poll(() => barCurrentOn(client, goblin.tokenId), {
                timeout: 10_000,
                message: `${who}'s bar is back to ${GOBLIN_HP} before the blow is measured again`,
              })
              .toBe(GOBLIN_HP);
          }
        },
      });
      const seen = await combatSeenBy(table.gm, table.worldId);
      expect(
        seen!.combatants.find((c) => c.label === "Goblin")!.active,
        "2 hit points is still in the fight",
      ).toBe(true);
    });

    await test.step("2 more: out of the fight, marked, and skipped", async () => {
      await applyFromTracker(table.gm, "Damage", 2);
      await expect(
        goblinRow(table.gm).getByTestId("combatant-out"),
        "the Game Master's tracker says why the goblin is out",
      ).toHaveText("Out: 0 hit points", { timeout: 10_000 });
      for (const seat of [aria, brom]) {
        await expect
          .poll(
            async () =>
              (await combatSeenBy(seat.page, table.worldId))!.combatants.find(
                (c) => c.label === "Goblin",
              )?.downedBy,
            { timeout: 10_000, message: `${seat.name} sees the goblin out` },
          )
          .toBe("HIT_POINTS");
        await expect
          .poll(() => barCurrentOn(seat.page, goblin.tokenId), {
            timeout: 5_000,
          })
          .toBe(0);
      }
      for (let turn = 0; turn < 4; turn += 1) {
        combat = await advanceTurn(table, combat.id);
        const active = combat.combatants.find(
          (c) => c.id === combat.activeCombatantId,
        );
        expect(active?.label, "the turn passes over the goblin").not.toBe(
          "Goblin",
        );
      }
    });

    await test.step("healed 3, it is back in the order", async () => {
      await applyFromTracker(table.gm, "Heal", 3);
      await expect(
        goblinRow(table.gm).getByTestId("combatant-out"),
      ).toHaveCount(0, { timeout: 10_000 });
      for (const [who, client] of everyone) {
        await expect
          .poll(() => barCurrentOn(client, goblin.tokenId), {
            timeout: 5_000,
            message: `${who}'s bar reads 3`,
          })
          .toBe(3);
      }
      const seen = await combatSeenBy(brom.page, table.worldId);
      const row = seen!.combatants.find((c) => c.label === "Goblin")!;
      expect(row.active).toBe(true);
      expect(row.downedBy).toBeNull();

      let reached = false;
      for (let turn = 0; turn < 3 && !reached; turn += 1) {
        combat = await advanceTurn(table, combat.id);
        reached =
          combat.combatants.find((c) => c.id === combat.activeCombatantId)
            ?.label === "Goblin";
      }
      expect(reached, "the goblin takes a turn again").toBe(true);
    });

    await test.step("a Game Master's Down is not undone by healing", async () => {
      await goblinRow(table.gm)
        .getByRole("button", { name: "Mark Goblin down" })
        .click();
      await expect(goblinRow(table.gm).getByTestId("combatant-out")).toHaveText(
        "Down",
        { timeout: 10_000 },
      );
      await applyFromTracker(table.gm, "Heal", 2);
      await expect
        .poll(() => barCurrentOn(aria.page, goblin.tokenId), {
          timeout: 5_000,
        })
        .toBe(5);
      const seen = await combatSeenBy(table.gm, table.worldId);
      const row = seen!.combatants.find((c) => c.label === "Goblin")!;
      expect(row.active, "healed, and still down").toBe(false);
      expect(row.downedBy).toBe("GAME_MASTER");
      await expect(goblinRow(table.gm).getByTestId("combatant-out")).toHaveText(
        "Down",
      );
    });
  } finally {
    await closeTable(table);
  }
});
