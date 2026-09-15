import { expect, test, type Page } from "./fixtures/test";
import {
  addCombatant,
  advanceTurn,
  answerOffer,
  attackFromSheet,
  attackLogOn,
  barCurrentOn,
  changeHitPoints,
  claimFor,
  grantAbility,
  makeAttackAs,
  offersOn,
  openCombatPanel,
  setAbilityScores,
  setDisclosure,
  setHitPoints,
  startCombat,
  tokenHitPointsOf,
} from "../playtest/combat";
import {
  addWall,
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";
import { expectOnEverySeatWithinOneSecond } from "./fixtures/seatTiming";

/**
 * Spec 046 tasks Phase 6 (plan phase 4, quickstart "an attack aimed at
 * something"): an attack is shown to every seat, a hit is an offer, a Game
 * Master resolves an absent player's offer for them, auto-apply acts only on
 * what the Game Master runs, and a hidden attacker is "Unknown" with neither
 * its token id nor its name in any attack or offer the player's client
 * receives (FR-002a).
 *
 * The weapons hit on any die (`1d20+100`) and deal a fixed amount, so every
 * step's arithmetic is the test's to assert rather than the dice's to decide.
 * The rolls are still the server's: nothing here writes an outcome.
 */

const SCORES = {
  strength: 14,
  dexterity: 12,
  constitution: 12,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};
const ARIA_HP = 16;
const GOBLIN_HP = 7;
const GOBLIN_AC = 13;
const OGRE_HP = 59;
/** A name distinctive enough that finding it in traffic means it leaked. */
const OGRE_NAME = "Grukk Stonebelly";

/** Every GraphQL response body and subscription frame a page receives. */
interface Traffic {
  responses: string[];
  frames: string[];
  /**
   * Forget everything received so far, and every response to a request sent
   * before now.
   *
   * The second half is the point. A response is recorded once its body has
   * been read, which is after it arrived, which is after its request left —
   * so emptying the arrays alone let a read sent while the ogre could still
   * be seen land after the reset and be judged as if it had been sent after
   * the ogre was hidden. It carried the name it was entitled to carry then.
   */
  reset: () => void;
}

function record(page: Page): Traffic {
  let epoch = 0;
  const sentIn = new WeakMap<object, number>();
  const traffic: Traffic = {
    responses: [],
    frames: [],
    reset: () => {
      epoch += 1;
      traffic.responses.length = 0;
      traffic.frames.length = 0;
    },
  };
  page.on("request", (request) => {
    sentIn.set(request, epoch);
  });
  page.on("response", (response) => {
    if (!response.url().includes("graphql")) return;
    const sent = sentIn.get(response.request());
    void response
      .text()
      .then((body) => {
        if (sent === epoch) traffic.responses.push(body);
      })
      .catch(() => undefined);
  });
  page.on("websocket", (socket) => {
    socket.on("framereceived", (frame) => {
      traffic.frames.push(String(frame.payload));
    });
  });
  return traffic;
}

/** What a page received that is about an attack or an offer. */
function aboutAttacks(traffic: Traffic): string[] {
  return [
    ...traffic.responses.filter((body) =>
      /"(attack|sceneAttacks|pendingOffers|makeAttack|resolveOffer|previewAttack)"\s*:/.test(
        body,
      ),
    ),
    ...traffic.frames.filter((frame) => /attackId|offerId/.test(frame)),
  ];
}

test("an attack is aimed at something, and the table is told", async ({
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
    sceneName: "The Ogre's Bridge",
  });
  const [aria, brom] = table.players;
  const ariaTraffic = record(aria.page);
  const bromTraffic = record(brom.page);

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: -192, y: 0 },
      seat: aria,
    });
    await placeCast(table, {
      label: "Brom",
      at: { x: -192, y: -192 },
      seat: brom,
    });
    // NPCs are written before they are placed: a copy starts from its sheet
    // (ADR-102), and its armour class is read from that sheet (ADR-101).
    const goblin = await placeCast(table, {
      label: "Goblin",
      at: { x: 0, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: GOBLIN_AC },
        hitPoints: { current: GOBLIN_HP, max: GOBLIN_HP },
      },
    });
    const ogre = await placeCast(table, {
      label: OGRE_NAME,
      at: { x: 192, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 11 },
        hitPoints: { current: OGRE_HP, max: OGRE_HP },
      },
    });
    await setAbilityScores(table, hero.actorId, { ...SCORES, armor_class: 14 });
    await setHitPoints(table, hero.actorId, { current: ARIA_HP, max: ARIA_HP });
    await setDisclosure(table, goblin.tokenId, "hitPoints", "VISIBLE");
    await claimFor(table, aria, hero.actorId);

    await grantAbility(table, hero.actorId, {
      name: "Longsword",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "5" },
      ],
    });
    const greatclub = await grantAbility(table, ogre.actorId, {
      name: "Greatclub",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "9" },
      ],
    });

    let combat = await startCombat(table);
    for (const [label, cast, initiative] of [
      ["Aria", hero, 20],
      ["Goblin", goblin, 10],
      ["Ogre", ogre, 5],
    ] as const) {
      combat = await addCombatant(table, combat.id, {
        label,
        actorId: cast.actorId,
        tokenId: cast.tokenId,
        initiative,
        isNpc: label !== "Aria",
      });
    }
    combat = await advanceTurn(table, combat.id);

    for (const client of [table.gm, aria.page, brom.page]) {
      await sitDown(table, client);
    }

    const seats = [
      ["the Game Master", table.gm],
      ["Aria", aria.page],
      ["Brom", brom.page],
    ] as const;
    const ariasHit = new RegExp(
      `Aria → Goblin.*Longsword.*vs ${GOBLIN_AC}: hit`,
    );
    const hitsOn = async (client: Page) =>
      (await attackLogOn(client)).filter((line) => ariasHit.test(line));

    await test.step("Aria attacks the goblin from her sheet, and every seat sees it within one second", async () => {
      // SC-001 and FR-002, asserted: from the moment Aria confirms the roll
      // to the slowest seat showing it, her own board included. Over a second
      // is measured once more: the Game Master declines the first offer, so
      // the goblin is owed exactly one when the next step looks, and Aria
      // swings again.
      await expectOnEverySeatWithinOneSecond(testInfo, {
        what: "attack reached every seat",
        seats,
        act: async () => {
          let rolled = 0;
          const said = await attackFromSheet(
            aria.page,
            hero.actorId,
            "Longsword",
            "Goblin",
            { onRoll: () => (rolled = Date.now()) },
          );
          expect(said, "the attack flow reports the hit").toMatch(
            new RegExp(`Aria → Goblin.*vs ${GOBLIN_AC}: hit`),
          );
          return rolled;
        },
        shown: async (client, attempt) => {
          const hits = await hitsOn(client);
          return (
            hits.length >= attempt &&
            hits.some((line) => /5 damage offered/.test(line))
          );
        },
        describe: async (client) =>
          (await attackLogOn(client)).join(" | ") || "an empty attack log",
        again: async () => {
          await answerOffer(table.gm, "Goblin", false);
          expect(
            await tokenHitPointsOf(table.gm, table.sceneId, goblin.tokenId),
            "a declined offer deals nothing",
          ).toBe(GOBLIN_HP);
        },
      });
    });

    await test.step("the Game Master takes the goblin's offer, and the bars move everywhere within one second", async () => {
      await expect
        .poll(() => offersOn(table.gm), { timeout: 10_000 })
        .toContain(`Goblin: take 5 damage?`);
      expect(
        await offersOn(aria.page),
        "the attacker is not asked what the goblin takes",
      ).toEqual([]);
      // FR-013, asserted: from the Game Master's Take to the slowest bar. Over
      // a second is measured once more: the goblin is healed back to full,
      // Aria hits it again, and the Game Master takes that offer instead.
      await expectOnEverySeatWithinOneSecond(testInfo, {
        what: "bars moved after an offer was taken",
        seats,
        prepare: async () => {
          await expect
            .poll(() => offersOn(table.gm), { timeout: 10_000 })
            .toContain(`Goblin: take 5 damage?`);
        },
        clock: "before-act",
        act: () => answerOffer(table.gm, "Goblin", true),
        shown: async (client) =>
          (await barCurrentOn(client, goblin.tokenId)) === GOBLIN_HP - 5,
        describe: async (client) =>
          `bar reads ${await barCurrentOn(client, goblin.tokenId)}`,
        again: async () => {
          await changeHitPoints(table.gm, goblin.tokenId, "HEALING", 5);
          for (const [who, client] of seats) {
            await expect
              .poll(() => barCurrentOn(client, goblin.tokenId), {
                timeout: 10_000,
                message: `${who}'s bar is back to ${GOBLIN_HP} before the offer is measured again`,
              })
              .toBe(GOBLIN_HP);
          }
          await attackFromSheet(aria.page, hero.actorId, "Longsword", "Goblin");
        },
      });
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, goblin.tokenId),
      ).toBe(GOBLIN_HP - 5);
      for (const [, client] of seats) {
        await expect
          .poll(async () => (await attackLogOn(client)).join(" | "), {
            timeout: 5_000,
          })
          .toMatch(/5 damage taken by/);
      }
    });

    await test.step("the ogre hits Aria: her offer waits for her, and the Game Master takes it on her behalf", async () => {
      await makeAttackAs(table.gm, {
        attackerTokenId: ogre.tokenId,
        abilityId: greatclub,
        targetTokenId: hero.tokenId,
      });
      await expect
        .poll(() => offersOn(aria.page), { timeout: 10_000 })
        .toContain("Aria: take 9 damage?");
      expect(await offersOn(brom.page), "nobody else is asked").toEqual([]);

      // Aria's player drops out and comes back: the offer is still there.
      await aria.page.goto("about:blank");
      await sitDown(table, aria.page);
      await expect
        .poll(() => offersOn(aria.page), {
          timeout: 20_000,
          message: "a pending offer survives a reconnect (FR-008)",
        })
        .toContain("Aria: take 9 damage?");

      await answerOffer(table.gm, "Aria", true);
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, hero.tokenId),
      ).toBe(ARIA_HP - 9);
      for (const client of [aria.page, brom.page]) {
        await expect
          .poll(async () => (await attackLogOn(client)).join(" | "), {
            timeout: 5_000,
            message: "the table is told the Game Master resolved it",
          })
          .toMatch(/9 damage taken by Game Master/);
      }
      await expect
        .poll(() => offersOn(aria.page), { timeout: 5_000 })
        .toEqual([]);
    });

    await test.step("auto-apply for this encounter applies to the goblin, and never to Aria", async () => {
      await openCombatPanel(table.gm);
      await table.gm.getByTestId("combat-auto-apply").selectOption("on");
      await expect(table.gm.getByTestId("combat-auto-apply")).toHaveValue(
        "on",
        { timeout: 10_000 },
      );

      const [onGoblin] = await makeAttackAs(table.gm, {
        attackerTokenId: ogre.tokenId,
        abilityId: greatclub,
        targetTokenId: goblin.tokenId,
      });
      expect(onGoblin.offer?.status, "applied with no offer to take").toBe(
        "APPLIED",
      );
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, goblin.tokenId),
      ).toBe(0);

      const [onAria] = await makeAttackAs(table.gm, {
        attackerTokenId: ogre.tokenId,
        abilityId: greatclub,
        targetTokenId: hero.tokenId,
      });
      expect(
        onAria.offer?.status,
        "damage to a player's character is always an offer",
      ).toBe("PENDING");
      await answerOffer(aria.page, "Aria", false);
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, hero.tokenId),
      ).toBe(ARIA_HP - 9);
    });

    await test.step("a hidden ogre attacks: players read Unknown, and nothing of it reaches their clients", async () => {
      // Out of sight behind a wall, and its name hidden by the Game Master.
      await addWall(table, { x: 96, y: -1024 }, { x: 96, y: 1024 });
      await must(
        table.gm,
        `mutation ($tokenId: UUID!) {
          setTokenNameVisibility(tokenId: $tokenId, visible: false) { tokenId }
        }`,
        { tokenId: ogre.tokenId },
      );
      ariaTraffic.reset();
      bromTraffic.reset();

      const [hidden] = await makeAttackAs(table.gm, {
        attackerTokenId: ogre.tokenId,
        abilityId: greatclub,
        targetTokenId: hero.tokenId,
      });
      expect(hidden.attacker.label, "the Game Master sees who it was").toBe(
        OGRE_NAME,
      );

      for (const [who, client] of [
        ["Aria", aria.page],
        ["Brom", brom.page],
      ] as const) {
        await expect
          .poll(async () => (await attackLogOn(client))[0] ?? "", {
            timeout: 10_000,
            message: `${who} is told Unknown attacked Aria`,
          })
          .toMatch(/^Unknown → Aria/);
        expect(
          (await attackLogOn(client))[0],
          `${who} is not told what it attacked with`,
        ).not.toContain("Greatclub");
      }
      await expect
        .poll(() => offersOn(aria.page), { timeout: 10_000 })
        .toContain("Aria: take 9 damage?");
      await expect
        .poll(async () => (await attackLogOn(table.gm))[0] ?? "", {
          timeout: 10_000,
        })
        .toContain(`${OGRE_NAME} → Aria`);

      for (const [who, traffic] of [
        ["Aria", ariaTraffic],
        ["Brom", bromTraffic],
      ] as const) {
        const relevant = aboutAttacks(traffic);
        expect(
          relevant.some(
            (body) =>
              body.includes('"attack"') || body.includes("sceneAttacks"),
          ),
          `${who}'s client did read the attack (the check below is not vacuous)`,
        ).toBe(true);
        expect(
          traffic.frames.some((frame) => frame.includes("attackId")),
          `${who}'s subscription did carry event 29`,
        ).toBe(true);
        for (const body of relevant) {
          expect(
            body,
            `${who} received the hidden ogre's token id`,
          ).not.toContain(ogre.tokenId);
          expect(
            body,
            `${who} received the hidden ogre's actor id`,
          ).not.toContain(ogre.actorId);
          expect(body, `${who} received the hidden ogre's name`).not.toContain(
            OGRE_NAME,
          );
          expect(
            body,
            `${who} received the hidden ogre's weapon`,
          ).not.toContain("Greatclub");
        }
        // Reported, not asserted: spec 045 sends every token and hides it by
        // not drawing it, so the scene's token list still carries the ogre's
        // id (research R8's consistency note).
        const elsewhere = [...traffic.responses, ...traffic.frames].filter(
          (body) => !relevant.includes(body),
        );
        testInfo.annotations.push({
          type: `${who}'s other traffic`,
          description:
            `${relevant.length} attack/offer messages checked; of ` +
            `${elsewhere.length} others, ` +
            `${elsewhere.filter((b) => b.includes(ogre.tokenId)).length} carry the ogre's token id and ` +
            `${elsewhere.filter((b) => b.includes(OGRE_NAME)).length} its name`,
        });
      }
    });
  } finally {
    await closeTable(table);
  }
});
