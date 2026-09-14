import { expect, test, type Browser, type Page } from "@playwright/test";
import { expectNoAxeViolations } from "../fixtures/axe";
import { openDockTab, uniqueSuffix } from "../fixtures/helpers";
import {
  createToken,
  dragToken,
  tokenPosition,
  waitForEngineReady,
  waitForTokenTrafficToSettle,
} from "../fixtures/offline";
import {
  enterPlayFromTheWorld,
  pausedOnServer,
  pauseThroughThePortal,
  readRows,
  seatATable,
  signInTheOperator,
  uuid,
  type JourneyTable,
} from "../fixtures/journeyTable";
import { markDocument } from "../fixtures/playPause";

/**
 * Spec 051 T071, US4 as people live it: an operator lifts a pause, and the
 * table picks up exactly where it stopped.
 *
 * # What is recorded before the pause
 *
 * The things a table would notice had changed: where the tokens stand, which
 * scene is up, what the player's map remembers (spec 045's explored areas, in
 * the player's own browser, and the epoch the server holds for them), and
 * whose turn it is in the fight. Rows are read with `updated_at`, so a write
 * that happened to store the same value still counts as a change.
 *
 * # What the lift is held to
 *
 * - blank or whitespace grounds cannot be submitted;
 * - two operator tabs lifting at once: one lifts, the other is told who did
 *   and when, as a status and not as an error (FR-040);
 * - the notice offers *Return to the world* within 30 s, with no reload, and
 *   it takes the table back to play (FR-041);
 * - everything recorded is identical afterwards;
 * - a scene taken down while the world was paused is still withheld, and the
 *   Game Master is told the way spec 015 tells an owner: the scene is gone
 *   from their Scenes page, and their standing page lists the strike with a
 *   counter-notice to file (FR-042).
 *
 * # UI only
 *
 * Every change is made through a page. `psql` reads the server's rows, and a
 * page's own IndexedDB is read to see the player's map. The one value typed
 * that a person would copy from elsewhere is the player's account id in Token
 * Management's "Owner user ID" field, which is what that field asks for.
 */

/** Long enough for a heartbeat (5 s) and a stream's liveness tick (5 s). */
const SETTLE_MS = 6_000;

/** The notice polls every 30 s (`PLAY_STATE_POLL_MS`); this is FR-041's bound. */
const RETURN_WITHIN_MS = 30_000;

interface Recorded {
  activeScene: string[];
  tokens: string[];
  combat: string[];
  exploration: string[];
  playerCells: string[];
  playerEpoch: number | null;
}

/** The player's remembered map for one scene, from their own browser. */
async function playerMap(
  page: Page,
  worldId: string,
  sceneId: string,
): Promise<{ cells: string[]; epoch: number | null }> {
  return page.evaluate(
    async ({ world, scene }) => {
      const db = await new Promise<IDBDatabase | null>((resolve) => {
        const request = indexedDB.open("thunderforge-exploration", 1);
        request.onupgradeneeded = () => {
          if (!request.result.objectStoreNames.contains("areas")) {
            request.result.createObjectStore("areas");
          }
        };
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => resolve(null);
      });
      if (!db) return { cells: [], epoch: null };
      return new Promise<{ cells: string[]; epoch: number | null }>(
        (resolve) => {
          const tx = db.transaction("areas", "readonly");
          const store = tx.objectStore("areas");
          const keys = store.getAllKeys();
          const values = store.getAll();
          tx.oncomplete = () => {
            const index = (keys.result as string[]).findIndex((key) =>
              key.endsWith(`:${world}:${scene}`),
            );
            db.close();
            if (index < 0) {
              resolve({ cells: [], epoch: null });
              return;
            }
            const value = (
              values.result as { cells: [number, number][]; epoch: number }[]
            )[index];
            resolve({
              cells: value.cells.map(([q, r]) => `${q},${r}`).sort(),
              epoch: value.epoch,
            });
          };
          tx.onerror = () => resolve({ cells: [], epoch: null });
        },
      );
    },
    { world: worldId, scene: sceneId },
  );
}

async function record(table: JourneyTable, sceneId: string): Promise<Recorded> {
  const map = await playerMap(table.playerPage, table.worldId, sceneId);
  return {
    activeScene: readRows(
      `select coalesce(active_scene_id::text, '<none>') from worlds where id = '${uuid(table.worldId)}'`,
    ),
    tokens: readRows(
      `select token_id, x, y, coalesce(owner_user_id::text, '<none>'), is_primary, updated_at from tokens where scene_id = '${uuid(sceneId)}' order by token_id`,
    ),
    combat: readRows(
      `select id, round, coalesce(active_combatant_id::text, '<none>'), updated_at from world_combats where world_id = '${uuid(table.worldId)}' and ended_at is null`,
    ),
    exploration: readRows(
      `select exploration_enabled, exploration_epoch, updated_at, (select count(*) from scene_exploration_resets r where r.scene_id = s.scene_id) from scenes s where scene_id = '${uuid(sceneId)}'`,
    ),
    playerCells: map.cells,
    playerEpoch: map.epoch,
  };
}

/** The lift dialog for `worldId`, opened from its row under *Active pauses*. */
async function openLift(operator: Page, worldId: string, worldName: string) {
  const row = operator.locator(
    `[data-testid="play-pause-active"][data-world-id="${worldId}"]`,
  );
  await expect(row).toBeVisible({ timeout: 20_000 });
  await row
    .getByRole("button", { name: `Lift the pause on ${worldName}` })
    .click();
  const dialog = operator.getByRole("dialog");
  await expect(dialog).toBeVisible();
  return dialog;
}

async function openPortal(operator: Page): Promise<void> {
  await operator.goto("/admin");
  await operator.getByTestId("admin-nav-play-pauses").click();
  await operator.waitForURL(/\/admin\/play-pauses$/);
  await expect(
    operator.getByRole("heading", { name: "Play pauses", level: 1 }),
  ).toBeVisible({ timeout: 20_000 });
}

/** A signed-out claimant files a complete notice on a scene. */
async function fileSceneNotice(
  browser: Browser,
  sceneId: string,
  sceneName: string,
): Promise<string> {
  const context = await browser.newContext();
  const claimant = await context.newPage();
  try {
    await claimant.goto("/legal/dmca");
    await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible({
      timeout: 20_000,
    });
    await claimant.getByLabel("Content type").click();
    await claimant.getByRole("option", { name: "Scene" }).click();
    await claimant.locator("#dmca-entity-id").fill(sceneId);
    await claimant.locator("#dmca-claimant-name").fill("Cartography Guild");
    await claimant
      .locator("#dmca-claimant-contact")
      .fill("notices@cartography.example.test");
    await claimant
      .locator("#dmca-work-description")
      .fill("An original battle map, published and registered.");
    await claimant
      .locator("#dmca-infringing-location")
      .fill(`The scene "${sceneName}", in a ThunderForge world.`);
    await claimant.locator("#dmca-good-faith").click();
    await claimant.locator("#dmca-accuracy").click();
    await claimant.locator("#dmca-signature").fill("Cartography Guild");
    await claimant.getByTestId("takedown-notice-submit").click();
    const accepted = claimant.getByTestId("takedown-notice-accepted");
    await expect(accepted).toBeVisible({ timeout: 20_000 });
    return (await accepted.locator("code").innerText()).trim();
  } finally {
    await context.close();
  }
}

test.describe("spec 051 US4 journey: play resumes as it was", () => {
  test("an operator lifts a pause; the table returns without reloading to exactly what it left, and a scene taken down meanwhile stays withheld", async ({
    browser,
  }) => {
    test.setTimeout(720_000);
    const table = await seatATable(browser, "Resumes");
    const gm = table.gmPage;
    const player = table.playerPage;
    const operator = await signInTheOperator(browser);
    // The operator's session carried into a second browser, rather than a
    // second sign-in: two second-factor sign-ins inside one TOTP step spend
    // the step (found by the T043 race). The race is still two browsers.
    const secondOperator = await (
      await browser.newContext({
        storageState: await operator.context().storageState(),
      })
    ).newPage();
    const gmTab = await gm.context().newPage();

    try {
      const sceneId = readRows(
        `select active_scene_id from worlds where id = '${uuid(table.worldId)}'`,
      )[0];
      expect(sceneId, "the world has a scene up").toMatch(/^[0-9a-f-]{36}$/);
      const playerId = readRows(
        `select id from users where username = '${table.player.username.replace(/'/g, "''")}'`,
      )[0];
      let movedTokenId = "";
      let playerTokenId = "";
      const sideName = `Side Vault ${uniqueSuffix()}`;
      let sideSceneId = "";

      await test.step("the Game Master makes a second scene in the Scenes section", async () => {
        await gmTab.goto(`/world/${table.worldId}/staging`);
        await gmTab.getByTestId("world-nav-scenes").click();
        await gmTab.waitForURL(new RegExp(`/world/${table.worldId}/scenes$`), {
          timeout: 15_000,
        });
        await gmTab.getByTestId("new-scene-name-input").fill(sideName);
        await gmTab.getByTestId("add-scene-button").click();
        await gmTab.getByRole("link", { name: sideName }).click({
          timeout: 15_000,
        });
        await gmTab.waitForURL(
          new RegExp(`/world/${table.worldId}/scenes/[^/]+$`),
          { timeout: 15_000 },
        );
        sideSceneId = /\/scenes\/([^/]+)$/.exec(
          new URL(gmTab.url()).pathname,
        )![1];
      });

      await test.step("the Game Master puts the player's token down and moves another", async () => {
        await waitForEngineReady(gm);
        // The token to move first, while it is the only one: two tokens
        // made in the same spot stack, and a press grabs whichever is on top.
        movedTokenId = await createToken(gm);
        await waitForTokenTrafficToSettle(gm);
        expect(await dragToken(gm, movedTokenId, { dx: 192, dy: 96 })).toBe(
          true,
        );
        await waitForTokenTrafficToSettle(gm);
        playerTokenId = await createToken(gm);
        await gm
          .getByTestId("token-panel-toggle-button")
          .click({ force: true });
        const item = gm.getByTestId(`token-list-item-${playerTokenId}`);
        await expect(item).toBeVisible({ timeout: 10_000 });
        await item.click({ force: true });
        const owner = gm.getByTestId(`token-owner-input-${playerTokenId}`);
        await expect(owner).toBeVisible({ timeout: 10_000 });
        await owner.fill(playerId);
        // Tab, not blur: see `assignTokenOwnership` in token-authoring.spec.ts.
        await owner.press("Tab");
        const primary = gm.getByTestId(
          `token-primary-checkbox-${playerTokenId}`,
        );
        await expect(primary).toBeEnabled({ timeout: 10_000 });
        await primary.check({ force: true });
        await expect
          .poll(
            () =>
              readRows(
                `select owner_user_id, is_primary from tokens where token_id = '${uuid(playerTokenId)}'`,
              )[0],
            { timeout: 15_000 },
          )
          .toBe(`${playerId}|t`);
        await gm.keyboard.press("Escape");
        await gm.keyboard.press("Escape");
      });

      await test.step("the Game Master has the scene remember what players explore", async () => {
        await gm.getByTestId("gm-tool-lights").click();
        const toggle = gm.getByTestId("scene-exploration-toggle");
        await expect(toggle).toBeVisible({ timeout: 10_000 });
        await toggle.click();
        await expect(toggle).toHaveAttribute("aria-pressed", "true");
        await expect
          .poll(
            () =>
              readRows(
                `select exploration_enabled from scenes where scene_id = '${uuid(sceneId)}'`,
              )[0],
            { timeout: 15_000 },
          )
          .toBe("t");
        await gm.getByTestId("gm-tool-lights").click();
      });

      await test.step("the player comes back to the table, and their map fills in", async () => {
        // A person arriving at a scene that now remembers: back through the
        // world's own Play button.
        await enterPlayFromTheWorld(player, table.worldId);
        await waitForEngineReady(player);
        await expect
          .poll(
            async () =>
              (await playerMap(player, table.worldId, sceneId)).cells.length,
            {
              timeout: 60_000,
              intervals: [2_000],
              message: "the player's browser keeps what their token has seen",
            },
          )
          .toBeGreaterThan(0);
      });

      await test.step("the Game Master starts a fight and takes the first turn", async () => {
        await openDockTab(gm, "combat");
        await expect(gm.getByTestId("combat-panel")).toBeVisible({
          timeout: 20_000,
        });
        await gm.getByTestId("start-combat-button").click();
        await expect(gm.getByTestId("advance-turn-button")).toBeVisible({
          timeout: 20_000,
        });
        await gm
          .getByTestId("combat-add-actor-select")
          .selectOption({ label: table.characterName });
        await gm.getByTestId("combat-add-button").click();
        const rows = gm.getByTestId("combatant-row");
        await expect(rows).toHaveCount(1, { timeout: 20_000 });
        await gm.getByTestId("advance-turn-button").click();
        await expect(rows.nth(0)).toHaveAttribute("data-active-turn", "true", {
          timeout: 20_000,
        });
      });

      // Let the exploration save timer (10 s) and token traffic settle, so
      // what is recorded is the table at rest.
      await gm.waitForTimeout(12_000);
      const before = await record(table, sceneId);
      const playerTokenBefore = await tokenPosition(player, playerTokenId);
      const movedTokenBefore = await tokenPosition(gm, movedTokenId);
      expect(before.tokens).toHaveLength(2);
      expect(before.combat).toHaveLength(1);
      expect(before.exploration[0]).toMatch(/^t\|/);
      expect(before.playerCells.length).toBeGreaterThan(0);
      expect(movedTokenBefore).not.toBeNull();

      const grounds = `Journey pause ${uniqueSuffix()}: holding while we look`;
      await test.step("the operator pauses the world, and the table lands on the notice", async () => {
        await pauseThroughThePortal(
          operator,
          table.worldId,
          table.worldName,
          grounds,
        );
        for (const page of [gm, player]) {
          await page.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
          await expect(
            page.getByRole("heading", { name: "Play is paused", level: 1 }),
          ).toBeVisible({ timeout: 15_000 });
          await markDocument(page);
        }
      });

      let caseId = "";
      await test.step("while paused, a claimant takes down the second scene", async () => {
        caseId = await fileSceneNotice(browser, sideSceneId, sideName);
        expect(caseId).toMatch(/^[0-9a-f-]{36}$/);
        expect(pausedOnServer(table.worldId)).toBe(true);
      });

      const liftGrounds = [
        `Journey lift ${uniqueSuffix()}: settled`,
        `Journey lift ${uniqueSuffix()}: also settled`,
      ];
      const dialogs =
        await test.step("two operator tabs open the lift, and blank grounds are refused", async () => {
          await openPortal(operator);
          await openPortal(secondOperator);
          const opened = [];
          for (const [index, tab] of [operator, secondOperator].entries()) {
            const dialog = await openLift(tab, table.worldId, table.worldName);
            await expect(dialog.getByRole("heading")).toHaveText(
              `Lift the pause on ${table.worldName}?`,
            );
            const field = dialog.getByRole("textbox", { name: "Grounds" });
            const submit = dialog.getByRole("button", { name: "Lift pause" });
            await expect(field).toHaveAttribute("aria-required", "true");
            await expect(submit, "blank grounds").toBeDisabled();
            await field.fill("  \n  ");
            await expect(submit, "whitespace grounds").toBeDisabled();
            // Enter in the field is a new line, not a way round the button.
            await field.press("Enter");
            await expect(dialog).toBeVisible();
            expect(pausedOnServer(table.worldId)).toBe(true);
            await field.fill(liftGrounds[index]);
            await expect(submit).toBeEnabled();
            opened.push(dialog);
          }
          await expectNoAxeViolations(operator, '[role="dialog"]');
          return opened;
        });

      const liftedAt =
        await test.step("both confirm at once: one lifts, the other is told who did", async () => {
          await Promise.all(
            dialogs.map((dialog) =>
              dialog.getByRole("button", { name: "Lift pause" }).click(),
            ),
          );
          const at = Date.now();
          for (const dialog of dialogs) {
            await expect(dialog).toBeHidden({ timeout: 15_000 });
          }
          const texts = await Promise.all(
            [operator, secondOperator].map(async (tab) => {
              const outcome = tab.getByTestId("play-pause-outcome");
              await expect(outcome).toBeVisible();
              return (await outcome.innerText()).trim();
            }),
          );
          const lifted = `Play in ${table.worldName} is no longer paused.`;
          const winners = texts.filter((text) => text === lifted);
          const losers = texts.filter((text) =>
            text.startsWith("Already lifted by "),
          );
          expect(winners, JSON.stringify(texts)).toHaveLength(1);
          expect(losers, JSON.stringify(texts)).toHaveLength(1);
          const liftedBy = readRows(
            `select u.username from world_play_pauses p join users u on u.id = p.lifted_by where p.world_id = '${uuid(table.worldId)}'`,
          );
          expect(liftedBy).toHaveLength(1);
          expect(losers[0]).toMatch(
            new RegExp(
              `^Already lifted by ${liftedBy[0]} at .*\\d.*\\. Play in ${table.worldName} is no longer paused\\.$`,
            ),
          );
          const loser = texts[0] === losers[0] ? operator : secondOperator;
          const outcome = loser.getByTestId("play-pause-outcome");
          await expect(outcome).toHaveAttribute("role", "status");
          await expect(outcome).toBeFocused();
          await expect(loser.getByRole("alert")).toHaveCount(0);
          expect(pausedOnServer(table.worldId)).toBe(false);
          // The record keeps one lift, with the winner's grounds.
          const kept = readRows(
            `select lift_grounds from world_play_pauses where world_id = '${uuid(table.worldId)}'`,
          );
          expect(liftGrounds).toContain(kept[0]);
          return at;
        });

      await test.step("the notice offers Return to the world within 30 s, without a reload", async () => {
        for (const page of [gm, player]) {
          const back = page.getByRole("link", { name: "Return to the world" });
          await expect(back).toBeVisible({
            timeout: Math.max(
              1_000,
              RETURN_WITHIN_MS - (Date.now() - liftedAt),
            ),
          });
        }
        for (const page of [gm, player]) {
          expect(
            await page.evaluate(
              () =>
                (window as unknown as { __e2eSameDocument?: boolean })
                  .__e2eSameDocument === true,
            ),
            "the notice must not have reloaded",
          ).toBe(true);
          await page.getByRole("link", { name: "Return to the world" }).click();
          await expect(page).toHaveURL(
            new RegExp(`/world/${table.worldId}/play$`),
            { timeout: 15_000 },
          );
        }
        for (const page of [gm, player]) {
          await expect(page.locator("canvas:visible")).toHaveCount(1, {
            timeout: 90_000,
          });
        }
      });

      await test.step("everything recorded is as it was", async () => {
        await waitForEngineReady(gm);
        await waitForEngineReady(player);
        await waitForTokenTrafficToSettle(gm);
        // Past the exploration save timer, so a reset would have shown.
        await gm.waitForTimeout(12_000);
        const after = await record(table, sceneId);
        expect(after.activeScene, "the same scene is up").toEqual(
          before.activeScene,
        );
        expect(after.tokens, "positions, owners and updated_at").toEqual(
          before.tokens,
        );
        expect(after.combat, "round, whose turn, and updated_at").toEqual(
          before.combat,
        );
        expect(after.exploration, "exploration and its epoch").toEqual(
          before.exploration,
        );
        expect(after.playerEpoch, "the player's map was not reset").toBe(
          before.playerEpoch,
        );
        const lost = before.playerCells.filter(
          (cell) => !after.playerCells.includes(cell),
        );
        expect(
          lost,
          "every cell the player had explored is still there",
        ).toEqual([]);
        await expect
          .poll(() => tokenPosition(gm, movedTokenId), { timeout: 20_000 })
          .toEqual(movedTokenBefore);
        await expect
          .poll(() => tokenPosition(player, playerTokenId), { timeout: 20_000 })
          .toEqual(playerTokenBefore);
        await openDockTab(gm, "combat");
        const rows = gm.getByTestId("combatant-row");
        await expect(rows).toHaveCount(1, { timeout: 20_000 });
        await expect(rows.nth(0)).toHaveAttribute("data-active-turn", "true");
        await expect(rows.nth(0)).toContainText(table.characterName);
      });

      await test.step("the scene taken down while paused is still withheld, and the Game Master is told as spec 015 tells them", async () => {
        await gmTab.goto(`/world/${table.worldId}/scenes`);
        await expect(
          gmTab.getByRole("link", { name: "Starting Scene" }),
        ).toBeVisible({ timeout: 20_000 });
        await expect(gmTab.getByRole("link", { name: sideName })).toHaveCount(
          0,
        );
        await expect(gmTab.locator("body")).not.toContainText(sideName);

        await gmTab.goto("/settings/standing");
        const strike = gmTab
          .getByTestId("standing-strike")
          .filter({ hasText: caseId });
        await expect(strike).toContainText("A scene", { timeout: 20_000 });
        await expect(
          strike.getByRole("button", { name: "File a counter-notice" }),
        ).toBeVisible();
        expect(
          readRows(
            `select count(*) from world_play_pauses where world_id = '${uuid(table.worldId)}' and lifted_at is null`,
          ),
        ).toEqual(["0"]);
      });

      await test.step("and play goes on: the table is still playing a few seconds later", async () => {
        await gm.waitForTimeout(SETTLE_MS);
        for (const page of [gm, player]) {
          await expect(page).toHaveURL(
            new RegExp(`/world/${table.worldId}/play$`),
          );
          await expect(page.getByTestId("play-paused-notice")).toHaveCount(0);
        }
      });
    } finally {
      await operator.context().close();
      await secondOperator.context().close();
      for (const context of table.contexts) await context.close();
    }
  });
});
