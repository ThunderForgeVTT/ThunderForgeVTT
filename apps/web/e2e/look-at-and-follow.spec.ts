import { expect, test, type Page } from "./fixtures/test";
import { expectNoAxeViolations } from "./fixtures/axe";
import { openDockTab } from "./fixtures/helpers";
import { serverTokenPosition } from "./fixtures/offline";
import { closeTable, openTable, placeCast, sitDown } from "../playtest/table";
import {
  addCombatant,
  advanceTurn,
  openCombatPanel,
  startCombat,
} from "../playtest/combat";

/**
 * The owner, 2026-09-15: WASD moves a token; a little target on every creature
 * in the actors panel and the combat tracker scrolls the viewport to it; and,
 * as a per-person toggle, the camera follows the turn, zoomed out a little.
 *
 * Every camera assertion asks the engine where the camera is and where the
 * creature is drawn (`__engineProbe`), rather than reading pixels: the camera
 * is the engine's, and so is the answer.
 */

const HIDDEN_NAME = "Vorlaine the Unseen";

type Camera = { x: number; y: number; scale: number };

async function camera(page: Page): Promise<Camera> {
  return page.evaluate(() =>
    (
      window as unknown as {
        __engineProbe: { camera: () => Camera };
      }
    ).__engineProbe.camera(),
  );
}

/** Where this board draws a token, in world units. */
async function drawnAt(
  page: Page,
  tokenId: string,
): Promise<{ x: number; y: number } | null> {
  return page.evaluate((id) => {
    const probe = (
      window as unknown as {
        __engineProbe: {
          tokenFootprints: () => { tokenId: string; x: number; y: number }[];
        };
      }
    ).__engineProbe;
    const found = probe.tokenFootprints().find((row) => row.tokenId === id);
    return found ? { x: found.x, y: found.y } : null;
  }, tokenId);
}

async function focusOutcome(page: Page): Promise<string | undefined> {
  return page.evaluate(
    () =>
      (
        window as unknown as {
          __engineProbe: { focusState: () => { outcome?: string } };
        }
      ).__engineProbe.focusState().outcome,
  );
}

/** How far the camera's centre is from where the token is drawn. */
async function distanceToToken(page: Page, tokenId: string): Promise<number> {
  const [cam, at] = await Promise.all([camera(page), drawnAt(page, tokenId)]);
  if (!at) return Number.POSITIVE_INFINITY;
  return Math.hypot(cam.x - at.x, cam.y - at.y);
}

test("WASD walks a token, a target looks at a creature, and the camera can follow the turn", async ({
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
    sceneName: "The Long Hall",
  });
  const [aria] = table.players;

  try {
    const hero = await placeCast(table, {
      label: "Aria",
      at: { x: 0, y: 0 },
      seat: aria,
    });
    // Far enough apart that "the camera is on it" cannot be true of two of
    // them at once.
    const boblin = await placeCast(table, {
      label: "Boblin",
      at: { x: 960, y: 0 },
      tokenType: "npc",
    });
    const hidden = await placeCast(table, {
      label: HIDDEN_NAME,
      at: { x: 0, y: 960 },
      tokenType: "npc",
      visibleToPlayers: false,
    });

    const combat = await startCombat(table);
    await addCombatant(table, combat.id, {
      label: "Aria",
      actorId: hero.actorId,
      tokenId: hero.tokenId,
      initiative: 20,
    });
    await addCombatant(table, combat.id, {
      label: "Boblin",
      actorId: boblin.actorId,
      tokenId: boblin.tokenId,
      initiative: 15,
      isNpc: true,
    });
    await addCombatant(table, combat.id, {
      label: HIDDEN_NAME,
      actorId: hidden.actorId,
      tokenId: hidden.tokenId,
      initiative: 10,
      isNpc: true,
    });

    await sitDown(table, table.gm);
    await sitDown(table, aria.page);
    for (const client of [table.gm, aria.page]) {
      await expect
        .poll(() => drawnAt(client, boblin.tokenId), { timeout: 30_000 })
        .not.toBeNull();
    }

    await test.step("W, A, S and D each walk the player's token, and typing in a field walks nothing", async () => {
      const at = async () =>
        (await serverTokenPosition(table.gm, table.sceneId, hero.tokenId))!;

      // Each key is judged against where the token stood just before it. The
      // token is placed on a raw point, and its first step lands it on a cell
      // centre, so "back where it started" is not a position it can return to.
      const steps: {
        key: string;
        axis: "x" | "y";
        sign: 1 | -1;
        name: string;
      }[] = [
        { key: "w", axis: "y", sign: 1, name: "W walks north" },
        { key: "d", axis: "x", sign: 1, name: "D walks east" },
        { key: "s", axis: "y", sign: -1, name: "S walks south" },
        { key: "a", axis: "x", sign: -1, name: "A walks west" },
      ];
      for (const step of steps) {
        const before = await at();
        await aria.page.keyboard.press(step.key);
        await expect
          .poll(
            async () =>
              ((await at())[step.axis] - before[step.axis]) * step.sign,
            {
              timeout: 20_000,
              message: step.name,
            },
          )
          .toBeGreaterThan(0);
      }
      const start = await at();

      // The same letters, typed into the actor search: a chat message or a
      // search must never walk a token across the map.
      await openDockTab(aria.page, "actors");
      const search = aria.page.getByTestId("actor-search-input");
      await search.click();
      await search.pressSequentially("wasdddd");
      await expect(search).toHaveValue("wasdddd");
      await aria.page.waitForTimeout(1_500);
      expect(
        await serverTokenPosition(table.gm, table.sceneId, hero.tokenId),
        "letters typed into a field move nothing",
      ).toEqual(start);
      await search.fill("");
    });

    await test.step("the Game Master looks at Boblin from the actors panel, by keyboard", async () => {
      await openDockTab(table.gm, "actors");
      const target = table.gm.getByRole("button", { name: "Look at Boblin" });
      await expect(target).toBeVisible({ timeout: 20_000 });
      await expectNoAxeViolations(table.gm, '[data-testid="actors-panel"]');

      await target.focus();
      await table.gm.keyboard.press("Enter");
      await expect
        .poll(() => distanceToToken(table.gm, boblin.tokenId), {
          timeout: 10_000,
          message: "the Game Master's camera goes to Boblin",
        })
        .toBeLessThan(2);

      // A Game Master may look at anything, a hidden NPC included.
      await table.gm
        .getByRole("button", { name: `Look at ${HIDDEN_NAME}` })
        .click();
      await expect
        .poll(() => distanceToToken(table.gm, hidden.tokenId), {
          timeout: 10_000,
        })
        .toBeLessThan(2);
    });

    await test.step("the player looks at Boblin from the combat tracker", async () => {
      await openCombatPanel(aria.page);
      const target = aria.page
        .getByTestId("combat-panel")
        .getByRole("button", { name: "Look at Boblin" });
      await expect(target).toBeVisible({ timeout: 20_000 });
      await expectNoAxeViolations(aria.page, '[data-testid="combat-panel"]');

      await target.click();
      await expect
        .poll(() => distanceToToken(aria.page, boblin.tokenId), {
          timeout: 10_000,
          message: "the player's camera goes to Boblin",
        })
        .toBeLessThan(2);
      expect(await focusOutcome(aria.page)).toBe("moved");
    });

    await test.step("a player is offered no target on a creature they may not know", async () => {
      // In the tracker the hidden NPC is "Unknown", and carries no target.
      await expect(aria.page.getByTestId("combatant-list")).toContainText(
        "Unknown",
      );
      await expect(
        aria.page.getByTestId(`combatant-look-at-${hidden.tokenId}`),
      ).toHaveCount(0);
      await expect(
        aria.page.getByRole("button", { name: /Look at Unknown/ }),
      ).toHaveCount(0);

      // And the actors panel does not list it at all.
      await openDockTab(aria.page, "actors");
      await expect(aria.page.getByTestId("actors-panel")).not.toContainText(
        HIDDEN_NAME,
      );
      await expect(
        aria.page.getByTestId(`actor-look-at-${hidden.tokenId}`),
      ).toHaveCount(0);

      // The Game Master's own tracker does offer it.
      await openCombatPanel(table.gm);
      await expect(
        table.gm.getByTestId(`combatant-look-at-${hidden.tokenId}`),
      ).toHaveCount(1);
    });

    await test.step("with Follow the turn on, a turn change takes the camera to whoever is up, zoomed out", async () => {
      await openCombatPanel(aria.page);
      const follow = aria.page.getByTestId("follow-the-turn");
      await expect(follow, "following starts off").not.toBeChecked();
      await follow.check();

      // Look somewhere else first, so arriving at the active creature is a
      // move this step caused.
      await aria.page
        .getByTestId("combat-panel")
        .getByRole("button", { name: "Look at Aria" })
        .click();
      await expect
        .poll(() => distanceToToken(aria.page, hero.tokenId), {
          timeout: 10_000,
        })
        .toBeLessThan(2);
      const before = await camera(aria.page);

      let turn = await advanceTurn(table, combat.id);
      // Whoever the server made active, the camera goes there — unless that
      // is the creature already on screen, in which case advance once more.
      let active = turn.combatants.find(
        (row) => row.id === turn.activeCombatantId,
      );
      if (active?.tokenId === hero.tokenId || !active?.tokenId) {
        turn = await advanceTurn(table, combat.id);
        active = turn.combatants.find(
          (row) => row.id === turn.activeCombatantId,
        );
      }
      expect(active?.tokenId, "a creature with a token is up").toBeTruthy();
      // A player may be taken only to a creature they may look at.
      const expected =
        active!.tokenId === hidden.tokenId ? null : active!.tokenId!;

      if (expected) {
        await expect
          .poll(() => distanceToToken(aria.page, expected), {
            timeout: 15_000,
            message: "the camera follows the turn to the creature that is up",
          })
          .toBeLessThan(2);
        await expect
          .poll(async () => (await camera(aria.page)).scale, {
            timeout: 5_000,
            message: "and frames its surroundings, not only the creature",
          })
          .not.toBeCloseTo(before.scale, 3);
      } else {
        await aria.page.waitForTimeout(1_500);
        expect(await focusOutcome(aria.page)).toBe("unnamed");
      }

      // Remembered across a reload.
      await aria.page.reload();
      await openCombatPanel(aria.page);
      await expect(aria.page.getByTestId("follow-the-turn")).toBeChecked();
    });

    await test.step("with it off, a turn change leaves the camera where it is", async () => {
      const follow = aria.page.getByTestId("follow-the-turn");
      await follow.uncheck();
      await expect
        .poll(() => drawnAt(aria.page, hero.tokenId), { timeout: 30_000 })
        .not.toBeNull();
      await aria.page
        .getByTestId("combat-panel")
        .getByRole("button", { name: "Look at Aria" })
        .click();
      await expect
        .poll(() => distanceToToken(aria.page, hero.tokenId), {
          timeout: 10_000,
        })
        .toBeLessThan(2);
      const held = await camera(aria.page);

      await advanceTurn(table, combat.id);
      await advanceTurn(table, combat.id);
      await aria.page.waitForTimeout(2_000);

      const after = await camera(aria.page);
      expect(
        Math.hypot(after.x - held.x, after.y - held.y),
        "the camera stays put while following is off",
      ).toBeLessThan(1);
      expect(after.scale).toBeCloseTo(held.scale, 3);
    });
  } finally {
    await closeTable(table);
  }
});
