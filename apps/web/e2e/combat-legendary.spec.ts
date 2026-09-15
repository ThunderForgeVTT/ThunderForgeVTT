import { expect, test, type Page } from "./fixtures/test";
import {
  actFromTracker,
  addCombatant,
  advanceTurn,
  answerOffer,
  attackLogOn,
  budgetOn,
  claimFor,
  grantAbility,
  legendaryOn,
  openCombatPanel,
  rosterOn,
  setAbilityCost,
  setAbilityScores,
  setHitPoints,
  startCombat,
} from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
  type Point,
} from "../playtest/table";

/**
 * Spec 046 tasks Phase 9 (plan phase 7, quickstart "legendary and lair"; US6,
 * FR-050–FR-053, SC-006).
 *
 * A dragon with three legendary actions a round goes first. At the end of
 * each of three players' turns the Game Master spends one from the tracker —
 * a tail swipe, resolved like any other attack — and every seat's tracker
 * reads 2, then 1, then 0. At the dragon's own turn it has 3 again. A lair
 * the Game Master adds sits at initiative 20 and loses the tie to the dragon,
 * shows no budget, and acts through the tracker too; every seat's log names
 * it.
 */

const GRID = 64;
const SCORES = {
  strength: 14,
  dexterity: 12,
  constitution: 12,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};
const DRAGON = "Vermithrax";
const LAIR = "The Sunken Crypt";

function cell(q: number, r: number): Point {
  return { x: (q + 0.5) * GRID, y: (r + 0.5) * GRID };
}

/** Polls every board until `label`'s legendary pip reads `expected`. */
async function everySeatReads(
  boards: readonly (readonly [string, Page])[],
  label: string,
  expected: string | null,
  why: string,
): Promise<void> {
  for (const [who, board] of boards) {
    await expect
      .poll(async () => (await legendaryOn(board, label))?.text ?? null, {
        timeout: 15_000,
        message: `${who}'s tracker: ${why}`,
      })
      .toBe(expected);
  }
}

test("a legendary creature acts between turns, and a lair takes 20", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(10 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria", "Brom", "Cora"],
    sceneName: "The Dragon's Hoard",
  });
  const [aria, brom, cora] = table.players;

  try {
    await must(
      table.gm,
      `mutation ($sceneId: UUID!, $input: GraphQLUpdateSceneInput!) {
        updateScene(sceneId: $sceneId, input: $input) { sceneId }
      }`,
      {
        sceneId: table.sceneId,
        input: { gridSize: GRID, width: 1280, height: 1280 },
      },
    );

    const heroes = [];
    for (const [label, seat, q] of [
      ["Aria", aria, -4],
      ["Brom", brom, -2],
      ["Cora", cora, 0],
    ] as const) {
      const hero = await placeCast(table, { label, at: cell(q, 3), seat });
      await setAbilityScores(table, hero.actorId, {
        ...SCORES,
        armor_class: 14,
      });
      await setHitPoints(table, hero.actorId, { current: 30, max: 30 });
      await claimFor(table, seat, hero.actorId);
      heroes.push({ label, seat, ...hero });
    }
    const dragon = await placeCast(table, {
      label: DRAGON,
      at: cell(2, 0),
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 19 },
        hitPoints: { current: 200, max: 200 },
        // FR-050: the sheet says how many a round, through combat.legendary.
        traits: { class: "monster", level: 17, legendary_actions: 3 },
      },
    });
    const tail = await grantAbility(table, dragon.actorId, {
      name: "Tail Swipe",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "1" },
      ],
    });
    await setAbilityCost(table, tail, {
      actionCost: "LEGENDARY",
      legendaryCost: 1,
    });
    await grantAbility(table, dragon.actorId, {
      name: "Falling Rocks",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "2" },
      ],
    });

    const combat = await startCombat(table);
    await addCombatant(table, combat.id, {
      label: DRAGON,
      actorId: dragon.actorId,
      tokenId: dragon.tokenId,
      initiative: 20,
      isNpc: true,
    });
    for (const [index, hero] of heroes.entries()) {
      await addCombatant(table, combat.id, {
        label: hero.label,
        actorId: hero.actorId,
        tokenId: hero.tokenId,
        initiative: 15 - index * 5,
      });
    }
    await advanceTurn(table, combat.id); // the dragon's turn

    for (const client of [table.gm, aria.page, brom.page, cora.page]) {
      await sitDown(table, client);
      await openCombatPanel(client);
    }
    const boards = [
      ["the Game Master", table.gm],
      ["Aria", aria.page],
      ["Brom", brom.page],
      ["Cora", cora.page],
    ] as const;

    await test.step("every seat is shown the dragon's three legendary actions", async () => {
      await everySeatReads(
        boards,
        DRAGON,
        "Legendary 3/3",
        "three legendary actions, none spent",
      );
      for (const [who, board] of boards.slice(1)) {
        expect(
          await board.getByTestId("spend-legendary-action").count(),
          `${who} is not offered the Game Master's control`,
        ).toBe(0);
        expect(
          await legendaryOn(board, "Aria"),
          `${who} sees no legendary line on a hero who has none`,
        ).toBeNull();
      }
    });

    for (const [index, hero] of heroes.entries()) {
      const left = 2 - index;
      await test.step(`at the end of ${hero.label}'s turn the dragon swipes: ${left} left`, async () => {
        await advanceTurn(table, combat.id);
        const said = await actFromTracker(
          table.gm,
          DRAGON,
          "Tail Swipe",
          hero.label,
        );
        expect(said, "it resolves like any other attack (FR-051)").toMatch(
          new RegExp(
            `^${DRAGON} → ${hero.label} · Tail Swipe · \\d+ vs 14: hit`,
          ),
        );
        expect(said, "not its own turn, and not past zero").not.toMatch(
          /overspent|own turn/,
        );
        // The hero's player is offered the damage; the Game Master clears it
        // from their own board so the tracker stays in reach.
        await answerOffer(table.gm, hero.label, false);
        await everySeatReads(
          boards,
          DRAGON,
          `Legendary ${left}/3`,
          `${3 - left} spent between other creatures' turns`,
        );
        await expect
          .poll(async () => (await attackLogOn(hero.seat.page))[0] ?? "", {
            timeout: 15_000,
            message: `${hero.label}'s log shows the swipe`,
          })
          .toMatch(new RegExp(`^${DRAGON} → ${hero.label} · Tail Swipe`));
      });
    }

    await test.step("at the start of the dragon's own turn it has three again", async () => {
      const back = await advanceTurn(table, combat.id);
      expect(
        back.combatants.find((c) => c.id === back.activeCombatantId)?.label,
      ).toBe(DRAGON);
      await everySeatReads(
        boards,
        DRAGON,
        "Legendary 3/3",
        "refilled at the start of its turn (FR-052, SC-006)",
      );
    });

    await test.step("a lair the Game Master adds sits at 20 and loses the tie", async () => {
      await openCombatPanel(table.gm);
      await table.gm.getByTestId("combat-add-lair-name").fill(LAIR);
      await table.gm.getByTestId("combat-add-lair-button").click();
      const order = [DRAGON, LAIR, "Aria", "Brom", "Cora"];
      for (const [who, board] of boards) {
        await expect
          .poll(
            async () => {
              const rows = await rosterOn(board);
              return order.map((name) =>
                rows.findIndex((row) => row.includes(name)),
              );
            },
            {
              timeout: 15_000,
              message: `${who}'s tracker places the lair after the dragon at 20, before the heroes`,
            },
          )
          .toEqual([0, 1, 2, 3, 4]);
        const lairRow = board
          .getByTestId("combatant-row")
          .filter({ hasText: LAIR });
        await expect(lairRow.getByTestId("combatant-lair")).toBeVisible();
        expect(
          await budgetOn(board, LAIR),
          `${who} is shown no budget for a lair`,
        ).toBeNull();
      }
      // The initiative a Game Master reads is 20.
      await expect(
        table.gm
          .getByTestId("combatant-row")
          .filter({ hasText: LAIR })
          .getByRole("spinbutton"),
      ).toHaveValue("20");
      for (const [who, board] of boards.slice(1)) {
        await expect(
          board
            .getByTestId("combatant-row")
            .filter({ hasText: LAIR })
            .locator("span")
            .first(),
          `${who} reads the lair at 20`,
        ).toHaveText("20");
      }
    });

    await test.step("on its count the lair acts, and every seat is told it did", async () => {
      await advanceTurn(table, combat.id);
      const said = await actFromTracker(
        table.gm,
        LAIR,
        "Falling Rocks",
        "Brom",
      );
      expect(said).toMatch(
        new RegExp(`^${LAIR} → Brom · Falling Rocks · \\d+ vs 14: hit`),
      );
      await answerOffer(table.gm, "Brom", false);
      for (const [who, board] of boards) {
        await expect
          .poll(async () => (await attackLogOn(board))[0] ?? "", {
            timeout: 15_000,
            message: `${who}'s log names the lair`,
          })
          .toMatch(new RegExp(`^${LAIR} → Brom · Falling Rocks`));
      }
    });
  } finally {
    await closeTable(table);
  }
});
