import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { clickBoard, rightClickBoard } from "./fixtures/boardPointer";
import { storeCounts } from "./fixtures/lightingProbe";
import { tokenPosition } from "./fixtures/offline";
import {
  barCurrentOn,
  claimFor,
  grantAbility,
  setAbilityScores,
  setDisclosure,
  setHitPoints,
} from "../playtest/combat";
import {
  closeTable,
  must,
  openTable,
  placeCast,
  sitDown,
} from "../playtest/table";

/**
 * The right-click menu on the play field (owner decision 2026-09-15; the
 * engine has emitted `canvas_context_menu` since spec 031, and nothing
 * listened).
 *
 * What each viewer is offered is what the server would let them do: a player
 * attacks a creature with their own sheet's attacks, and a Game Master damages,
 * heals, links, names or removes it — or, on bare board, places a token or a
 * light. The same menu opens from the keyboard on the selected token, with the
 * ContextMenu key or Shift+F10, and gives focus back when it closes.
 */

const SCORES = {
  strength: 14,
  dexterity: 12,
  constitution: 12,
  intelligence: 10,
  wisdom: 10,
  charisma: 10,
};
const GOBLIN_HP = 7;

const menu = (page: Page) => page.getByTestId("canvas-menu");

async function rightClickToken(page: Page, tokenId: string): Promise<void> {
  const at = await tokenPosition(page, tokenId);
  if (!at) throw new Error(`token ${tokenId} is not on this board`);
  await rightClickBoard(page, at);
}

/** Right-click until the menu opens: the first press can land mid-settle. */
async function openMenuOn(
  page: Page,
  tokenId: string | null,
  empty?: { x: number; y: number },
) {
  await expect(async () => {
    if (tokenId) await rightClickToken(page, tokenId);
    else await rightClickBoard(page, empty!);
    await expect(menu(page)).toBeVisible({ timeout: 3_000 });
  }).toPass({ timeout: 30_000 });
}

async function activeTag(page: Page): Promise<string | null> {
  return page.evaluate(() => document.activeElement?.tagName ?? null);
}

test("the play field's right-click menu offers each viewer what they may do", async ({
  page,
  browser,
}, testInfo) => {
  test.setTimeout(8 * 60_000);

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
      at: { x: -192, y: 0 },
      seat: aria,
    });
    const goblin = await placeCast(table, {
      label: "Goblin",
      at: { x: 0, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 13 },
        hitPoints: { current: GOBLIN_HP, max: GOBLIN_HP },
      },
    });
    await setAbilityScores(table, hero.actorId, { ...SCORES, armor_class: 14 });
    await setHitPoints(table, hero.actorId, { current: 12, max: 12 });
    await setDisclosure(table, goblin.tokenId, "hitPoints", "VISIBLE");
    await claimFor(table, aria, hero.actorId);
    const longsword = await grantAbility(table, hero.actorId, {
      name: "Longsword",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+5" },
        { effectType: "DAMAGE", formula: "1d8+2" },
      ],
    });

    for (const client of [table.gm, aria.page]) {
      await sitDown(table, client);
      await expect
        .poll(() => storeCounts(client), { timeout: 20_000 })
        .toMatchObject({ tokens: 2 });
    }
    for (const client of [table.gm, aria.page]) {
      await expect
        .poll(() => barCurrentOn(client, goblin.tokenId), { timeout: 20_000 })
        .toBe(GOBLIN_HP);
    }

    await test.step("Aria right-clicks the goblin, chooses Attack, and the attack flow opens aimed at it", async () => {
      await openMenuOn(aria.page, goblin.tokenId);
      const attack = aria.page.getByTestId(`canvas-menu-attack-${longsword}`);
      await expect(attack).toHaveText("Attack Goblin with Longsword");
      await expect(
        aria.page.getByRole("menu", { name: "Actions for Goblin" }),
      ).toBeVisible();
      // Nothing of the Game Master's is offered to a player.
      for (const gmOnly of ["damage", "heal", "link", "name", "remove"]) {
        await expect(
          aria.page.getByTestId(`canvas-menu-${gmOnly}`),
        ).toHaveCount(0);
      }
      await expectNoAxeViolations(aria.page, '[data-testid="canvas-menu"]');

      await attack.click();
      const dialog = aria.page.getByTestId("canvas-menu-attack-dialog");
      await expect(dialog).toBeVisible();
      await expect(dialog.getByTestId("attack-flow")).toBeVisible();
      await expect(dialog.getByTestId("attack-flow-target")).toHaveValue(
        goblin.tokenId,
      );
      await expectNoAxeViolations(
        aria.page,
        '[data-testid="canvas-menu-attack-dialog"]',
      );

      await aria.page.keyboard.press("Escape");
      await expect(dialog).toBeHidden();
      await expect
        .poll(() => activeTag(aria.page), {
          message: "focus goes back to the board the menu was opened from",
        })
        .toBe("CANVAS");
    });

    await test.step("Aria right-clicking her own token is offered nothing", async () => {
      await rightClickToken(aria.page, hero.tokenId);
      await aria.page.waitForTimeout(1_500);
      await expect(menu(aria.page)).toBeHidden();
    });

    await test.step("the Game Master right-clicks the goblin and damages it by 3, and the bars move", async () => {
      await openMenuOn(table.gm, goblin.tokenId);
      for (const item of ["damage", "heal", "link", "name", "remove"]) {
        await expect(table.gm.getByTestId(`canvas-menu-${item}`)).toBeVisible();
      }
      await expectNoAxeViolations(table.gm, '[data-testid="canvas-menu"]');

      await table.gm.getByTestId("canvas-menu-damage").click();
      const dialog = table.gm.getByTestId("canvas-menu-hit-points-dialog");
      await expect(dialog).toBeVisible();
      const amount = dialog.getByTestId("canvas-menu-hit-points-amount");
      await expect(amount).toBeFocused();
      await expectNoAxeViolations(
        table.gm,
        '[data-testid="canvas-menu-hit-points-dialog"]',
      );
      await amount.fill("3");
      await dialog.getByTestId("canvas-menu-hit-points-apply").click();
      await expect(dialog).toBeHidden();

      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
      ] as const) {
        await expect
          .poll(() => barCurrentOn(client, goblin.tokenId), {
            timeout: 15_000,
            message: `${who}'s bar for the goblin reads ${GOBLIN_HP - 3}`,
          })
          .toBe(GOBLIN_HP - 3);
      }
    });

    await test.step("the Game Master right-clicks bare board and adds a light there", async () => {
      const before = (await storeCounts(table.gm))!.lights;
      const spot = { x: 150, y: -150 };
      await openMenuOn(table.gm, null, spot);
      await expect(
        table.gm.getByRole("menu", { name: "Board actions" }),
      ).toBeVisible();
      await expect(
        table.gm.getByTestId("canvas-menu-place-token"),
      ).toBeVisible();
      await table.gm.getByTestId("canvas-menu-add-light").click();

      for (const client of [table.gm, aria.page]) {
        await expect
          .poll(async () => (await storeCounts(client))?.lights, {
            timeout: 15_000,
          })
          .toBe(before + 1);
      }
      const { lightSources } = await must<{
        lightSources: {
          x: number;
          y: number;
          radius: number;
          brightRadius: number;
        }[];
      }>(
        table.gm,
        `query ($sceneId: UUID!) { lightSources(sceneId: $sceneId) { x y radius brightRadius } }`,
        { sceneId: table.sceneId },
      );
      const added = lightSources.at(-1)!;
      expect(Math.hypot(added.x - spot.x, added.y - spot.y)).toBeLessThan(2);
      expect(added.brightRadius).toBeLessThan(added.radius);
    });

    await test.step("a player's bare board offers nothing", async () => {
      await rightClickBoard(aria.page, { x: 150, y: 150 });
      await aria.page.waitForTimeout(1_500);
      await expect(menu(aria.page)).toBeHidden();
    });

    await test.step("from the keyboard: Shift+F10 on the selected goblin, Damage 2, and Escape closes", async () => {
      // Select the goblin the way a Game Master does, with a click.
      await expect(async () => {
        const at = (await tokenPosition(table.gm, goblin.tokenId))!;
        await clickBoard(table.gm, at);
        expect(
          await table.gm.evaluate(
            () => window.__worldProbe?.state().selectedTokenId ?? null,
          ),
        ).toBe(goblin.tokenId);
      }).toPass({ timeout: 20_000 });

      await table.gm.keyboard.press("Shift+F10");
      await expect(
        table.gm.getByRole("menu", { name: "Actions for Goblin" }),
      ).toBeVisible();
      // Focus is in the menu: the first item, reached without a pointer.
      await expect(table.gm.getByTestId("canvas-menu-damage")).toBeFocused();
      await table.gm.keyboard.press("Enter");
      const dialog = table.gm.getByTestId("canvas-menu-hit-points-dialog");
      await expect(dialog).toBeVisible();
      await table.gm.keyboard.type("2");
      await table.gm.keyboard.press("Enter");
      await expect(dialog).toBeHidden();
      await expect
        .poll(() => barCurrentOn(aria.page, goblin.tokenId), {
          timeout: 15_000,
        })
        .toBe(GOBLIN_HP - 5);

      // Open again, walk the menu, and leave it with Escape: nothing chosen,
      // nothing moved, and focus back where it was.
      const position = await tokenPosition(table.gm, goblin.tokenId);
      const focusedBefore = await activeTag(table.gm);
      await table.gm.keyboard.press("Shift+F10");
      await expect(menu(table.gm)).toBeVisible();
      await table.gm.keyboard.press("ArrowDown");
      await expect(table.gm.getByTestId("canvas-menu-heal")).toBeFocused();
      await table.gm.keyboard.press("Escape");
      await expect(menu(table.gm)).toBeHidden();
      expect(
        await tokenPosition(table.gm, goblin.tokenId),
        "the arrow key moved the menu's focus, not the selected token",
      ).toEqual(position);
      await expect
        .poll(() => activeTag(table.gm), {
          message: "Escape gives focus back to where the menu was opened from",
        })
        .toBe(focusedBefore);
    });
  } finally {
    await closeTable(table);
  }
});
