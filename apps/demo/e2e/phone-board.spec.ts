import { expect, test, type Page } from "@playwright/test";
import { openDemo, WORLD_ID } from "./support";

/**
 * Spec 102 US1: a player plays from their phone. The map is the screen; five
 * tiles at the bottom do the four jobs (move my token, roll, my sheet, chat)
 * and More holds the rest. Contract: specs/102-mobile-play-board/contracts.
 */

const PHONE = { width: 412, height: 915 };

/** The share of the viewport no control covers, by the contract's formula. */
async function mapShare(page: Page): Promise<number> {
  return page.evaluate(() => {
    const w = innerWidth;
    const h = innerHeight;
    const roots = [
      '[data-testid="phone-row"] button',
      '[data-testid="phone-top-stack"] > *',
      '[data-testid="demo-notice"]',
    ];
    const boxes = roots
      .flatMap((sel) => [...document.querySelectorAll(sel)])
      .map((el) => el.getBoundingClientRect())
      .filter((r) => r.width > 0 && r.height > 0);
    // Union by sampling an 8px grid: exact enough, and no rectangle algebra.
    let covered = 0;
    let total = 0;
    for (let y = 4; y < h; y += 8)
      for (let x = 4; x < w; x += 8) {
        total++;
        if (
          boxes.some(
            (r) => x >= r.left && x < r.right && y >= r.top && y < r.bottom,
          )
        )
          covered++;
      }
    return (total - covered) / total;
  });
}

/** Every visible button is thumb-sized and nothing sits on top of it. */
async function expectReachable(page: Page, scope: string) {
  const problems = await page.evaluate((scope) => {
    const out: string[] = [];
    for (const el of document.querySelectorAll<HTMLElement>(
      `${scope} button`,
    )) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.height === 0) continue;
      const name = el.getAttribute("aria-label") ?? el.textContent?.trim();
      if (r.width < 44 || r.height < 44)
        out.push(`${name}: ${Math.round(r.width)}x${Math.round(r.height)}`);
      const hit = document.elementFromPoint(
        r.left + r.width / 2,
        r.top + r.height / 2,
      );
      if (!hit || !el.contains(hit)) out.push(`${name}: covered`);
    }
    return out;
  }, scope);
  expect(problems, `${scope}: every button reachable`).toEqual([]);
}

test("a player plays from a phone", async ({ browser, baseURL }) => {
  const { page } = await openDemo(browser, baseURL);
  try {
    await page.setViewportSize(PHONE);
    await page.goto(`/demo/world/${WORLD_ID}/play`);

    await test.step("switch the demo to the player", async () => {
      const notice = page.getByTestId("demo-notice");
      await notice.getByTestId("demo-notice-toggle").click();
      await notice.getByRole("button", { name: "View as player" }).click();
      await expect(page.getByTestId("demo-viewer")).toContainText(
        "Viewing as a player",
      );
    });

    const board = page.getByTestId("phone-board");
    await expect(board).toHaveAttribute("data-role", "player", {
      timeout: 60_000,
    });
    await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });

    await test.step("five tiles, and no desktop chrome", async () => {
      const row = page.getByTestId("phone-row");
      await expect(row.getByRole("button")).toHaveText([
        "Me",
        "Dice",
        "Sheet",
        "Chat",
        "More",
      ]);
      await expect(page.locator('[data-testid^="gm-tool-"]')).toHaveCount(0);
      await expect(
        page.locator('[data-testid^="world-dock-tab-"]'),
      ).toHaveCount(0);
      await expect(page.getByTestId("dice-roller-panel")).toHaveCount(0);
      await expect(page.getByTestId("token-panel-toggle-button")).toHaveCount(
        0,
      );
    });

    await test.step("the map is the screen", async () => {
      expect(await mapShare(page)).toBeGreaterThanOrEqual(0.85);
      await expectReachable(page, '[data-testid="phone-row"]');
      await expectReachable(page, '[data-testid="demo-notice"]');
    });

    await test.step("Me finds my token", async () => {
      await page.getByTestId("phone-row-me").click();
      await expect(page.getByTestId("phone-me-note")).toHaveCount(0);
      await expect(page.getByTestId("phone-me-chooser")).toHaveCount(0);
    });

    const sheet = page.getByTestId("board-sheet");
    for (const [tile, content] of [
      ["dice", "dice"],
      ["sheet", "sheet"],
      ["chat", "chat"],
      ["more", "more"],
    ] as const) {
      await test.step(`${tile} opens its sheet`, async () => {
        await page.getByTestId(`phone-row-${tile}`).click();
        await expect(sheet).toHaveAttribute("data-content", content);
        await expect(sheet).toHaveAttribute("data-size", "half");
        await expect(page.getByTestId(`phone-row-${tile}`)).toHaveAttribute(
          "aria-expanded",
          "true",
        );
        // Measured where it settles, not partway up its slide in.
        await sheet.evaluate((el) =>
          Promise.all(el.getAnimations().map((a) => a.finished)),
        );
        const box = await sheet.boundingBox();
        expect(box!.height).toBeLessThanOrEqual(PHONE.height * 0.6);
        await expectReachable(page, '[data-testid="board-sheet"] header');
        await expectReachable(page, '[data-testid="phone-row"]');
      });
    }

    await test.step("Dice holds the roller; Sheet holds my character", async () => {
      await page.getByTestId("phone-row-dice").click();
      await expect(sheet.getByTestId("dice-roller-panel")).toBeVisible();

      // One tap per die, all seven on one thumb-sized row.
      const quick = sheet.getByRole("group", { name: "Quick roll" });
      const boxes = await quick
        .getByRole("button")
        .evaluateAll((els) => els.map((el) => el.getBoundingClientRect()));
      expect(boxes).toHaveLength(7);
      expect(new Set(boxes.map((box) => Math.round(box.top))).size).toBe(1);
      for (const box of boxes) expect(box.height).toBeGreaterThanOrEqual(44);
      const diceResult = sheet.getByTestId("dice-roll-result");
      await sheet.getByTestId("dice-quick-d6").click();
      await expect(diceResult).toContainText("1d6", { timeout: 15_000 });
      await sheet.getByTestId("dice-quick-coin").click();
      await expect(sheet.getByTestId("dice-formula-input")).toHaveValue("1dc");
      await expect(diceResult).toBeVisible({ timeout: 15_000 });
      await expect(sheet.getByTestId("dice-roll-error")).toHaveCount(0);

      // Closing the tray and opening it again keeps the formula and the
      // answer (the owner's T017: a second tap made the roll vanish).
      await page.getByTestId("phone-row-dice").click();
      await expect(sheet).toHaveCount(0);
      await page.getByTestId("phone-row-dice").click();
      await expect(sheet.getByTestId("dice-formula-input")).toHaveValue("1dc");
      await expect(sheet.getByTestId("dice-roll-result")).toBeVisible();

      await page.getByTestId("phone-row-sheet").click();
      await expect(sheet.getByTestId("in-pane-character-sheet")).toBeVisible();
      await expect(sheet.getByTestId("in-pane-sheet-dismiss")).toHaveCount(0);
    });

    await test.step("Sheet is rolled from where things are", async () => {
      // The owner's T017 try-out: no pinned bar of rolls; abilities and
      // weapons roll from their usual place, with editing behind a toggle.
      const rollList = sheet.getByTestId("in-pane-roll-list");
      const body = sheet.getByTestId("in-pane-sheet-body");
      await expect(body.getByTestId("dnd5e-actor-sheet")).toBeVisible({
        timeout: 15_000,
      });
      const [listBox, bodyBox] = await Promise.all([
        rollList.boundingBox(),
        body.boundingBox(),
      ]);
      expect(listBox!.y).toBeGreaterThanOrEqual(bodyBox!.y + bodyBox!.height);
      await expect(body).toHaveAttribute("data-editing", "false");
      await expect(body.getByTestId("dnd5e-score-strength-input")).toHaveCount(
        0,
      );

      const perception = body.getByTestId("dnd5e-roll-skill-perception");
      await perception.scrollIntoViewIfNeeded();
      const tap = await perception.boundingBox();
      expect(tap!.height, "a skill is thumb-sized").toBeGreaterThanOrEqual(44);
      await perception.click();
      const result = sheet.getByTestId("in-pane-roll-result");
      await expect(result).toBeVisible({ timeout: 15_000 });
      // The answer is pinned under the name, not left at the top of a sheet
      // scrolled away from.
      await expect(result).toBeInViewport();

      await expect(result).toContainText("Perception");

      // The last answer stays until the next lands, named, so the pinned
      // top never collapses between taps.
      await body.getByTestId("dnd5e-roll-save-dexterity").click();
      await expect(result).toContainText("Dexterity save", { timeout: 15_000 });

      // An attack opens in view, not out of sight (the owner's T017 "long
      // sword no worky").
      const attack = rollList.locator('[data-attack="true"]').first();
      await attack.scrollIntoViewIfNeeded();
      expect((await attack.boundingBox())!.height).toBeGreaterThanOrEqual(44);
      await attack.click();
      await expect(sheet.getByTestId("attack-flow")).toBeInViewport();

      const edit = sheet.getByTestId("in-pane-sheet-edit");
      await edit.click();
      await expect(edit).toHaveAttribute("aria-pressed", "true");
      await expect(
        body.getByTestId("dnd5e-score-strength-input"),
      ).toBeVisible();
      await expect(perception).toHaveCount(0);
      await edit.click();
      await expect(
        body.getByTestId("dnd5e-roll-skill-perception"),
      ).toBeVisible();
    });

    await test.step("Chat opens on the newest, composer in reach", async () => {
      await page.getByTestId("phone-row-chat").click();
      const log = sheet.getByTestId("chat-log");
      // The rolls above landed in the log; it opens on the last of them.
      await expect(log).toContainText("Dexterity save", { timeout: 15_000 });
      await expect
        .poll(() =>
          log.evaluate(
            (el) => el.scrollHeight - el.scrollTop - el.clientHeight,
          ),
        )
        .toBeLessThanOrEqual(2);
      await expect(sheet.getByTestId("chat-input")).toBeInViewport();
      await expect(sheet.getByTestId("chat-send-button")).toBeInViewport();
    });

    await test.step("More offers only what a player may use", async () => {
      await page.getByTestId("phone-row-more").click();
      await expect(sheet.getByTestId("phone-item-requests")).toHaveCount(0);
      await expect(sheet.getByTestId("phone-item-walls")).toHaveCount(0);
      await expect(sheet.getByTestId("phone-item-fullscreen")).toBeVisible();
    });

    await test.step("Back closes the sheet and keeps the board", async () => {
      await page.goBack();
      await expect(sheet).toHaveCount(0);
      await expect(page).toHaveURL(new RegExp(`/world/${WORLD_ID}/play`));
    });
  } finally {
    await page.context().close();
  }
});

test("a desktop or tablet has no phone board", async ({ browser, baseURL }) => {
  const { page } = await openDemo(browser, baseURL);
  try {
    for (const viewport of [
      { width: 1280, height: 800 },
      { width: 820, height: 1180 },
    ]) {
      await page.setViewportSize(viewport);
      await page.goto(`/demo/world/${WORLD_ID}/play`);
      await expect(page.getByTestId("world-dock")).toBeVisible({
        timeout: 60_000,
      });
      await expect(page.locator('[data-testid^="phone-"]')).toHaveCount(0);
    }
  } finally {
    await page.context().close();
  }
});
