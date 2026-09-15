import { expect, test, type Page } from "./fixtures/test";
import { dragToken, serverTokenPosition } from "./fixtures/offline";
import {
  addCombatant,
  advanceTurn,
  answerOffer,
  attackLogOn,
  attackWarningsFromSheet,
  budgetOn,
  budgetTextOn,
  claimFor,
  grantAbility,
  makeAttackAs,
  openCombatPanel,
  setAbilityReach,
  setAbilityScores,
  setHitPoints,
  startCombat,
} from "../playtest/combat";
import {
  closeTable,
  expectAgreed,
  must,
  openTable,
  placeCast,
  sitDown,
  walk,
  type Point,
} from "../playtest/table";

/**
 * Spec 046 tasks Phase 8 (plan phase 6, quickstart "the economy of a round";
 * US5, SC-005, C9).
 *
 * On Aria's turn her action, bonus action, reaction and movement are shown
 * unspent on every seat. She attacks: her action is spent everywhere. She
 * attacks again: not refused, and every seat is shown the overspend as a
 * debt. She walks four squares by keyboard: 10 ft of her 30 left. On the
 * goblin's turn she takes a reaction, which stays spent through the ogre's
 * turn; a Game Master dragging the goblin spends the goblin's movement. When
 * the turn comes back to her, her budget is fresh — and nobody else's changed.
 * A combatant whose name is hidden shows its budget under "Unknown".
 *
 * The scene is given a 64-unit grid anchored to a 1280-square map (as in
 * `combat-reach.spec.ts`), so a square is 5 ft and every position is a whole
 * number of squares. The longsword hits on any die for a fixed 5.
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
const OGRE_NAME = "Grukk Stonebelly";
const FRESH = "Action 1/1 · Bonus 1/1 · Reaction 1/1 · Move 30/30 ft";

/** The centre of the one-square cell `(q, r)`, counted from the origin vertex. */
function cell(q: number, r: number): Point {
  return { x: (q + 0.5) * GRID, y: (r + 0.5) * GRID };
}

/** Polls every board until `label`'s budget reads `expected`. */
async function everySeatReads(
  boards: readonly (readonly [string, Page])[],
  label: string,
  expected: string,
  why: string,
): Promise<void> {
  for (const [who, board] of boards) {
    await expect
      .poll(() => budgetTextOn(board, label), {
        timeout: 15_000,
        message: `${who}'s tracker: ${why}`,
      })
      .toBe(expected);
  }
}

test("a round is an economy, shown on every seat and never refused", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(10 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria", "Brom"],
    sceneName: "The Toll Bridge",
  });
  const [aria, brom] = table.players;
  // The engine's snap is float arithmetic: 96.00001 is square 1.
  const roundedServerPosition = async (tokenId: string) => {
    const at = await serverTokenPosition(table.gm, table.sceneId, tokenId);
    return at && { x: Math.round(at.x), y: Math.round(at.y) };
  };

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

    const hero = await placeCast(table, {
      label: "Aria",
      at: cell(-5, 2),
      seat: aria,
    });
    await placeCast(table, { label: "Brom", at: cell(-5, -3), seat: brom });
    const goblin = await placeCast(table, {
      label: "Goblin",
      at: cell(0, 0),
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 13 },
        hitPoints: { current: 7, max: 7 },
      },
    });
    const ogre = await placeCast(table, {
      label: OGRE_NAME,
      at: cell(4, 0),
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 11 },
        hitPoints: { current: 59, max: 59 },
      },
    });
    await must(
      table.gm,
      `mutation ($tokenId: UUID!) {
        setTokenNameVisibility(tokenId: $tokenId, visible: false) { tokenId }
      }`,
      { tokenId: ogre.tokenId },
    );
    await setAbilityScores(table, hero.actorId, { ...SCORES, armor_class: 14 });
    await setHitPoints(table, hero.actorId, { current: 16, max: 16 });
    await claimFor(table, aria, hero.actorId);
    const longsword = await grantAbility(table, hero.actorId, {
      name: "Longsword",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "5" },
      ],
    });
    // A reach that covers the room, so the only flag this spec reads is its own.
    await setAbilityReach(table, longsword, { reach: 1000 });

    const combat = await startCombat(table);
    for (const [label, cast, initiative] of [
      ["Aria", hero, 20],
      ["Goblin", goblin, 10],
      [OGRE_NAME, ogre, 5],
    ] as const) {
      await addCombatant(table, combat.id, {
        label,
        actorId: cast.actorId,
        tokenId: cast.tokenId,
        initiative,
        isNpc: label !== "Aria",
      });
    }
    await advanceTurn(table, combat.id);

    for (const client of [table.gm, aria.page, brom.page]) {
      await sitDown(table, client);
      await openCombatPanel(client);
    }
    const boards = [
      ["the Game Master", table.gm],
      ["Aria", aria.page],
      ["Brom", brom.page],
    ] as const;
    const players = [
      ["Aria", aria.page],
      ["Brom", brom.page],
    ] as const;

    await test.step("on Aria's turn, all four lines are unspent on every seat", async () => {
      await everySeatReads(boards, "Aria", FRESH, "Aria starts her turn fresh");
      await everySeatReads(
        boards,
        "Goblin",
        FRESH,
        "and every creature's budget is shown, not only the active one's",
      );
      // The ogre's name is hidden: players read "Unknown", with its budget.
      await everySeatReads(
        players,
        "Unknown",
        FRESH,
        "a hidden creature's budget is shown under Unknown",
      );
      await everySeatReads(
        [["the Game Master", table.gm]],
        OGRE_NAME,
        FRESH,
        "the Game Master sees the ogre by name",
      );
      for (const [who, board] of players) {
        expect(
          (await board.getByTestId("combat-panel").textContent()) ?? "",
          `${who}'s tracker does not name the hidden ogre`,
        ).not.toContain(OGRE_NAME);
      }
    });

    await test.step("she attacks: her action is spent, everywhere", async () => {
      const { warnings, said } = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Longsword",
        "Goblin",
      );
      expect(warnings, "nothing to warn about").toEqual([]);
      expect(said).toMatch(/^Aria → Goblin · Longsword · \d+ vs 13: hit/);
      await answerOffer(table.gm, "Goblin", false);
      await openCombatPanel(aria.page);
      await everySeatReads(
        boards,
        "Aria",
        "Action 0/1 · Bonus 1/1 · Reaction 1/1 · Move 30/30 ft",
        "Aria's action is spent",
      );
    });

    await test.step("she attacks again: warned, not refused, and shown as a debt", async () => {
      const { warnings, said } = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Longsword",
        "Goblin",
      );
      expect(warnings, "warned before rolling").toEqual(["Overspent"]);
      expect(said, "made, and flagged (C9)").toMatch(
        /^Aria → Goblin · Longsword · \d+ vs 13: hit · 5 damage offered · overspent$/,
      );
      await answerOffer(table.gm, "Goblin", false);
      await openCombatPanel(aria.page);
      await everySeatReads(
        boards,
        "Aria",
        "Action −1/1 · Bonus 1/1 · Reaction 1/1 · Move 30/30 ft",
        "Aria's second action shows as one over",
      );
      for (const [who, board] of boards) {
        const shown = await budgetOn(board, "Aria");
        expect(
          shown?.action,
          `${who} is shown two spent of one, marked overspent`,
        ).toMatchObject({
          allowed: 1,
          spent: 2,
          remaining: -1,
          overspent: true,
        });
        await expect
          .poll(async () => (await attackLogOn(board))[0] ?? "", {
            timeout: 5_000,
            message: `${who}'s log flags the second swing`,
          })
          .toMatch(/^Aria → Goblin.*overspent$/);
      }
    });

    await test.step("she walks four squares: 10 ft of her 30 left", async () => {
      const from = await expectAgreed(table, hero.tokenId, "Aria stands still");
      // Nothing typed into: movement keys go to the board.
      await aria.page.evaluate(() =>
        (document.activeElement as HTMLElement | null)?.blur(),
      );
      await walk(aria, "east", 4);
      await expect
        .poll(() => roundedServerPosition(hero.tokenId), {
          timeout: 10_000,
          message: "four presses east move Aria four squares",
        })
        .toEqual({ x: Math.round(from.x) + 4 * GRID, y: Math.round(from.y) });
      await everySeatReads(
        boards,
        "Aria",
        "Action −1/1 · Bonus 1/1 · Reaction 1/1 · Move 10/30 ft",
        "Aria's movement is counted against her speed",
      );
    });

    await test.step("on the goblin's turn: Aria's reaction is spent, and a Game Master's drag spends the goblin's movement", async () => {
      await advanceTurn(table, combat.id);
      await everySeatReads(
        boards,
        "Aria",
        "Action −1/1 · Bonus 1/1 · Reaction 1/1 · Move 10/30 ft",
        "Aria's turn ending gives her nothing back",
      );
      await makeAttackAs(aria.page, {
        attackerTokenId: hero.tokenId,
        abilityId: longsword,
        targetTokenId: goblin.tokenId,
        actionCost: "REACTION",
      });
      await answerOffer(table.gm, "Goblin", false);
      await everySeatReads(
        boards,
        "Aria",
        "Action −1/1 · Bonus 1/1 · Reaction 0/1 · Move 10/30 ft",
        "Aria's reaction, taken on the goblin's turn, is spent",
      );

      await openCombatPanel(table.gm);
      await dragToken(table.gm, goblin.tokenId, { dx: GRID, dy: 0 });
      await expect
        .poll(() => roundedServerPosition(goblin.tokenId), {
          timeout: 10_000,
          message: "the goblin is dragged one square east",
        })
        .toEqual(cell(1, 0));
      await everySeatReads(
        boards,
        "Goblin",
        "Action 1/1 · Bonus 1/1 · Reaction 1/1 · Move 25/30 ft",
        "the Game Master moved the goblin, and it is the goblin's movement",
      );
    });

    await test.step("through the ogre's turn her reaction stays spent; at her own, all is fresh", async () => {
      await advanceTurn(table, combat.id);
      await everySeatReads(
        boards,
        "Aria",
        "Action −1/1 · Bonus 1/1 · Reaction 0/1 · Move 10/30 ft",
        "a reaction spent between turns waits for its owner's turn",
      );
      await advanceTurn(table, combat.id);
      await everySeatReads(
        boards,
        "Aria",
        FRESH,
        "the turn comes back to Aria, and her budget is fresh",
      );
      await everySeatReads(
        boards,
        "Goblin",
        "Action 1/1 · Bonus 1/1 · Reaction 1/1 · Move 25/30 ft",
        "and nobody else's budget changed",
      );
    });
  } finally {
    await closeTable(table);
  }
});
