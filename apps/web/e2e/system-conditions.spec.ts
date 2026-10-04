import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { rightClickBoard } from "./fixtures/boardPointer";
import { storeCounts } from "./fixtures/lightingProbe";
import { tokenPosition } from "./fixtures/offline";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";

/**
 * Conditions on the board (spec 067 Story 4).
 *
 * A system declares its conditions and a marker for each; a Game Master puts
 * a character under one from the right-click menu; every seat that is sent
 * the character's token has the marker drawn on it.
 *
 * That includes a creature the Game Master has not shown the players: its
 * token is on their board, nameless, and so is its marker — a condition goes
 * where the token goes, no further and no less. That someone who is sent no
 * token is sent no condition is the server's to prove
 * (`graphql/actor_conditions_tests.rs`).
 *
 * What is drawn is asked of each engine (`__engineProbe.tokenConditions`),
 * not inferred from the store: the marker is the claim.
 */

type Drawn = { id: string; glyph: string; color: string }[];

/** The markers this page's canvas draws on a token; `null` when it has none. */
async function markersOn(page: Page, tokenId: string): Promise<Drawn | null> {
  return page.evaluate((id) => {
    const probe = (
      window as unknown as {
        __engineProbe?: {
          tokenConditions?: () => { tokenId: string; conditions: Drawn }[];
        };
      }
    ).__engineProbe;
    const entry = probe?.tokenConditions?.().find((t) => t.tokenId === id);
    return entry ? entry.conditions : null;
  }, tokenId);
}

/** Open the Game Master's conditions dialog on a token. */
async function openConditions(page: Page, tokenId: string) {
  await expect(async () => {
    const at = await tokenPosition(page, tokenId);
    if (!at) throw new Error(`token ${tokenId} is not on this board`);
    await rightClickBoard(page, at);
    await expect(page.getByTestId("canvas-menu")).toBeVisible({
      timeout: 3_000,
    });
  }).toPass({ timeout: 30_000 });
  await page.getByTestId("canvas-menu-conditions").click();
  const dialog = page.getByTestId("canvas-menu-conditions-dialog");
  await expect(dialog).toBeVisible();
  return dialog;
}

test("a condition a Game Master applies is drawn for every seat that may see the token", async ({
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
    sceneName: "The Poisoned Well",
  });
  const [aria] = table.players;

  try {
    const goblin = await placeCast(table, {
      label: "Goblin",
      at: { x: 0, y: 0 },
      tokenType: "npc",
    });
    // A creature the players have not been shown: nameless on their board,
    // but on it.
    const lurker = await placeCast(table, {
      label: "Lurker",
      at: { x: 192, y: 0 },
      tokenType: "npc",
      visibleToPlayers: false,
    });

    await sitDown(table, table.gm);
    await sitDown(table, aria.page);
    for (const client of [table.gm, aria.page]) {
      await expect
        .poll(() => storeCounts(client), { timeout: 20_000 })
        .toMatchObject({ tokens: 2 });
    }

    const poisoned = [{ id: "poisoned", glyph: "dot", color: "danger" }];

    await test.step("the Game Master poisons the goblin, and both canvases draw the marker", async () => {
      const dialog = await openConditions(table.gm, goblin.tokenId);
      await expect(dialog.getByText("Poisoned", { exact: true })).toBeVisible();
      await expectNoAxeViolations(
        table.gm,
        '[data-testid="canvas-menu-conditions-dialog"]',
      );
      await dialog.getByTestId("canvas-menu-condition-poisoned").click();
      await expect(
        dialog.getByTestId("canvas-menu-condition-poisoned"),
      ).toBeChecked();
      await dialog.getByTestId("canvas-menu-conditions-done").click();
      await expect(dialog).toBeHidden();

      for (const [who, client] of [
        ["the Game Master", table.gm],
        ["Aria", aria.page],
      ] as const) {
        await expect
          .poll(() => markersOn(client, goblin.tokenId), {
            timeout: 20_000,
            message: `${who}'s canvas draws the goblin's poisoned marker`,
          })
          .toEqual(poisoned);
      }
    });

    await test.step("a player is offered no way to set a condition", async () => {
      const at = await tokenPosition(aria.page, goblin.tokenId);
      await rightClickBoard(aria.page, at!);
      await aria.page.waitForTimeout(1_500);
      await expect(aria.page.getByTestId("canvas-menu-conditions")).toHaveCount(
        0,
      );
      await aria.page.keyboard.press("Escape");
    });

    await test.step("a creature the players have not been shown carries its marker wherever its token is drawn", async () => {
      const dialog = await openConditions(table.gm, lurker.tokenId);
      await dialog.getByTestId("canvas-menu-condition-poisoned").click();
      await expect(
        dialog.getByTestId("canvas-menu-condition-poisoned"),
      ).toBeChecked();
      await dialog.getByTestId("canvas-menu-conditions-done").click();

      for (const client of [table.gm, aria.page]) {
        await expect
          .poll(() => markersOn(client, lurker.tokenId), { timeout: 20_000 })
          .toEqual(poisoned);
      }
    });

    await test.step("lifting it removes the marker from both canvases", async () => {
      const dialog = await openConditions(table.gm, goblin.tokenId);
      const box = dialog.getByTestId("canvas-menu-condition-poisoned");
      await expect(box).toBeChecked();
      await box.click();
      await expect(box).not.toBeChecked();
      await dialog.getByTestId("canvas-menu-conditions-done").click();

      for (const client of [table.gm, aria.page]) {
        await expect
          .poll(() => markersOn(client, goblin.tokenId), { timeout: 20_000 })
          .toBeNull();
      }
    });
  } finally {
    await closeTable(table);
  }
});
