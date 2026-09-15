import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { attackLogOn, grantAbility, makeAttackAs } from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";

/**
 * Spec 046 FR-044, finished: a multiattack is built by picking the world's
 * abilities and putting them in order, where T061 left a box for ability ids.
 *
 * Everything in the picker is done from the keyboard (focus a control, press
 * a key), because an ordered list that can only be rearranged with a mouse is
 * one a keyboard user cannot build. The ogre's multiattack is then used, and
 * the attack log shows one row per part, in the order the picker set.
 */

const SCORES = {
  strength: 18,
  dexterity: 8,
  constitution: 16,
  intelligence: 5,
  wisdom: 7,
  charisma: 7,
};
const OGRE_NAME = "Brugha the Ogre";

/** The picker's parts, as the names it shows, in order. */
async function partsOn(page: Page): Promise<string[]> {
  const rows = page.getByTestId("multiattack-part");
  const names: string[] = [];
  for (const row of await rows.all()) {
    names.push((await row.locator("span").nth(1).textContent()) ?? "");
  }
  return names;
}

async function press(page: Page, name: string, key: string): Promise<void> {
  const control = page.getByRole("button", { name, exact: true });
  await control.focus();
  await page.keyboard.press(key);
}

test("an ogre's multiattack is picked and ordered from the keyboard, and swings once per part", async ({
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
    sceneName: "The Ogre's Ford",
  });
  const [aria] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: -96, y: 0 },
      seat: aria,
      sheet: {
        scores: { ...SCORES, armor_class: 14 },
        hitPoints: { current: 40, max: 40 },
      },
    });
    const ogre = await placeCast(table, {
      label: OGRE_NAME,
      at: { x: 0, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 11 },
        hitPoints: { current: 59, max: 59 },
      },
    });
    const hitsAlways = (damage: string) => [
      { effectType: "ATTACK_ROLL" as const, formula: "1d20+100" },
      { effectType: "DAMAGE" as const, formula: damage },
    ];
    await grantAbility(table, ogre.actorId, {
      name: "Greatclub",
      classification: "feat",
      effects: hitsAlways("9"),
    });
    await grantAbility(table, ogre.actorId, {
      name: "Fist",
      classification: "feat",
      effects: hitsAlways("4"),
    });
    await grantAbility(table, ogre.actorId, {
      name: "Stomp",
      classification: "feat",
      effects: hitsAlways("2"),
    });
    const multiattack = await grantAbility(table, ogre.actorId, {
      name: "Multiattack",
      classification: "feat",
      effects: [],
    });

    await test.step("the Game Master builds Fist, Greatclub from the world's abilities, with keys only", async () => {
      await table.gm.goto(
        `/world/${table.worldId}/ability/${multiattack}/edit`,
      );
      const picker = table.gm.getByTestId("attack-fields-multiattack");
      await expect(picker).toBeVisible({ timeout: 15_000 });
      await expect(
        table.gm.getByRole("group", { name: "Multiattack" }),
        "the picker is a named group",
      ).toBeVisible();
      const choice = table.gm.getByLabel("Add an attack");
      await expect(choice).toBeEnabled({ timeout: 10_000 });
      await expect(
        choice.locator("option", { hasText: "Multiattack" }),
        "a multiattack is not offered as a part of itself",
      ).toHaveCount(0);

      for (const name of ["Stomp", "Greatclub", "Fist"]) {
        await choice.focus();
        await choice.selectOption({ label: name });
        await press(table.gm, "Add part", "Enter");
      }
      expect(await partsOn(table.gm)).toEqual(["Stomp", "Greatclub", "Fist"]);

      // Fist to the top, from the keyboard, twice; focus follows it.
      await press(table.gm, "Move Fist, part 3, up", "Enter");
      await expect(
        table.gm.getByRole("button", { name: "Move Fist, part 2, up" }),
      ).toBeFocused();
      await table.gm.keyboard.press("Enter");
      expect(await partsOn(table.gm)).toEqual(["Fist", "Stomp", "Greatclub"]);

      // Stomp out; focus lands on the part that took its place.
      await press(table.gm, "Remove Stomp, part 2", "Space");
      expect(await partsOn(table.gm)).toEqual(["Fist", "Greatclub"]);
      await expect(
        table.gm.getByRole("button", { name: "Remove Greatclub, part 2" }),
      ).toBeFocused();
      await expect(
        table.gm.getByRole("status").filter({ hasText: "Removed Stomp" }),
      ).toHaveCount(1);

      await expectNoAxeViolations(
        table.gm,
        '[data-testid="attack-fields-multiattack"]',
      );

      await press(table.gm, "Save attack details", "Enter");
      await expect(
        table.gm
          .getByTestId("attack-fields-editor")
          .getByRole("status")
          .filter({ hasText: "Saved" }),
      ).toBeVisible({ timeout: 10_000 });

      // What was saved is what is read back, in order.
      const { ability } = await must<{ ability: { multiattack: string[] } }>(
        table.gm,
        `query ($abilityId: UUID!) { ability(abilityId: $abilityId) { multiattack } }`,
        { abilityId: multiattack },
      );
      expect(ability.multiattack).toHaveLength(2);
      await table.gm.reload();
      await expect(table.gm.getByTestId("multiattack-part")).toHaveCount(2, {
        timeout: 15_000,
      });
      expect(await partsOn(table.gm)).toEqual(["Fist", "Greatclub"]);
    });

    await test.step("the ogre uses it on Aria: two attack rows, Fist then Greatclub", async () => {
      await sitDown(table, table.gm);
      const made = await makeAttackAs(table.gm, {
        attackerTokenId: ogre.tokenId,
        abilityId: multiattack,
        targetTokenId: hero.tokenId,
      });
      expect(made.map((a) => a.abilityName)).toEqual(["Fist", "Greatclub"]);
      expect(made.map((a) => a.outcome)).toEqual(["HIT", "HIT"]);

      await expect
        .poll(
          async () =>
            (await attackLogOn(table.gm)).filter((line) =>
              line.includes(`${OGRE_NAME} → Aria`),
            ),
          { timeout: 15_000 },
        )
        .toHaveLength(2);
      const rows = (await attackLogOn(table.gm)).filter((line) =>
        line.includes(`${OGRE_NAME} → Aria`),
      );
      expect(rows.some((line) => line.includes("Fist"))).toBe(true);
      expect(rows.some((line) => line.includes("Greatclub"))).toBe(true);
    });
  } finally {
    await closeTable(table);
  }
});
