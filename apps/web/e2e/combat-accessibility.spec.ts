import type { Locator } from "@playwright/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { openDockTab } from "./fixtures/helpers";
import { expect, test, type Page } from "./fixtures/test";
import {
  actFromTracker,
  addCombatant,
  advanceTurn,
  claimFor,
  grantAbility,
  openCombatPanel,
  setAbilityCost,
  setAbilityScores,
  setHitPoints,
  startCombat,
  tokenHitPointsOf,
} from "../playtest/combat";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";

/**
 * Spec 046 T107: the fight's controls, by keyboard alone, held to WCAG 2.2 AA.
 *
 * What spec 046 added to a table — the Game Master's Damage and Heal, the
 * budget and legendary pips, "Spend legendary action", "Add lair", the attack
 * flow on a character's sheet and in the tracker, the offer prompt and the
 * attack log — each checked by axe where it is drawn, and driven without a
 * pointer: every press here is a key. (A `<select>` is chosen with
 * `selectOption`, which sets the value the way a keyboard does and does not
 * click.)
 *
 * The focus rule it proves: when the attack flow closes — Escape, Cancel or
 * Done — focus goes back to the control that opened it, on the sheet and in
 * the tracker, rather than to the page; and when an offer is answered, focus
 * moves to the next offer's Take.
 */

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
const GOBLIN_HP = 7;

/** A focused control shows it: an outline, or a ring drawn as a shadow. */
async function showsFocus(target: Locator): Promise<boolean> {
  return target.evaluate((element) => {
    const style = getComputedStyle(element);
    const outlined =
      style.outlineStyle !== "none" &&
      Number.parseFloat(style.outlineWidth) > 0;
    return (
      element.matches(":focus-visible") &&
      (outlined || style.boxShadow !== "none")
    );
  });
}

/**
 * Focus `target` the way a keyboard does: a script's `focus()` after a click
 * need not show a focus ring, and a Tab away and back always does.
 */
async function keyboardFocus(page: Page, target: Locator): Promise<void> {
  await target.focus();
  await page.keyboard.press("Tab");
  await page.keyboard.press("Shift+Tab");
  await expect(target).toBeFocused();
}

/**
 * The contrast of `target`'s text against what is painted behind it, as WCAG
 * computes it. Axe leaves text over a translucent panel as "needs review"
 * rather than a violation, and the dock and the table feed are translucent,
 * so their coloured words are measured here instead. Colours are resolved
 * through a canvas, which reads any CSS colour (the theme's are `oklch`), and
 * the backgrounds are composited from the element outwards over the page's.
 */
async function contrastOf(target: Locator): Promise<number> {
  return target.evaluate((element) => {
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const context = canvas.getContext("2d", { willReadFrequently: true })!;
    const rgba = (colour: string): [number, number, number, number] => {
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = "rgba(0,0,0,0)";
      context.fillStyle = colour;
      context.fillRect(0, 0, 1, 1);
      const [r, g, b, a] = context.getImageData(0, 0, 1, 1).data;
      return [r, g, b, a / 255];
    };
    const layers: [number, number, number, number][] = [];
    for (
      let node: Element | null = element;
      node !== null;
      node = node.parentElement
    ) {
      const layer = rgba(getComputedStyle(node).backgroundColor);
      if (layer[3] > 0) layers.push(layer);
      if (layer[3] >= 1) break;
    }
    let behind: [number, number, number] = [255, 255, 255];
    for (const [r, g, b, a] of layers.reverse()) {
      behind = [
        r * a + behind[0] * (1 - a),
        g * a + behind[1] * (1 - a),
        b * a + behind[2] * (1 - a),
      ];
    }
    const [tr, tg, tb, ta] = rgba(getComputedStyle(element).color);
    const text = [
      tr * ta + behind[0] * (1 - ta),
      tg * ta + behind[1] * (1 - ta),
      tb * ta + behind[2] * (1 - ta),
    ];
    const luminance = (rgb: number[]) => {
      const [r, g, b] = rgb.map((channel) => {
        const c = channel / 255;
        return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
      });
      return 0.2126 * r + 0.7152 * g + 0.0722 * b;
    };
    const [light, dark] = [luminance(text), luminance(behind)].sort(
      (x, y) => y - x,
    );
    return (light + 0.05) / (dark + 0.05);
  });
}

/** Tab forward until `target` has focus, at most `limit` presses. */
async function tabTo(page: Page, target: Locator, limit = 8): Promise<void> {
  for (let press = 0; press < limit; press += 1) {
    if (await target.evaluate((element) => element === document.activeElement))
      return;
    await page.keyboard.press("Tab");
  }
  await expect(target).toBeFocused();
}

test("the fight's controls work from the keyboard, and focus comes back", async ({
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
    sceneName: "The Quiet Hall",
  });
  const [aria] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: -192, y: 0 },
      seat: aria,
    });
    await setAbilityScores(table, hero.actorId, { ...SCORES, armor_class: 14 });
    await setHitPoints(table, hero.actorId, { current: 30, max: 30 });
    await claimFor(table, aria, hero.actorId);
    const longsword = await grantAbility(table, hero.actorId, {
      name: "Longsword",
      classification: "feat",
      effects: [
        { effectType: "ATTACK_ROLL", formula: "1d20+100" },
        { effectType: "DAMAGE", formula: "1" },
      ],
    });
    // Five feet, so a swing across the hall is flagged, and the flag's words
    // are on the flow and the log for axe and the contrast check to read.
    await setAbilityCost(table, longsword, { actionCost: "ACTION", reach: 5 });
    const goblin = await placeCast(table, {
      label: "Goblin",
      at: { x: 0, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 13 },
        hitPoints: { current: GOBLIN_HP, max: GOBLIN_HP },
      },
    });
    const dragon = await placeCast(table, {
      label: DRAGON,
      at: { x: 192, y: 0 },
      tokenType: "npc",
      sheet: {
        scores: { ...SCORES, armor_class: 19 },
        hitPoints: { current: 200, max: 200 },
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

    const combat = await startCombat(table);
    for (const [label, cast, initiative] of [
      ["Aria", hero, 20],
      [DRAGON, dragon, 10],
      ["Goblin", goblin, 5],
    ] as const) {
      await addCombatant(table, combat.id, {
        label,
        actorId: cast.actorId,
        tokenId: cast.tokenId,
        initiative,
        isNpc: label !== "Aria",
      });
    }
    await advanceTurn(table, combat.id); // Aria's turn

    for (const client of [table.gm, aria.page]) {
      await sitDown(table, client);
    }
    await openCombatPanel(table.gm);

    await test.step("Damage by keyboard: Tab from initiative, type, Enter, and the goblin is out", async () => {
      const gm = table.gm;
      await gm.getByLabel("Initiative for Goblin").focus();
      await gm.keyboard.press("Tab");
      const amount = gm.getByLabel("Hit points to change for Goblin");
      await expect(amount).toBeFocused();
      expect(await showsFocus(amount), "the amount shows its focus").toBe(true);
      await gm.keyboard.type(String(GOBLIN_HP));
      await gm.keyboard.press("Tab");
      const damage = gm.getByRole("button", { name: "Damage Goblin" });
      await expect(damage).toBeFocused();
      expect(await showsFocus(damage), "Damage shows its focus").toBe(true);
      await gm.keyboard.press("Enter");
      await expect(
        amount,
        "focus goes back to the amount, not to the page",
      ).toBeFocused();
      await expect
        .poll(() => tokenHitPointsOf(gm, table.sceneId, goblin.tokenId), {
          timeout: 10_000,
          message: "the goblin took all seven",
        })
        .toBe(0);
    });

    await test.step("the tracker passes axe on both seats", async () => {
      for (const client of [table.gm, aria.page]) {
        await openCombatPanel(client);
        await expect(
          client
            .getByTestId("combatant-row")
            .filter({ hasText: "Goblin" })
            .getByTestId("combatant-out"),
          "an out combatant is on the tracker axe reads",
        ).toHaveText("Out: 0 hit points", { timeout: 15_000 });
        await expect(
          client
            .getByTestId("combatant-row")
            .filter({ hasText: DRAGON })
            .getByTestId("budget-legendary"),
        ).toBeVisible({ timeout: 15_000 });
        await expectNoAxeViolations(client, '[data-testid="combat-panel"]');
      }
      const pip = table.gm
        .getByTestId("combatant-row")
        .filter({ hasText: DRAGON })
        .getByTestId("budget-legendary");
      await expect(pip, "a pip is named in words").toHaveAttribute(
        "aria-label",
        "Legendary actions: 3 of 3 left",
      );
    });

    await test.step("Add lair by keyboard", async () => {
      const gm = table.gm;
      const name = gm.getByLabel("Add lair", { exact: true });
      await keyboardFocus(gm, name);
      expect(await showsFocus(name), "the lair's name shows its focus").toBe(
        true,
      );
      await gm.keyboard.type(LAIR);
      await gm.keyboard.press("Tab");
      await expect(gm.getByTestId("combat-add-lair-button")).toBeFocused();
      await gm.keyboard.press("Enter");
      await expect(
        gm.getByRole("button", { name: `Lair action for ${LAIR}` }),
      ).toBeVisible({ timeout: 15_000 });
      await expectNoAxeViolations(gm, '[data-testid="combat-panel"]');
    });

    await test.step("a legendary action by keyboard, and focus back on its button", async () => {
      const gm = table.gm;
      const spend = gm.getByRole("button", {
        name: `Spend legendary action for ${DRAGON}`,
      });
      await keyboardFocus(gm, spend);
      expect(await showsFocus(spend), "the button shows its focus").toBe(true);
      await gm.keyboard.press("Enter");
      await expect(spend).toHaveAttribute("aria-expanded", "true");
      await gm.keyboard.press("Tab");
      const ability = gm.getByTestId("combatant-act-ability");
      await expect(ability).toBeFocused();
      await expect(
        ability.locator("option").filter({ hasText: "Tail Swipe" }),
      ).toHaveCount(1, { timeout: 15_000 });
      await ability.selectOption({ label: "Tail Swipe (costs 1)" });
      const flow = gm.getByTestId("combatant-act").getByTestId("attack-flow");
      await expect(flow).toBeVisible();
      await expect(
        ability,
        "choosing from the list leaves focus in the list",
      ).toBeFocused();
      await gm.keyboard.press("Tab");
      const target = flow.getByTestId("attack-flow-target");
      await expect(target).toBeFocused();
      await target.selectOption({ label: "Aria" });
      await gm.waitForTimeout(750); // the preview answers
      await tabTo(gm, flow.getByTestId("attack-flow-confirm"));
      await gm.keyboard.press("Enter");
      const result = flow.getByRole("status");
      await expect(result, "the result is announced").toContainText(
        `${DRAGON} → Aria · Tail Swipe`,
        { timeout: 15_000 },
      );
      await expectNoAxeViolations(gm, '[data-testid="combatant-act"]');
      await gm.keyboard.press("Escape");
      await expect(flow).toBeHidden();
      await expect(
        spend,
        "focus returns to what opened the flow",
      ).toBeFocused();

      // A second, so Aria has two offers waiting for the next step.
      await actFromTracker(gm, DRAGON, "Tail Swipe", "Aria");
    });

    await test.step("Aria answers two offers by keyboard", async () => {
      const prompt = aria.page.getByTestId("offer-prompt");
      await expect(prompt.getByTestId("offer-row")).toHaveCount(2, {
        timeout: 15_000,
      });
      await expect(
        aria.page.getByTestId("offer-announcement"),
        "their arrival is announced",
      ).toHaveText("2 offers waiting");
      await expectNoAxeViolations(aria.page, '[data-testid="table-feed"]');
      const firstTake = prompt.getByTestId("offer-take").first();
      await keyboardFocus(aria.page, firstTake);
      expect(await showsFocus(firstTake), "Take shows its focus").toBe(true);
      await aria.page.keyboard.press("Enter");
      await expect(prompt.getByTestId("offer-row")).toHaveCount(1, {
        timeout: 15_000,
      });
      await expect(
        prompt.getByTestId("offer-take"),
        "focus moves to the next offer, not to the page",
      ).toBeFocused();
      await aria.page.keyboard.press("Tab");
      await expect(prompt.getByTestId("offer-decline")).toBeFocused();
      await aria.page.keyboard.press("Enter");
      await expect(prompt).toBeHidden({ timeout: 15_000 });
    });

    await test.step("the sheet's attack flow by keyboard, and focus back on the attack", async () => {
      const player = aria.page;
      await openDockTab(player, "actors");
      if (!(await player.getByTestId("in-pane-character-sheet").isVisible())) {
        const view = player.getByTestId(`actor-view-${hero.actorId}`);
        await view.focus();
        await player.keyboard.press("Enter");
      }
      const sheet = player.getByTestId("in-pane-character-sheet");
      await expect(sheet).toBeVisible({ timeout: 10_000 });
      const attack = sheet
        .locator('[data-testid^="in-pane-roll-ability-"][data-attack="true"]')
        .filter({ hasText: "Longsword" })
        .first();
      await expect(attack).toBeEnabled({ timeout: 15_000 });

      // Opened and left with Escape.
      await keyboardFocus(player, attack);
      expect(await showsFocus(attack), "the attack shows its focus").toBe(true);
      await player.keyboard.press("Enter");
      const flow = player.getByTestId("attack-flow");
      await expect(flow).toBeVisible();
      await expect(
        flow.getByTestId("attack-flow-target"),
        "focus moves into the flow it opened",
      ).toBeFocused();
      await expectNoAxeViolations(player, '[data-testid="attack-flow"]');
      await player.keyboard.press("Escape");
      await expect(flow).toBeHidden();
      await expect(attack, "Escape returns focus to the attack").toBeFocused();

      // Opened, rolled, and left with Done.
      await player.keyboard.press("Enter");
      await expect(flow).toBeVisible();
      await flow
        .getByTestId("attack-flow-target")
        .selectOption({ label: "Goblin" });
      await player.waitForTimeout(750); // the preview answers
      await tabTo(player, flow.getByTestId("attack-flow-confirm"));
      await player.keyboard.press("Enter");
      await expect(flow.getByRole("status")).toContainText("Aria → Goblin", {
        timeout: 15_000,
      });
      await expectNoAxeViolations(player, '[data-testid="attack-flow"]');
      const flag = flow.getByTestId("attack-flow-flag").first();
      await expect(flag, "a warning is shown to measure").toBeVisible();
      expect(
        await contrastOf(flag),
        "a warning's words are legible (WCAG 1.4.3, 4.5:1)",
      ).toBeGreaterThanOrEqual(4.5);
      await player.keyboard.press("Tab");
      const done = flow.getByTestId("attack-flow-close");
      await expect(done).toBeFocused();
      await expect(done).toHaveText("Done");
      await player.keyboard.press("Enter");
      await expect(flow).toBeHidden();
      await expect(attack, "Done returns focus to the attack").toBeFocused();

      // The Game Master's log is told, in a sentence.
      await expect(
        table.gm.getByTestId("attack-log-announcement"),
      ).toContainText("Aria → Goblin · Longsword", { timeout: 15_000 });
      // The outcome's green and the flag's amber are both on the entry axe
      // reads. The log must also stay opaque: over a translucent panel axe
      // only asks for review, and at 90% the board took the green to 4.46:1.
      for (const board of [player, table.gm]) {
        const entry = board.getByTestId("attack-log-entry").first();
        const outcome = entry.getByTestId("attack-log-outcome");
        await expect(outcome).toBeVisible();
        await expect(entry.getByTestId("attack-log-flags")).toBeVisible({
          timeout: 15_000,
        });
        expect(
          await contrastOf(outcome),
          "the outcome is legible over the board (WCAG 1.4.3, 4.5:1)",
        ).toBeGreaterThanOrEqual(4.5);
      }
      await expectNoAxeViolations(player, '[data-testid="table-feed"]');
      await expectNoAxeViolations(table.gm, '[data-testid="table-feed"]');
    });
  } finally {
    await closeTable(table);
  }
});
