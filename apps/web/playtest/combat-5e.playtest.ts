import { expect, test } from "@playwright/test";
import {
  abilityRollsOn,
  activeRowOn,
  addCombatant,
  advanceTurn,
  answerOffer,
  attackFromSheet,
  attackLogOn,
  attackWarningsFromSheet,
  footprintOn,
  setAbilityReach,
  barCurrentOn,
  changeHitPoints,
  claimFor,
  offersOn,
  grantAbility,
  combatSeenBy,
  endCombat,
  openCombatPanel,
  refusalOfAdvanceTurn,
  rollInPanel,
  rollShown,
  rosterOn,
  roundOn,
  setAbilityScores,
  setTraits,
  darkvisionOn,
  gridSizeOf,
  setDisclosure,
  setHitPoints,
  startCombat,
  statusOn,
  systemDataOf,
  tokenHitPointsOf,
  type Combat,
} from "./combat";
import {
  closeTable,
  openTable,
  placeCast,
  placeToken,
  sitDown,
  snapshot,
  tryDrag,
} from "./table";

/**
 * A D&D 5e fight, played: two heroes and a goblin, initiative, turns, an
 * attack, damage, a creature at zero, and the Game Master calling it.
 *
 * Initiative and turn order are real, and are checked hard. Since spec 046 so
 * are damage (the Game Master's hit-point change reaches every board), a
 * creature at zero (out of the fight, and skipped), whose turn it is (a
 * player is refused a move on somebody else's), and an attack: Aria swings
 * her longsword from her own sheet at the goblin, the server rolls it against
 * the goblin's armour class, every seat is shown it, and a hit is offered to
 * the Game Master to take; and size and reach (the ogre fills two squares by
 * two on every board, and a longsword swung across the room is warned and
 * flagged). What is not there yet — the economy of a round — is recorded as a
 * FINDING (soft, so the session plays to
 * the end) rather than faked with a test-only path: a scenario that writes the
 * outcome itself is a scenario proving nothing.
 */

/** The scores a 5e actor needs before the pack will accept anything else. */
const HERO_SCORES = {
  strength: 16,
  dexterity: 14,
  constitution: 14,
  intelligence: 10,
  wisdom: 12,
  charisma: 8,
};
const CASTER_SCORES = {
  ...HERO_SCORES,
  strength: 8,
  dexterity: 14,
  intelligence: 16,
};
// Spec 046: armour class is the 5e pack's declared defence, kept with the
// scores. A goblin wears leather and a shield (15); an ogre, hide (11); Aria,
// chain mail (16); Brom, nothing (12).
const GOBLIN_SCORES = {
  ...HERO_SCORES,
  strength: 8,
  dexterity: 14,
  armor_class: 15,
};
const OGRE_SCORES = {
  ...HERO_SCORES,
  strength: 19,
  dexterity: 8,
  armor_class: 11,
};
const GOBLIN_AC = GOBLIN_SCORES.armor_class;

// The creatures' own numbers, so the fight is a fight somebody could run: a
// goblin is Small with 7 hit points, an ogre is Large with 59.
const GOBLIN_HP = 7;
const OGRE_HP = 59;
/**
 * Large: ten feet of space, which is two squares of a five-foot grid. A size
 * on the ogre's sheet, as 5e declares it (spec 046 FR-030), not a token scale
 * that only draws it bigger. 5e keeps size among the traits, beside a class
 * and level its pack requires of every sheet.
 */
const OGRE_TRAITS = { class: "monster", level: 5, size: "large" };
const OGRE_FOOTPRINT = 2;

test("a Game Master runs a 5e fight for two players", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(900_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria", "Brom"],
    sceneName: "The Guardroom",
  });
  const [aria, brom] = table.players;

  try {
    const cast: Record<string, { actorId: string; tokenId: string }> = {};
    let combat: Combat | null = null;
    const rolled: Record<string, number> = {};
    // A second goblin placed from the same NPC. Not in `cast`, which is the
    // roster: it is on the board, not in the fight.
    let secondGoblinTokenId = "";
    let longsword = "";

    await test.step("the table takes its places", async () => {
      cast.Aria = await placeCast(table, {
        label: "Aria",
        at: { x: -200, y: 0 },
        seat: aria,
      });
      cast.Brom = await placeCast(table, {
        label: "Brom",
        at: { x: -200, y: -150 },
        seat: brom,
      });
      // The creatures are written before they are placed: an NPC's token is a
      // copy of its sheet as it stands when it goes down (spec 046 ADR-102).
      cast.Goblin = await placeCast(table, {
        label: "Goblin",
        at: { x: 200, y: 0 },
        tokenType: "npc",
        sheet: {
          scores: GOBLIN_SCORES,
          hitPoints: { current: GOBLIN_HP, max: GOBLIN_HP },
        },
      });
      // A second goblin of the same NPC: its own copy, with its own hit
      // points, and no second actor.
      secondGoblinTokenId = (
        await placeToken(table, cast.Goblin.actorId, {
          at: { x: 300, y: -100 },
          label: "Goblin 2",
        })
      ).tokenId;
      // An ogre: Large, so the board has a large piece on it as well as a
      // small one.
      cast.Ogre = await placeCast(table, {
        label: "Ogre",
        at: { x: 500, y: 150 },
        tokenType: "npc",
        sheet: {
          scores: OGRE_SCORES,
          hitPoints: { current: OGRE_HP, max: OGRE_HP },
          traits: OGRE_TRAITS,
        },
      });

      await setAbilityScores(table, cast.Aria.actorId, {
        ...HERO_SCORES,
        armor_class: 16,
      });
      await setAbilityScores(table, cast.Brom.actorId, {
        ...CASTER_SCORES,
        armor_class: 12,
      });
      // Each player claims the character they play: a claimed character's
      // sheet is the one that opens inside their dock, where their attacks
      // are (spec 031 US2).
      await claimFor(table, aria, cast.Aria.actorId);
      await claimFor(table, brom, cast.Brom.actorId);
      await setHitPoints(table, cast.Aria.actorId, { current: 16, max: 16 });
      await setHitPoints(table, cast.Brom.actorId, { current: 11, max: 11 });

      // Two styles, each a real ability with its own rolls: Aria swings,
      // Brom throws fire.
      longsword = await grantAbility(table, cast.Aria.actorId, {
        name: "Longsword",
        classification: "feat",
        description: "A blade, swung at whatever is in front of it.",
        effects: [
          { effectType: "ATTACK_ROLL", formula: "1d20+5" },
          { effectType: "DAMAGE", formula: "1d8+3" },
        ],
      });
      await grantAbility(table, cast.Brom.actorId, {
        name: "Fire Bolt",
        classification: "spell",
        grade: 0,
        description: "A mote of fire, thrown.",
        effects: [
          { effectType: "ATTACK_ROLL", formula: "1d20+5" },
          { effectType: "DAMAGE", formula: "1d10" },
        ],
      });

      for (const client of [table.gm, aria.page, brom.page]) {
        await sitDown(table, client);
      }
      // Hit points reach the board: the bars are built from the actor's
      // resource data when the scene loads.
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
      ] as const) {
        await expect
          .poll(() => statusOn(client, cast.Aria.tokenId), {
            timeout: 30_000,
            message: `${who}'s board draws Aria's hit points`,
          })
          .toContain("hitPoints");
      }
      // The large piece is large on every board, not just where it was made:
      // the engine is told the ogre fills two squares a side, and draws it
      // over two squares of this scene's grid.
      const grid = await gridSizeOf(table);
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(
            async () =>
              (await footprintOn(client, cast.Ogre.tokenId))?.footprint,
            {
              timeout: 20_000,
              message: `${who}'s board has the ogre filling two squares by two`,
            },
          )
          .toBe(OGRE_FOOTPRINT);
        await expect
          .poll(
            async () => {
              const drawn = await footprintOn(client, cast.Ogre.tokenId);
              return drawn ? Math.max(drawn.width, drawn.height) : 0;
            },
            { timeout: 10_000, message: `${who} draws the ogre that big` },
          )
          .toBeCloseTo(OGRE_FOOTPRINT * grid, 0);
      }
      await snapshot(table, "1 · the guardroom");
    });

    await test.step("a dwarf's darkvision reaches further than a human's", async () => {
      // Spec 045 US6 and owner decision 2: the *game system* says how its
      // creatures see. D&D 5e's manifest declares where darkvision lives on a
      // character, the server resolves it from that character's own sheet,
      // and the engine is handed the answer in world units.
      //
      // Asked of the engine rather than of the server, because the server
      // answering correctly proves only that it can read its own manifest.
      // The whole point of this step is the chain: manifest, sheet, server,
      // web, engine. That chain had never been run end to end.
      await setTraits(table, cast.Aria.actorId, {
        class: "fighter",
        level: 3,
        darkvision: 60,
      });
      await setTraits(table, cast.Brom.actorId, { class: "wizard", level: 3 });

      // A 5-foot square at this scene's grid: sixty feet is twelve squares.
      const expected = 12 * (await gridSizeOf(table));
      await expect
        .poll(() => darkvisionOn(table.gm, cast.Aria.tokenId), {
          timeout: 20_000,
          message:
            "Aria's sixty feet of darkvision should reach the engine as " +
            `${expected} world units`,
        })
        .toBeCloseTo(expected, 0);

      expect(
        await darkvisionOn(table.gm, cast.Brom.tokenId),
        "and a character whose sheet says nothing about darkvision sees by " +
          "the default rules, rather than inheriting anybody else's sight",
      ).toBe(0);

      // FR-067: a sheet edit reaches every board. Nothing announced one at
      // all before spec 045 gave sheet changes a world event of their own.
      await setTraits(table, cast.Brom.actorId, {
        class: "wizard",
        level: 3,
        darkvision: 30,
      });
      await expect
        .poll(() => darkvisionOn(table.gm, cast.Brom.tokenId), {
          timeout: 20_000,
          message:
            "granting Brom darkvision on his sheet reaches the board without " +
            "a reload (FR-067)",
        })
        .toBeCloseTo(6 * (await gridSizeOf(table)), 0);
    });

    await test.step("everyone rolls for initiative", async () => {
      rolled.Aria = await rollInPanel(aria.page, "1d20");
      rolled.Brom = await rollInPanel(brom.page, "1d20");
      rolled.Goblin = await rollInPanel(table.gm, "1d20");
      rolled.Ogre = await rollInPanel(table.gm, "1d20");
      for (const [who, value] of Object.entries(rolled)) {
        expect(value, `${who}'s d20 is a d20`).toBeGreaterThanOrEqual(1);
        expect(value, `${who}'s d20 is a d20`).toBeLessThanOrEqual(20);
      }

      // Was FINDING 263 ("Aria's roll should reach the table"). A free roll
      // in the dice roller stays the roller's own; what the table is shown
      // is an *attack* (spec 046 FR-002), which is checked hard in "Aria
      // attacks the goblin" below.
      testInfo.annotations.push({
        type: "free roll",
        description: `Aria's initiative d20 on Brom's screen: ${
          (await rollShown(brom.page)) ?? "nothing"
        }`,
      });
      await snapshot(table, "2 · initiative");
    });

    await test.step("the Game Master files the fight", async () => {
      combat = await startCombat(table);
      // Tiebreaks so the order is the numbers' to decide, not the ids'.
      combat = await addCombatant(table, combat.id, {
        label: "Aria",
        actorId: cast.Aria.actorId,
        tokenId: cast.Aria.tokenId,
        initiative: rolled.Aria,
        tiebreak: 3,
      });
      combat = await addCombatant(table, combat.id, {
        label: "Goblin",
        actorId: cast.Goblin.actorId,
        tokenId: cast.Goblin.tokenId,
        initiative: rolled.Goblin,
        tiebreak: 2,
        isNpc: true,
      });
      combat = await addCombatant(table, combat.id, {
        label: "Brom",
        actorId: cast.Brom.actorId,
        tokenId: cast.Brom.tokenId,
        initiative: rolled.Brom,
        tiebreak: 1,
      });
      combat = await addCombatant(table, combat.id, {
        label: "Ogre",
        actorId: cast.Ogre.actorId,
        tokenId: cast.Ogre.tokenId,
        initiative: rolled.Ogre,
        tiebreak: 0,
        isNpc: true,
      });

      const expected = [...combat.combatants]
        .sort((a, b) => b.initiative - a.initiative || b.tiebreak - a.tiebreak)
        .map((c) => c.label);
      expect(
        combat.combatants.map((c) => c.label),
        "the server puts the roster in initiative order",
      ).toEqual(expected);

      // Both players' panels have it, live, without a reload.
      for (const seat of [aria, brom]) {
        await openCombatPanel(seat.page);
        await expect
          .poll(() => rosterOn(seat.page), {
            timeout: 20_000,
            message: `${seat.name}'s panel shows the roster`,
          })
          .toHaveLength(Object.keys(cast).length);
      }
      await openCombatPanel(table.gm);
      await snapshot(table, "3 · the roster");
    });

    await test.step("turns pass, and every board follows", async () => {
      const order = combat!.combatants.map((c) => c.label);
      for (const [index, label] of order.entries()) {
        combat = await advanceTurn(table, combat!.id);
        for (const [who, client] of [
          ["the Game Master", table.gm],
          ["Aria", aria.page],
          ["Brom", brom.page],
        ] as const) {
          await expect
            .poll(() => activeRowOn(client), {
              timeout: 20_000,
              message: `turn ${index + 1}: ${who}'s board marks ${label}`,
            })
            .toContain(label);
        }
      }
      // Round 1 to everyone, until the order wraps.
      for (const client of [table.gm, aria.page, brom.page]) {
        expect(await roundOn(client)).toContain("1");
      }
      combat = await advanceTurn(table, combat!.id);
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(() => roundOn(client), {
            timeout: 20_000,
            message: `${who}'s board opens the second round`,
          })
          .toContain("2");
      }
      await snapshot(table, "4 · round two");
    });

    await test.step("a turn is a pointer, not an economy", async () => {
      // What a round actually is at a 5e table: on your turn you move, take
      // an action and perhaps a bonus action; between turns you may take a
      // reaction. A monster may have multiattack, and a big one — a
      // tarrasque — spends legendary actions at the end of *other* creatures'
      // turns, with lair actions on initiative count 20.
      //
      // The tracker points at whose turn it is and counts rounds. Nothing
      // here records what anyone has spent.
      const panel =
        (await table.gm.getByTestId("combat-panel").textContent()) ?? "";
      const knowsTheEconomy = /action|bonus|reaction|legendary/i.test(panel);
      expect
        .soft(
          knowsTheEconomy,
          "FINDING: a round should be an economy, not a pointer. A turn " +
            "affords an action, a bonus action and movement; a reaction " +
            "happens between turns; a legendary creature spends legendary " +
            "actions at the end of other creatures' turns, and lair actions " +
            "on initiative 20. The tracker offers none of it, so everything " +
            "a table spends is remembered out loud.",
        )
        .toBe(true);
      await snapshot(table, "4b · the economy that isn't");
    });

    await test.step("a player cannot take the table's turn", async () => {
      expect(
        await brom.page.getByTestId("advance-turn-button").count(),
        "a player is shown no way to advance the turn",
      ).toBe(0);
      const refusal = await refusalOfAdvanceTurn(brom.page, combat!.id);
      expect(
        refusal.join(" "),
        "and the server refuses one that asks anyway",
      ).toMatch(/only the gm/i);
    });

    await test.step("Aria attacks the goblin", async () => {
      // Her turn first: an attack on somebody else's turn is refused (C1).
      for (
        let turn = 0;
        turn < 5 &&
        combat!.combatants.find((c) => c.id === combat!.activeCombatantId)
          ?.label !== "Aria";
        turn += 1
      ) {
        combat = await advanceTurn(table, combat!.id);
      }

      // Was FINDING 416 ("nothing can say whether it hit"). The longsword,
      // swung from Aria's own sheet at the goblin: the server rolls it
      // against the goblin's armour class and says hit or miss.
      const said = await attackFromSheet(
        aria.page,
        cast.Aria.actorId,
        "Longsword",
        "Goblin",
      );
      expect(
        said,
        "the attack is rolled against the goblin's armour class and judged",
      ).toMatch(new RegExp(`Aria → Goblin.*vs ${GOBLIN_AC}: (hit|miss)`));
      const hit = /: hit/.test(said);

      // Was FINDING 263 ("Aria's roll should reach the table"): every other
      // seat is shown it, without asking.
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(async () => (await attackLogOn(client)).join(" | "), {
            timeout: 5_000,
            message: `${who} is shown Aria's attack on the goblin`,
          })
          .toMatch(
            new RegExp(`Aria → Goblin.*Longsword.*vs ${GOBLIN_AC}: (hit|miss)`),
          );
      }

      // A miss offers nothing. Aria swings until she lands one, as a player
      // would (nothing yet spends her action; Phase 8's economy flags it),
      // so every run also plays the hit.
      const swings = [said];
      let landed = hit;
      if (!landed) {
        expect(await offersOn(table.gm), "a miss offers nothing").toEqual([]);
      }
      while (!landed && swings.length < 8) {
        const again = await attackFromSheet(
          aria.page,
          cast.Aria.actorId,
          "Longsword",
          "Goblin",
        );
        swings.push(again);
        landed = /: hit/.test(again);
      }
      testInfo.annotations.push({
        type: "attack",
        description: swings.join(" / "),
      });

      // A hit's damage is offered to whoever controls the goblin — the Game
      // Master — who declines it here, so the goblin meets the Game Master's
      // own damage below whole.
      if (landed) {
        await expect
          .poll(() => offersOn(table.gm), {
            timeout: 10_000,
            message: "the Game Master is offered the goblin's damage",
          })
          .toContainEqual(expect.stringMatching(/^Goblin: take \d+ damage\?$/));
        await answerOffer(table.gm, "Goblin", false);
        await expect
          .poll(async () => (await attackLogOn(brom.page)).join(" | "), {
            timeout: 5_000,
            message: "the table is told the Game Master declined it",
          })
          .toMatch(/damage declined by/);
      }
      await snapshot(table, "5 · the attack");
    });

    await test.step("two styles, and the sheets that hold them", async () => {
      // Each player rolls their own style's numbers, through the roller every
      // seat has.
      const sword = await rollInPanel(aria.page, "1d8+3");
      expect(sword, "a longsword's damage").toBeGreaterThanOrEqual(4);
      expect(sword).toBeLessThanOrEqual(11);
      const firebolt = await rollInPanel(brom.page, "1d10");
      expect(firebolt, "a fire bolt's damage").toBeGreaterThanOrEqual(1);
      expect(firebolt).toBeLessThanOrEqual(10);

      // Was FINDING 439. Where a player finds their style: their own
      // character sheet, which opens in the dock for the character they
      // claimed, with the longsword's attack and damage on it.
      const onAriasSheet = await abilityRollsOn(aria.page, cast.Aria.actorId);
      expect(
        onAriasSheet,
        "Aria's longsword is on Aria's own sheet to roll",
      ).toBeGreaterThan(0);
      await snapshot(table, "6 · two styles");
    });

    await test.step("size, and the reach it ought to give", async () => {
      // Aria stands a long way from the ogre; a longsword does not.
      const ariaAt = await aria.page.evaluate(
        (id) =>
          window.__worldProbe?.state()?.tokens.find((t) => t.id === id) ?? null,
        cast.Aria.tokenId,
      );
      const ogreAt = await aria.page.evaluate(
        (id) =>
          window.__worldProbe?.state()?.tokens.find((t) => t.id === id) ?? null,
        cast.Ogre.tokenId,
      );
      const apart = Math.hypot(
        (ogreAt?.x ?? 0) - (ariaAt?.x ?? 0),
        (ogreAt?.y ?? 0) - (ariaAt?.y ?? 0),
      );
      expect(apart, "they are nowhere near each other").toBeGreaterThan(500);

      // Was FINDING 481 ("a creature should carry its size, and each of its
      // attacks its own reach or range"). Two separate things a table plays
      // by. Size is the space a creature fills: an ogre is Large, two squares
      // of a five-foot grid. Reach belongs to the *attack*, not the size — an
      // ogre is Large and its greatclub still reaches only five feet.
      const ogre = await systemDataOf(table.gm, cast.Ogre.actorId);
      expect(
        ogre.traitData?.size,
        "the ogre's own sheet carries its size",
      ).toBe("large");

      // The longsword says how far it reaches, and Aria swings it across the
      // room anyway: warned before rolling, not refused, and the table told.
      await setAbilityReach(table, longsword, { reach: 5 });
      const { warnings, said } = await attackWarningsFromSheet(
        aria.page,
        cast.Aria.actorId,
        "Longsword",
        "Ogre",
      );
      expect(
        warnings.join(" | "),
        "Aria is warned before rolling that the ogre is out of her reach",
      ).toMatch(/^Out of reach: \d+ ft, reach 5 ft/);
      expect(said, "and the swing is still made (FR-033)").toMatch(
        /Aria → Ogre.*(hit|miss).*out of reach/,
      );
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(async () => (await attackLogOn(client))[0] ?? "", {
            timeout: 5_000,
            message: `${who} is shown the swing flagged out of reach`,
          })
          .toMatch(/Aria → Ogre.*out of reach/);
      }
      // A hit across the room is still a hit the Game Master is offered; they
      // decline it, as a table would.
      if (/: hit/.test(said)) {
        await answerOffer(table.gm, "Ogre", false);
      }
      testInfo.annotations.push({
        type: "reach",
        description: `${warnings.join("; ")} — ${said}`,
      });
      await snapshot(table, "7 · reach");
    });

    await test.step("damage lands, and the board shows it", async () => {
      // Spec 046 FR-014: the Game Master's Damage, the same mutation the
      // tracker's button calls. It spends temporary hit points first and stops
      // at zero. The goblin is a copy of its NPC (spec 046 ADR-102), so the
      // hit is written to that token alone and announced as a token change
      // (event 14) — which every board re-reads its bars on.
      const before = await barCurrentOn(table.gm, cast.Goblin.tokenId);
      expect(before, "the Game Master's board draws the goblin whole").toBe(
        GOBLIN_HP,
      );
      const after = await changeHitPoints(
        table.gm,
        cast.Goblin.tokenId,
        "DAMAGE",
        GOBLIN_HP,
      );
      expect(after.current, "the goblin is at zero").toBe(0);
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, cast.Goblin.tokenId),
        "the server holds the goblin at zero",
      ).toBe(0);

      // Spec 046 SC-008 (tasks T046): two goblins of one NPC are two
      // creatures. The hit landed on one copy; the other copy, and the NPC's
      // own sheet, are exactly as they were.
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, secondGoblinTokenId),
        "the second goblin copy is untouched by a hit on the first",
      ).toBe(GOBLIN_HP);
      const npcSheet = await systemDataOf(table.gm, cast.Goblin.actorId);
      expect(
        npcSheet.resourceData?.current_hp,
        "the goblin NPC's own sheet is untouched by a hit on a copy",
      ).toBe(GOBLIN_HP);
      await expect
        .poll(() => barCurrentOn(table.gm, secondGoblinTokenId), {
          timeout: 5_000,
          message: "the second goblin's bar still reads whole",
        })
        .toBe(GOBLIN_HP);

      // Was FINDING 522 ("the goblin's bar should shorten on every board").
      // Its wording blamed a missing event; `updateActorSystemData` had
      // announced sheet changes since spec 045, and the bars ignored it.
      // Hard now, with no reload.
      await expect
        .poll(() => barCurrentOn(table.gm, cast.Goblin.tokenId), {
          timeout: 5_000,
          message:
            "the goblin's bar shortens on the Game Master's board without a " +
            "reload (spec 029 US1, spec 046 US2)",
        })
        .toBe(0);
      await snapshot(table, "6 · zero hit points");
    });

    await test.step("a goblin at zero is out of the fight", async () => {
      // Was FINDING 548. Spec 046 C8: reaching zero marks the combatant out,
      // by hit points, which healing would undo and a Game Master's Down
      // would not.
      await expect
        .poll(
          async () => {
            const seen = await combatSeenBy(brom.page, table.worldId);
            const row = seen?.combatants.find((c) => c.label === "Goblin");
            return row ? `${row.active}/${row.downedBy}` : "missing";
          },
          {
            timeout: 10_000,
            message: "a creature at zero hit points is out of the fight",
          },
        )
        .toBe("false/HIT_POINTS");
      await expect(
        table.gm
          .getByTestId("combatant-row")
          .filter({ hasText: "Goblin" })
          .getByTestId("combatant-out"),
        "and the Game Master's tracker says why",
      ).toHaveText("Out: 0 hit points", { timeout: 10_000 });

      // Nobody pressed Down, and the turn order obeys it anyway.
      combat = (await combatSeenBy(table.gm, table.worldId))!;
      for (let turn = 0; turn < 3; turn += 1) {
        combat = await advanceTurn(table, combat!.id);
        const active = combat!.combatants.find(
          (c) => c.id === combat!.activeCombatantId,
        );
        expect(
          active?.label,
          "a combatant out of the fight is skipped by the turn",
        ).not.toBe("Goblin");
      }
      await snapshot(table, "7 · out of the fight");
    });

    await test.step("the one status the table does see", async () => {
      await setDisclosure(table, cast.Goblin.tokenId, "hitPoints", "VISIBLE");
      for (const seat of [aria, brom]) {
        await expect
          .poll(() => statusOn(seat.page, cast.Goblin.tokenId), {
            timeout: 20_000,
            message: `${seat.name} is shown the goblin's own figures`,
          })
          .toContain("visible");
      }
      await snapshot(table, "8 · disclosed");
    });

    await test.step("a player is held to the turn", async () => {
      // Was FINDING 606. Spec 046 C1: while it is somebody else's turn, a
      // player's move is refused by the server, the token goes back, and the
      // refusal names whose turn it is.
      let active = combat!.combatants.find(
        (c) => c.id === combat!.activeCombatantId,
      );
      if (active?.label === "Aria" || active?.label === "Brom") {
        // Make it the ogre's turn, so both players are waiting.
        for (let turn = 0; turn < 4 && active?.label !== "Ogre"; turn += 1) {
          combat = await advanceTurn(table, combat!.id);
          active = combat!.combatants.find(
            (c) => c.id === combat!.activeCombatantId,
          );
        }
      }
      const waiting = aria;
      const token = cast.Aria;
      const from = await waiting.page.evaluate(
        (id) =>
          window.__worldProbe?.state()?.tokens.find((t) => t.id === id) ?? null,
        token.tokenId,
      );
      const moved = await tryDrag(waiting, token.tokenId, { x: 128, y: 0 });
      await expect(
        waiting.page.getByText(`It is ${active?.label}'s turn`).first(),
        "the player is told whose turn it is",
      ).toBeVisible({ timeout: 10_000 });
      await expect
        .poll(
          () =>
            waiting.page.evaluate(
              (id) =>
                window.__worldProbe?.state()?.tokens.find((t) => t.id === id)
                  ?.x ?? null,
              token.tokenId,
            ),
          {
            timeout: 10_000,
            message:
              `it is ${active?.label}'s turn, so ${waiting.name}'s token ` +
              "goes back where it was",
          },
        )
        .toBe(from?.x);
      testInfo.annotations.push({
        type: "turn order",
        description:
          `on ${active?.label}'s turn ${waiting.name}'s drag ` +
          `${moved ? "moved and was put back" : "was refused"}`,
      });
      await snapshot(table, "9 · out of turn");
    });

    await test.step("the Game Master calls it", async () => {
      // Read before it ends: a finished fight has no round to report.
      const lastRound = await roundOn(table.gm);
      await endCombat(table, combat!.id);
      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        // Back to the combat tab first: looking at a player's own sheet
        // earlier left their dock on Actors, and a panel nobody is looking at
        // says nothing at all.
        await openCombatPanel(client);
        await expect
          .poll(
            async () =>
              (await client.getByTestId("combat-panel").textContent()) ?? "",
            {
              timeout: 20_000,
              message: `${who}'s panel returns to rest`,
            },
          )
          .toContain("No combat in progress");
      }
      await snapshot(table, "10 · the fight ends");
      console.log(
        `[playtest] dnd5e combat: initiative ${JSON.stringify(rolled)}, ` +
          `reached ${lastRound}`,
      );
    });
  } finally {
    await closeTable(table);
  }
});
