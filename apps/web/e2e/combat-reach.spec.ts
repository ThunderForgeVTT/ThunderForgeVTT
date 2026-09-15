import { expect, test, type Page } from "./fixtures/test";
import { canvasBox, dragToken, serverTokenPosition } from "./fixtures/offline";
import {
  addCombatant,
  advanceTurn,
  answerOffer,
  attackLogOn,
  attackWarningsFromSheet,
  claimFor,
  footprintOn,
  grantAbility,
  setAbilityReach,
  setAbilityScores,
  setDisclosure,
  setHitPoints,
  startCombat,
  tokenHitPointsOf,
  type FootprintDrawn,
} from "../playtest/combat";
import {
  addWall,
  closeTable,
  expectAgreed,
  makeDoor,
  must,
  openTable,
  placeCast,
  sitDown,
  walk,
  type Point,
} from "../playtest/table";

/**
 * Spec 046 tasks Phase 7 (plan phase 5, quickstart "size, reach, range, line
 * of sight"; US4, SC-004).
 *
 * A Large ogre fills two squares by two on every board, and the grid treats it
 * so: it is drawn, named and barred as two squares, a press on any of its
 * squares picks it up, a drag snaps it to the vertex its four squares meet
 * at, and a Large hero walks by keyboard one square at a time in every
 * direction. Then reach, from footprint to footprint: a hero beside the ogre
 * swings unflagged; four squares away she is warned, still swings, and the
 * table is told "out of reach"; a bow past its normal range is a long shot;
 * and through a closed door the swing is flagged with no line of sight and is
 * not auto-applied though auto-apply is on.
 *
 * The scene is given a 64-unit grid anchored to a 1280-square map, so a vertex
 * sits on the world origin and every position below is a whole number of
 * squares from it — the arithmetic is the test's, not the dice's. The weapons
 * hit on any die for fixed damage, as in `combat-attack.spec.ts`.
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
const OGRE_HP = 59;
const LARGE = { class: "monster", level: 5, size: "large" };

/** The centre of the one-square cell `(q, r)`, counted from the origin vertex. */
function cell(q: number, r: number): Point {
  return { x: (q + 0.5) * GRID, y: (r + 0.5) * GRID };
}

/** The centre of a 2×2 whose lower-left square is `(q, r)`: a vertex. */
function block(q: number, r: number): Point {
  return { x: (q + 1) * GRID, y: (r + 1) * GRID };
}

async function moveTo(page: Page, tokenId: string, at: Point): Promise<void> {
  await must(
    page,
    `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
      updateToken(tokenId: $tokenId, input: $input) { tokenId }
    }`,
    { tokenId, input: { x: at.x, y: at.y } },
  );
}

async function drawn(page: Page, tokenId: string): Promise<FootprintDrawn> {
  let found: FootprintDrawn | null = null;
  await expect
    .poll(
      async () => {
        found = await footprintOn(page, tokenId);
        return found?.footprint ?? null;
      },
      { timeout: 20_000, message: `the engine draws ${tokenId}` },
    )
    .not.toBeNull();
  return found!;
}

test("a Large ogre fills four squares, and reach is measured from them", async ({
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
    sceneName: "The Ogre's Door",
  });
  const [aria, brom] = table.players;

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

    // The ogre fills squares (0,0)..(1,1). Aria stands diagonally beside its
    // lower-left square. Brom has been made Large, and walks.
    const ogre = await placeCast(table, {
      label: "Ogre",
      at: block(0, 0),
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 11 },
        hitPoints: { current: OGRE_HP, max: OGRE_HP },
        traits: LARGE,
      },
    });
    const hero = await placeCast(table, {
      label: "Aria",
      at: cell(-1, -1),
      seat: aria,
    });
    const giant = await placeCast(table, {
      label: "Brom",
      at: block(-4, 1),
      seat: brom,
      sheet: {
        scores: SCORES,
        hitPoints: { current: 20, max: 20 },
        traits: { class: "fighter", level: 3, size: "large" },
      },
    });
    await setAbilityScores(table, hero.actorId, { ...SCORES, armor_class: 14 });
    await setHitPoints(table, hero.actorId, { current: 16, max: 16 });
    await setDisclosure(table, ogre.tokenId, "hitPoints", "VISIBLE");
    await setDisclosure(table, hero.tokenId, "hitPoints", "VISIBLE");
    await claimFor(table, aria, hero.actorId);

    const longsword = await grantAbility(table, hero.actorId, {
      name: "Longsword",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "5" },
      ],
    });
    await setAbilityReach(table, longsword, { reach: 5 });
    // A bow scaled to the board: ten feet normal, twenty long.
    const shortbow = await grantAbility(table, hero.actorId, {
      name: "Shortbow",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "3" },
      ],
    });
    await setAbilityReach(table, shortbow, { rangeNormal: 10, rangeLong: 20 });

    for (const client of [table.gm, aria.page, brom.page]) {
      await sitDown(table, client);
    }

    await test.step("the ogre is two squares by two on every board, drawn, named and barred", async () => {
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(
            async () => (await footprintOn(client, ogre.tokenId))?.footprint,
            {
              timeout: 20_000,
              message: `${who}'s engine is told the ogre fills two squares a side`,
            },
          )
          .toBe(2);
        await expect
          .poll(
            async () => {
              const o = await footprintOn(client, ogre.tokenId);
              return o
                ? [Math.round(Math.max(o.width, o.height)), o.x, o.y]
                : null;
            },
            {
              timeout: 10_000,
              message: `${who} draws the ogre over its four squares, centred on their vertex`,
            },
          )
          .toEqual([2 * GRID, block(0, 0).x, block(0, 0).y]);
        const a = await drawn(client, hero.tokenId);
        expect(a.footprint, `${who} still draws Aria as one square`).toBe(1);
        expect(Math.round(Math.max(a.width, a.height))).toBe(GRID);
      }

      // The name and the bars follow the footprint: on the Game Master's
      // board, which draws both for both, the ogre's bars span two squares
      // and its name sits half a square higher than Aria's.
      await expect
        .poll(
          async () => {
            const o = await footprintOn(table.gm, ogre.tokenId);
            const a = await footprintOn(table.gm, hero.tokenId);
            if (!o || !a || o.nameY === null || a.nameY === null) return null;
            return {
              ogreBars: o.barWidth === null ? null : Math.round(o.barWidth),
              ariaBars: a.barWidth === null ? null : Math.round(a.barWidth),
              // Half a square for the taller token, and a little more for the
              // larger type a larger token's name is set in
              // (`nameplate::font_px`).
              nameHigherByHalfASquare:
                o.nameY - a.nameY >= GRID / 2 &&
                o.nameY - a.nameY < GRID / 2 + 12,
            };
          },
          {
            timeout: 20_000,
            message: "the ogre's name and bars are placed for two squares",
          },
        )
        .toEqual({
          ogreBars: 2 * GRID,
          ariaBars: GRID,
          nameHigherByHalfASquare: true,
        });
    });

    await test.step("a press on the ogre's far square picks it up, and a drag snaps it to a vertex", async () => {
      const box = await canvasBox(table.gm);
      const cx = box.x + box.width / 2;
      const cy = box.y + box.height / 2;
      // Nothing selected, then a press three quarters of a square up and
      // right of the ogre's centre: inside its upper-right square, and outside
      // where a one-square token there would be.
      await table.gm.keyboard.press("Escape");
      const far = {
        x: block(0, 0).x + 0.75 * GRID,
        y: block(0, 0).y + 0.75 * GRID,
      };
      await table.gm.mouse.click(cx + far.x, cy - far.y);
      await expect
        .poll(
          () =>
            table.gm.evaluate(
              () => window.__worldProbe?.state().selectedTokenId ?? null,
            ),
          {
            timeout: 5_000,
            message: "the press on the ogre's upper-right square selected it",
          },
        )
        .toBe(ogre.tokenId);

      // A drag of a square and a quarter east and a third of a square north
      // lands one square east: on a vertex, where a 2×2 belongs, not on a
      // square's centre (160, 96) where a one-square token would.
      await dragToken(table.gm, ogre.tokenId, { dx: 80, dy: -20 });
      const landed = await expectAgreed(
        table,
        ogre.tokenId,
        "every board and the server agree where the dragged ogre is",
      );
      expect(landed).toEqual(block(1, 0));
      await moveTo(table.gm, ogre.tokenId, block(0, 0));
      await expectAgreed(table, ogre.tokenId, "the ogre is put back");
    });

    await test.step("a Large hero walks one square at a time in every direction", async () => {
      await expect
        .poll(
          async () => (await footprintOn(brom.page, giant.tokenId))?.footprint,
          {
            timeout: 20_000,
          },
        )
        .toBe(2);
      let at = await expectAgreed(
        table,
        giant.tokenId,
        "Brom stands where he was put",
      );
      expect(at).toEqual(block(-4, 1));
      for (const [step, dx, dy] of [
        ["west", -GRID, 0],
        ["south", 0, -GRID],
        ["east", GRID, 0],
        ["north", 0, GRID],
      ] as const) {
        await walk(brom, step);
        await expect
          .poll(
            () => serverTokenPosition(table.gm, table.sceneId, giant.tokenId),
            {
              timeout: 10_000,
              message: `a ${step} key moves Large Brom one square ${step}`,
            },
          )
          .toEqual({ x: at.x + dx, y: at.y + dy });
        at = await expectAgreed(table, giant.tokenId, `Brom after ${step}`);
      }
    });

    const combat = await startCombat(table);
    await addCombatant(table, combat.id, {
      label: "Aria",
      actorId: hero.actorId,
      tokenId: hero.tokenId,
      initiative: 20,
    });
    await addCombatant(table, combat.id, {
      label: "Ogre",
      actorId: ogre.actorId,
      tokenId: ogre.tokenId,
      initiative: 5,
      isNpc: true,
    });
    await advanceTurn(table, combat.id);
    // Each swing after the first is made on a new turn of Aria's, so the
    // flags a step reads are the ones it is about: a second action in one
    // turn is flagged overspent (spec 046 Phase 8, `combat-economy.spec.ts`).
    const aNewTurnForAria = async () => {
      await advanceTurn(table, combat.id); // the ogre's
      await advanceTurn(table, combat.id); // Aria's again
    };

    await test.step("beside the ogre's corner square, a longsword swings unflagged", async () => {
      const { warnings, said } = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Longsword",
        "Ogre",
      );
      expect(warnings, "nothing to warn about").toEqual([]);
      expect(said).toMatch(
        /^Aria → Ogre · Longsword · \d+ vs 11: hit · 5 damage offered$/,
      );
      await answerOffer(table.gm, "Ogre", false);
    });

    await test.step("four squares away she is warned, still swings, and the table is told", async () => {
      // Four columns west of the ogre's nearest square.
      await moveTo(table.gm, hero.tokenId, cell(-4, 0));
      await aNewTurnForAria();
      const { warnings, said } = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Longsword",
        "Ogre",
      );
      expect(warnings, "warned before rolling (FR-033)").toEqual([
        "Out of reach: 20 ft, reach 5 ft",
      ]);
      expect(said, "not refused, and flagged").toMatch(
        /^Aria → Ogre · Longsword · \d+ vs 11: hit · 5 damage offered · out of reach$/,
      );
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(async () => (await attackLogOn(client))[0] ?? "", {
            timeout: 5_000,
            message: `${who} is shown the swing flagged out of reach`,
          })
          .toMatch(/^Aria → Ogre.*out of reach$/);
      }
      await answerOffer(table.gm, "Ogre", false);
    });

    await test.step("a bow past its normal range is a long shot", async () => {
      await aNewTurnForAria();
      const { warnings, said } = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Shortbow",
        "Ogre",
      );
      expect(warnings).toEqual(["Long range: 20 ft, normal range 10 ft"]);
      expect(said).toMatch(/Shortbow.*long range$/);
      await expect
        .poll(async () => (await attackLogOn(brom.page))[0] ?? "", {
          timeout: 5_000,
        })
        .toMatch(/Shortbow.*long range$/);
      await answerOffer(table.gm, "Ogre", false);

      await moveTo(table.gm, hero.tokenId, cell(-6, 0));
      await aNewTurnForAria();
      const beyond = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Shortbow",
        "Ogre",
      );
      expect(beyond.warnings).toEqual(["Beyond range: 30 ft, range 20 ft"]);
      expect(beyond.said).toMatch(/beyond range$/);
      await answerOffer(table.gm, "Ogre", false);
    });

    await test.step("through a closed door: no line of sight, and not auto-applied", async () => {
      await must(
        table.gm,
        `mutation ($combatId: UUID!, $enabled: Boolean) {
          setCombatAutoApply(combatId: $combatId, enabled: $enabled) { id }
        }`,
        { combatId: combat.id, enabled: true },
      );
      // Beside the ogre's lower-left square, with a door on the line between.
      await moveTo(table.gm, hero.tokenId, cell(-1, 0));
      const door = await addWall(
        table,
        { x: 0, y: -3 * GRID },
        { x: 0, y: 4 * GRID },
      );
      await makeDoor(table, door);
      await aNewTurnForAria();

      const { warnings } = await attackWarningsFromSheet(
        aria.page,
        hero.actorId,
        "Longsword",
        "Ogre",
      );
      expect(warnings).toEqual(["No line of sight: a wall is in the way"]);
      await expect
        .poll(async () => (await attackLogOn(table.gm))[0] ?? "", {
          timeout: 5_000,
          message:
            "the Game Master is shown the swing, offered and flagged, not applied",
        })
        .toMatch(
          // The log sets each part in its own element, with no separator text.
          /^Aria → Ogre.*Longsword.*vs 11: hit.*5 damage offered.*no line of sight$/,
        );
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, ogre.tokenId),
        "auto-apply is on, and the ogre is untouched: the hit waits for a person",
      ).toBe(OGRE_HP);
      // Aria's board does not draw the ogre behind the door, so her log does
      // not name it (research R11: redaction follows the board).
      await expect
        .poll(async () => (await attackLogOn(aria.page))[0] ?? "", {
          timeout: 5_000,
        })
        .toMatch(/^Aria → Unknown.*no line of sight$/);
    });
  } finally {
    await closeTable(table);
  }
});
