import { expect, test, type Page } from "./fixtures/test";
import {
  barCurrentOn,
  changeHitPoints,
  setAbilityScores,
  setDisclosure,
  setHitPoints,
  systemDataOf,
  tokenHitPointsOf,
} from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  placeToken,
  sitDown,
} from "../playtest/table";

/**
 * Spec 046 tasks Phase 5 (plan phase 3; FR-015, FR-016, FR-017, SC-008;
 * ADR-102): a token is its actor, or a copy of it.
 *
 * The independent test: Aria (linked) and two goblins placed from one NPC
 * (copies). Damage goblin A: goblin B and the goblin NPC are unchanged.
 * Damage Aria's token: her sheet changes. "Boblin", marked unique on his NPC
 * page, is placed linked. Goblin A, relinked from the token panel, takes the
 * NPC's hit points.
 *
 * The two Game Master controls this phase adds — the NPC page's Unique toggle
 * and the token panel's Linked / Copy choice — are pressed, not bypassed.
 * What every board draws is read from each client's engine; what the server
 * holds is read through `tokenStatus`, the resolver the bars come from.
 */

const SCORES = {
  strength: 10,
  dexterity: 12,
  constitution: 10,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};
const GOBLIN_HP = 7;
const ARIA_HP = 12;
const BOBLIN_HP = 11;

async function actorCount(page: Page, worldId: string): Promise<number> {
  const { worldActors } = await must<{ worldActors: { id: string }[] }>(
    page,
    `query ($worldId: UUID!) { worldActors(worldId: $worldId) { id } }`,
    { worldId },
  );
  return worldActors.length;
}

test("a linked token shares its actor's hit points, and each copy keeps its own", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(6 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Aria"],
    sceneName: "The Goblin Warren",
  });
  const [aria] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: -200, y: 0 },
      seat: aria,
    });
    await setAbilityScores(table, hero.actorId, SCORES);
    await setHitPoints(table, hero.actorId, { current: ARIA_HP, max: ARIA_HP });

    const goblinA = await placeCast(table, {
      label: "Goblin",
      at: { x: 200, y: 0 },
      sheet: {
        scores: SCORES,
        hitPoints: { current: GOBLIN_HP, max: GOBLIN_HP },
      },
    });
    const goblinB = await placeToken(table, goblinA.actorId, {
      at: { x: 300, y: 0 },
      label: "Goblin B",
    });

    await test.step("placement: the character is linked, the goblins are copies", async () => {
      const { tokens } = await must<{
        tokens: { tokenId: string; linked: boolean; tokenType: string }[];
      }>(
        table.gm,
        `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId linked tokenType } }`,
        { sceneId: table.sceneId },
      );
      const find = (id: string) => tokens.find((t) => t.tokenId === id)!;
      expect(find(hero.tokenId).linked, "Aria's token is Aria").toBe(true);
      expect(find(goblinA.tokenId).linked, "goblin A is a copy").toBe(false);
      expect(find(goblinB.tokenId).linked, "goblin B is a copy").toBe(false);
      expect(
        find(goblinB.tokenId).tokenType,
        "a goblin placed with no type named is typed as the NPC it is",
      ).toBe("npc");
    });

    for (const tokenId of [goblinA.tokenId, goblinB.tokenId]) {
      await setDisclosure(table, tokenId, "hitPoints", "VISIBLE");
    }
    for (const client of [table.gm, aria.page]) {
      await sitDown(table, client);
    }
    for (const tokenId of [goblinA.tokenId, goblinB.tokenId]) {
      await expect
        .poll(() => barCurrentOn(aria.page, tokenId), {
          timeout: 30_000,
          message: "Aria's board draws each goblin whole",
        })
        .toBe(GOBLIN_HP);
    }

    await test.step("damage to goblin A changes goblin A alone", async () => {
      const after = await changeHitPoints(
        table.gm,
        goblinA.tokenId,
        "DAMAGE",
        5,
      );
      expect(after.current).toBe(2);
      await expect
        .poll(() => barCurrentOn(aria.page, goblinA.tokenId), {
          timeout: 5_000,
          message: "goblin A's bar reads 2 on Aria's board, with no reload",
        })
        .toBe(2);
      expect(
        await barCurrentOn(aria.page, goblinB.tokenId),
        "goblin B's bar is whole",
      ).toBe(GOBLIN_HP);
      expect(
        await tokenHitPointsOf(table.gm, table.sceneId, goblinB.tokenId),
        "the server holds goblin B whole",
      ).toBe(GOBLIN_HP);
      const npc = await systemDataOf(table.gm, goblinA.actorId);
      expect(
        npc.resourceData?.current_hp,
        "the goblin NPC's own sheet is untouched",
      ).toBe(GOBLIN_HP);
    });

    await test.step("damage to Aria's token changes Aria's sheet", async () => {
      await changeHitPoints(table.gm, hero.tokenId, "DAMAGE", 4);
      const sheet = await systemDataOf(table.gm, hero.actorId);
      expect(sheet.resourceData?.current_hp).toBe(ARIA_HP - 4);
      await expect
        .poll(() => barCurrentOn(aria.page, hero.tokenId), {
          timeout: 5_000,
          message: "Aria's own bar moves with her sheet",
        })
        .toBe(ARIA_HP - 4);
    });

    await test.step("Boblin, marked unique on his page, is placed linked", async () => {
      const { createActor } = await must<{ createActor: { id: string } }>(
        table.gm,
        `mutation ($input: CreateActorInput!) { createActor(input: $input) { id } }`,
        {
          input: {
            worldId: table.worldId,
            label: "Boblin",
            isNpc: true,
            gameSystemId: table.system,
          },
        },
      );
      await setAbilityScores(table, createActor.id, SCORES);
      await setHitPoints(table, createActor.id, {
        current: BOBLIN_HP,
        max: BOBLIN_HP,
      });

      await table.gm.goto(
        `/world/${table.worldId}/actor/${createActor.id}/view`,
      );
      const toggle = table.gm.getByTestId("actor-unique-toggle");
      await expect(toggle).not.toBeChecked({ timeout: 20_000 });
      // Clicked rather than `check()`ed: the box is controlled by what the
      // server answers, so it changes a round trip after the click, not on it.
      await toggle.click();
      await expect(toggle).toBeChecked({ timeout: 10_000 });
      await expect(table.gm.getByTestId("actor-unique-block")).toContainText(
        "New tokens share this NPC's hit points.",
      );

      const boblin = await placeToken(table, createActor.id, {
        at: { x: 0, y: 200 },
      });
      expect(boblin.linked, "a unique NPC's token is linked").toBe(true);
      await changeHitPoints(table.gm, boblin.tokenId, "DAMAGE", 1);
      const sheet = await systemDataOf(table.gm, createActor.id);
      expect(
        sheet.resourceData?.current_hp,
        "a hit on Boblin's token is a hit on Boblin",
      ).toBe(BOBLIN_HP - 1);
    });

    await test.step("relinking goblin A from the token panel gives it the NPC's hit points", async () => {
      await sitDown(table, table.gm);
      const collapse = table.gm.getByTestId("world-dock-collapse");
      if (await collapse.isVisible().catch(() => false)) {
        await collapse.click();
      }
      await table.gm
        .getByTestId("token-panel-toggle-button")
        .click({ force: true });
      await table.gm
        .getByTestId(`token-list-item-${goblinA.tokenId}`)
        .click({ force: true });
      await expect(
        table.gm.getByTestId(`token-link-copy-${goblinA.tokenId}`),
        "the panel shows goblin A as a copy",
      ).toBeChecked({ timeout: 10_000 });
      // Not forced: the control has to be reachable where the popover puts
      // it. (It was not, until the popover learned to scroll: opened near the
      // top of a 720px screen, its first controls were cut off above it.)
      const linked = table.gm.getByTestId(
        `token-link-linked-${goblinA.tokenId}`,
      );
      await linked.click({ timeout: 10_000 });
      await expect(linked, "the panel shows goblin A linked").toBeChecked({
        timeout: 10_000,
      });
      await expect
        .poll(
          async () =>
            (
              await must<{ tokens: { tokenId: string; linked: boolean }[] }>(
                table.gm,
                `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId linked } }`,
                { sceneId: table.sceneId },
              )
            ).tokens.find((t) => t.tokenId === goblinA.tokenId)?.linked,
          { timeout: 10_000, message: "goblin A is linked on the server" },
        )
        .toBe(true);

      await expect
        .poll(
          () => tokenHitPointsOf(table.gm, table.sceneId, goblinA.tokenId),
          {
            timeout: 10_000,
            message: "goblin A now reads the NPC's hit points, not its own 2",
          },
        )
        .toBe(GOBLIN_HP);
      await expect
        .poll(() => barCurrentOn(aria.page, goblinA.tokenId), {
          timeout: 5_000,
          message: "and Aria's board draws it so, with no reload",
        })
        .toBe(GOBLIN_HP);
    });

    await test.step("no copy made an actor of its own (SC-008)", async () => {
      expect(
        await actorCount(table.gm, table.worldId),
        "Aria, the goblin NPC and Boblin — three actors for four creatures",
      ).toBe(3);
    });
  } finally {
    await closeTable(table);
  }
});
