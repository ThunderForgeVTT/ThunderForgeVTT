import { expect, test } from "@playwright/test";
import {
  abilityRollsOn,
  activeRowOn,
  addCombatant,
  advanceTurn,
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
  settle,
  startCombat,
  statusOn,
  systemDataOf,
  updateCombatant,
  type Combat,
} from "./combat";
import {
  closeTable,
  drag,
  openTable,
  placeCast,
  sitDown,
  snapshot,
} from "./table";

/**
 * A D&D 5e fight, played: two heroes and a goblin, initiative, turns, an
 * attack, damage, a creature at zero, and the Game Master calling it.
 *
 * Initiative and turn order are real, and are checked hard. The fight itself
 * is not: there is no armour class to beat, no target on a roll, no operation
 * that deals damage, and nothing that notices a creature at zero hit points.
 * Those are recorded as FINDINGs (soft, so the session plays to the end)
 * rather than faked with a test-only path — a scenario that writes the
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
const GOBLIN_SCORES = { ...HERO_SCORES, strength: 8, dexterity: 14 };
const OGRE_SCORES = { ...HERO_SCORES, strength: 19, dexterity: 8 };

// The creatures' own numbers, so the fight is a fight somebody could run: a
// goblin is Small with 7 hit points, an ogre is Large with 59.
const GOBLIN_HP = 7;
const OGRE_HP = 59;
/** Large: ten feet of space, which is two squares of a five-foot grid. */
const OGRE_SCALE = 2;

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
      cast.Goblin = await placeCast(table, {
        label: "Goblin",
        at: { x: 200, y: 0 },
        tokenType: "npc",
      });
      // An ogre: twice the size, so the board has a large piece on it as well
      // as a small one.
      cast.Ogre = await placeCast(table, {
        label: "Ogre",
        at: { x: 500, y: 150 },
        tokenType: "npc",
        scale: OGRE_SCALE,
      });

      await setAbilityScores(table, cast.Aria.actorId, HERO_SCORES);
      await setAbilityScores(table, cast.Brom.actorId, CASTER_SCORES);
      await setAbilityScores(table, cast.Goblin.actorId, GOBLIN_SCORES);
      await setAbilityScores(table, cast.Ogre.actorId, OGRE_SCORES);
      await setHitPoints(table, cast.Aria.actorId, { current: 16, max: 16 });
      await setHitPoints(table, cast.Brom.actorId, { current: 11, max: 11 });
      await setHitPoints(table, cast.Goblin.actorId, {
        current: GOBLIN_HP,
        max: GOBLIN_HP,
      });
      await setHitPoints(table, cast.Ogre.actorId, {
        current: OGRE_HP,
        max: OGRE_HP,
      });

      // Two styles, each a real ability with its own rolls: Aria swings,
      // Brom throws fire.
      await grantAbility(table, cast.Aria.actorId, {
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
      // The large piece is large on every board, not just where it was made.
      for (const [who, client] of [
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(
            () =>
              client.evaluate(
                (id) =>
                  window.__worldProbe?.state()?.tokens.find((t) => t.id === id)
                    ?.scale ?? null,
                cast.Ogre.tokenId,
              ),
            { timeout: 20_000, message: `${who} sees the ogre at its size` },
          )
          .toBe(OGRE_SCALE);
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

      // Aria rolled in front of the table. Nobody else saw a thing: a roll
      // is not announced to the world, and the history is the Game Master's
      // alone.
      const onBromsScreen = await rollShown(brom.page);
      expect
        .soft(
          onBromsScreen,
          "FINDING: Aria's roll should reach the table. Nothing announces a " +
            "roll — there is no world event for one, and `worldRollRecords` " +
            "is Game-Master-only — so a player's roll is invisible to every " +
            "other seat.",
        )
        .toContain(String(rolled.Aria));
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
      // A longsword, +3 to hit. The number is real; what it means is not.
      const attack = await rollInPanel(aria.page, "1d20+3");
      expect(attack).toBeGreaterThanOrEqual(4);
      expect(attack).toBeLessThanOrEqual(23);

      const goblin = await systemDataOf(table.gm, cast.Goblin.actorId);
      const describesDefence = JSON.stringify(goblin)
        .toLowerCase()
        .match(/armor|armour|"ac"/);
      expect
        .soft(
          describesDefence,
          `FINDING: Aria rolled ${attack} to hit, and nothing can say whether ` +
            "it hit. A 5e actor has no armour class the product reads, no " +
            "roll takes a target, and nothing compares a result to a defence.",
        )
        .toBeTruthy();
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

      // Where a player should find their style: their own character sheet.
      const onAriasSheet = await abilityRollsOn(aria.page, cast.Aria.actorId);
      expect
        .soft(
          onAriasSheet,
          "FINDING: Aria's longsword should be on Aria's own sheet to roll. " +
            "A Game Master authors and attaches an ability, and whether the " +
            "player can reach it from Play is a different question — one " +
            "worth answering before a table relies on it.",
        )
        .toBeGreaterThan(0);
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

      // Two separate things a table plays by, and the product has neither.
      //
      // Size is the space a creature fills: an ogre is Large, ten feet, two
      // squares of a five-foot grid; a tarrasque is Gargantuan, twenty feet,
      // four. Reach belongs to the *attack*, not the size — an ogre is Large
      // and its greatclub still reaches only five feet, while a tarrasque's
      // four attacks reach ten, fifteen, ten and twenty. Deriving reach from
      // size would get both of them wrong.
      const ogre = await systemDataOf(table.gm, cast.Ogre.actorId);
      const describesSize = JSON.stringify(ogre)
        .toLowerCase()
        .match(/size|reach|large/);
      expect
        .soft(
          describesSize,
          "FINDING: a creature should carry its size, and each of its attacks " +
            "its own reach or range. The ogre is Large — it should fill two " +
            "squares by two — and its greatclub reaches five feet, the same " +
            "as Aria's sword. Today its size is a token scale that only draws " +
            "it bigger, the actor holds no size, no attack holds a reach, and " +
            "no roll has a target to measure one against.",
        )
        .toBeTruthy();

      // Then the demonstration, unasserted: a longsword swung across the room.
      const swing = await rollInPanel(aria.page, "1d20+5");
      testInfo.annotations.push({
        type: "reach",
        description:
          `Aria swung a longsword at an ogre ${apart.toFixed(0)} units away ` +
          `and rolled ${swing}; nothing remarked on the distance.`,
      });
      await snapshot(table, "7 · reach");
    });

    await test.step("damage, written by hand", async () => {
      const before = await statusOn(table.gm, cast.Goblin.tokenId);
      await setHitPoints(table, cast.Goblin.actorId, {
        current: 0,
        max: GOBLIN_HP,
      });
      const stored = await systemDataOf(table.gm, cast.Goblin.actorId);
      expect(
        stored.resourceData?.current_hp,
        "the server holds the goblin at zero",
      ).toBe(0);

      // Nobody's bar moves: writing system data announces nothing.
      const after = await settle(
        () => statusOn(table.gm, cast.Goblin.tokenId),
        (value) => value !== before,
        8_000,
      );
      expect
        .soft(
          after,
          "FINDING: the goblin's bar should shorten on every board when its " +
            "hit points change — spec 029 US1's own test. `updateActorSystemData` " +
            "emits no world event, so no client re-reads the status until a " +
            "token event or a reload.",
        )
        .not.toBe(before);

      // It is the announcement that is missing, not the value: a reload
      // shows the new figure.
      await sitDown(table, table.gm);
      await openCombatPanel(table.gm);
      await expect
        .poll(() => statusOn(table.gm, cast.Goblin.tokenId), {
          timeout: 30_000,
          message: "after a reload the Game Master's board has the new figure",
        })
        .not.toBe(before);
      await snapshot(table, "6 · zero hit points");
    });

    await test.step("a goblin at zero is just a number", async () => {
      const seen = await combatSeenBy(table.gm, table.worldId);
      const goblinRow = seen!.combatants.find((c) => c.label === "Goblin")!;
      expect
        .soft(
          goblinRow.active,
          "FINDING: a creature at zero hit points should be out of the fight. " +
            "Nothing connects hit points to anything: 5e has no downed or " +
            "dying state, and the tracker's own 'Down' is a button the Game " +
            "Master presses by hand.",
        )
        .toBe(false);

      // So the Game Master presses it, and the turn order obeys that.
      combat = await updateCombatant(table, {
        combatantId: goblinRow.id,
        active: false,
      });
      for (let turn = 0; turn < 3; turn += 1) {
        combat = await advanceTurn(table, combat!.id);
        const active = combat!.combatants.find(
          (c) => c.id === combat!.activeCombatantId,
        );
        expect(
          active?.label,
          "a combatant marked down is skipped by the turn",
        ).not.toBe("Goblin");
      }
      await snapshot(table, "7 · marked down");
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

    await test.step("nothing holds a player to their turn", async () => {
      const active = combat!.combatants.find(
        (c) => c.id === combat!.activeCombatantId,
      );
      const waiting = active?.label === "Aria" ? brom : aria;
      const token = waiting === brom ? cast.Brom : cast.Aria;
      const from = await waiting.page.evaluate(
        (id) =>
          window.__worldProbe?.state()?.tokens.find((t) => t.id === id) ?? null,
        token.tokenId,
      );
      await drag(waiting, token.tokenId, { x: 128, y: 0 });
      const to = await waiting.page.evaluate(
        (id) =>
          window.__worldProbe?.state()?.tokens.find((t) => t.id === id) ?? null,
        token.tokenId,
      );
      expect
        .soft(
          to?.x === from?.x,
          `FINDING: it is ${active?.label}'s turn, and ${waiting.name} moved ` +
            "anyway. Nothing checks whose turn it is before a move, a roll or " +
            "an ability — the tracker is bookkeeping the product never reads.",
        )
        .toBe(true);
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
