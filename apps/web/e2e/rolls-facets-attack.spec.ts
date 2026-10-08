import { expect, test, type Page } from "./fixtures/test";
import { openDockTab } from "./fixtures/helpers";
import { openChat } from "./fixtures/rolls";
import {
  claimFor,
  grantAbility,
  setAbilityScores,
  setTraits,
  systemDataOf,
} from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";

/**
 * Spec 084 US4 (FR-014, research R7): a player spends Heroic Inspiration to
 * reroll a missed attack's to-hit. The reroll is the same attack, judged
 * against the defence it was made against.
 *
 * The goblin's Armor Class is 99, so the miss and the reroll's miss are both
 * certain without seeding the server's dice: what is asserted is the chain in
 * both chats, a second attack that points back at the first and is offered no
 * damage, and the sheet's Inspiration spent.
 */

const SCORES = {
  strength: 10,
  dexterity: 16,
  constitution: 12,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};

/** The to-hit rolls of the Dagger in a chat; damage is "Dagger damage". */
function daggerRolls(page: Page) {
  return page.getByTestId("roll-entry").filter({
    has: page.getByTestId("roll-label").getByText(/^Dagger:/),
  });
}

interface SceneAttack {
  id: string;
  outcome: string;
  defence: number | null;
  rerollOf: string | null;
  damage: { resultValue: number } | null;
  offer: { id: string } | null;
}

test("a missed attack rerolled with Heroic Inspiration is judged again against the same defence", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(5 * 60_000);

  const table = await openTable({
    browser,
    gm: page,
    testInfo,
    system: "dnd5e",
    players: ["Pip"],
  });
  const [pip] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Pip",
      at: { x: -192, y: 0 },
      seat: pip,
    });
    await setAbilityScores(table, hero.actorId, SCORES);
    await claimFor(table, pip, hero.actorId);
    // Only someone who may still act for the character may spend for it.
    await must(
      table.gm,
      `mutation ($input: SetActorPermissionInput!) {
        setActorPermission(input: $input) { actorId }
      }`,
      {
        input: { actorId: hero.actorId, userId: pip.userId, level: "EDITOR" },
      },
    );
    // 1. A goblin no dagger can hit.
    await placeCast(table, {
      label: "Goblin",
      at: { x: 0, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 99 },
        hitPoints: { current: 7, max: 7 },
      },
    });
    await grantAbility(table, hero.actorId, {
      name: "Dagger",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+3" },
        { effectType: "DAMAGE", formula: "2" },
      ],
    });
    await setTraits(table, hero.actorId, { inspiration: true });

    for (const client of [table.gm, pip.page]) {
      await sitDown(table, client);
    }

    // 2. The player attacks from the dock's sheet, and misses.
    const play = pip.page;
    await openDockTab(play, "actors");
    if (!(await play.getByTestId("in-pane-character-sheet").isVisible())) {
      await play.getByTestId(`actor-view-${hero.actorId}`).click();
    }
    const sheet = play.getByTestId("in-pane-character-sheet");
    await expect(sheet).toBeVisible({ timeout: 10_000 });
    await sheet
      .locator('[data-testid^="in-pane-roll-ability-"][data-attack="true"]')
      .filter({ hasText: "Dagger" })
      .first()
      .click();
    const flow = play.getByTestId("attack-flow");
    await expect(flow).toBeVisible();
    await flow
      .getByTestId("attack-flow-target")
      .selectOption({ label: "Goblin" });
    await flow.getByTestId("attack-flow-confirm").click();
    await expect(flow.getByTestId("attack-flow-result")).toBeVisible({
      timeout: 15_000,
    });
    await expect(flow.getByTestId("attack-flow-error")).toHaveCount(0);
    await flow.getByTestId("attack-flow-close").click();

    for (const client of [table.gm, play]) {
      await openChat(client);
    }
    const mine = daggerRolls(play).last();
    await expect(mine).toBeVisible({ timeout: 15_000 });
    const firstId = await mine.getAttribute("data-roll-id");
    expect(firstId).not.toBeNull();

    const attacksOnScene = async () =>
      (
        await must<{ sceneAttacks: SceneAttack[] }>(
          table.gm,
          `query ($sceneId: UUID!) {
            sceneAttacks(sceneId: $sceneId) {
              id outcome defence rerollOf damage { resultValue } offer { id }
            }
          }`,
          { sceneId: table.sceneId },
        )
      ).sceneAttacks;
    const [first] = await attacksOnScene();
    expect(first).toMatchObject({ outcome: "MISS", defence: 99 });

    // 3. The player rerolls the to-hit; only their chat offers it.
    const reroll = mine.getByTestId("roll-reroll-inspiration");
    await expect(reroll).toHaveText("Reroll (Heroic Inspiration)", {
      timeout: 15_000,
    });
    const gmFirst = table.gm.locator(`[data-roll-id="${firstId}"]`);
    await expect(gmFirst).toBeVisible({ timeout: 15_000 });
    await expect(gmFirst.getByTestId("roll-reroll-inspiration")).toHaveCount(0);
    await reroll.click();

    // 4. Both chats strike the first to-hit and tag the new one.
    for (const client of [table.gm, play]) {
      const struck = client.locator(`[data-roll-id="${firstId}"]`);
      await expect(struck).toHaveAttribute("data-rerolled", "true", {
        timeout: 15_000,
      });
      await expect(daggerRolls(client)).toHaveCount(2, { timeout: 15_000 });
      const second = daggerRolls(client).last();
      await expect(second.getByTestId("roll-spent")).toHaveText(
        "Rerolled with Heroic Inspiration",
      );
    }

    // The reroll is a second attack on the first one's defence: a miss,
    // with no damage rolled and nothing offered.
    await expect
      .poll(async () => (await attacksOnScene()).length, { timeout: 15_000 })
      .toBe(2);
    const second = (await attacksOnScene()).find(
      (attack) => attack.rerollOf === first.id,
    );
    expect(second).toMatchObject({
      outcome: "MISS",
      defence: 99,
      damage: null,
      offer: null,
    });

    // Inspiration is spent, and no button is left to press.
    expect(
      (await systemDataOf(table.gm, hero.actorId)).traitData?.inspiration,
    ).toBe(false);
    await expect(
      play.getByTestId("roll-entry").getByTestId("roll-reroll-inspiration"),
    ).toHaveCount(0);
  } finally {
    await closeTable(table);
  }
});
