import { expect, test, type Browser, type Page } from "@playwright/test";
import { uniqueSuffix } from "../fixtures/helpers";
import {
  expectOnlyTheNotice,
  seatATable,
  signInTheOperator,
} from "../fixtures/journeyTable";
import {
  createToken,
  dragToken,
  tokenPosition,
  waitForEngineReady,
  waitForTokenTrafficToSettle,
} from "../fixtures/offline";
import { expectPausedNotice, markDocument } from "../fixtures/playPause";
import { switchToScene } from "../fixtures/world-cache";

/**
 * Spec 051 T073, the capstone: the whole story of a pause, once, in order,
 * the way it happens to people.
 *
 *  1. A table plays two scenes.
 *  2. A DMCA notice is filed through the form, against the second.
 *  3. The operator reviews the request it raised and approves it.
 *  4. The table sees the notice.
 *  5. The player tries a refresh and a new tab, and gets the notice both times.
 *  6. The operator lifts the pause.
 *  7. The table returns, and play goes on: a token the Game Master moves
 *     reaches the player's browser.
 *  8. The Game Master reads the history: times, and nothing else.
 *  9. The operator reads the record: the takedown that prompted the pause,
 *     the approval's note as its grounds, and the lift.
 *
 * # Deliberately sequential, and with no GraphQL shortcuts
 *
 * Every other spec 051 test proves one of these in isolation, several with a
 * request made straight to the server to set up or to check. This one never
 * calls GraphQL: each step starts from what the step before left on screen.
 * It fails when the parts do not join, which no isolated test can.
 *
 * The notice is filed against the scene the table played second, not the one
 * it is on, so that after the lift the table returns to a scene that was never
 * taken down, and play resuming is not confused with content being withheld.
 */

/** The portal re-reads every 5 s; a round trip on top. */
const PORTAL_REFRESH_BUDGET_MS = 15_000;

/** The notice polls every 30 s (`PLAY_STATE_POLL_MS`); FR-041's bound. */
const RETURN_WITHIN_MS = 30_000;

/** A signed-out claimant files a complete notice against a scene. */
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

async function openPortal(operator: Page): Promise<void> {
  await operator.goto("/admin");
  await operator.getByTestId("admin-nav-play-pauses").click();
  await operator.waitForURL(/\/admin\/play-pauses$/);
  await expect(
    operator.getByRole("heading", { name: "Play pauses", level: 1 }),
  ).toBeVisible({ timeout: 20_000 });
}

test.describe("spec 051 capstone journey: the whole story", () => {
  test("a table plays; a notice becomes a pause; the table is held, then returns and plays on; both sides read what happened", async ({
    browser,
  }) => {
    test.setTimeout(900_000);
    const table = await seatATable(browser, "Whole Story");
    const gm = table.gmPage;
    const player = table.playerPage;
    const gmTab = await gm.context().newPage();
    const operator = await signInTheOperator(browser);
    const sideName = `Map Room ${uniqueSuffix()}`;
    let sideSceneId = "";

    try {
      await test.step("1. the table plays two scenes", async () => {
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
        await gmTab.close();

        // Opened afresh, because the playfield's scene switcher does not hear
        // of a scene added in another tab (found writing this journey; not
        // spec 051's to fix). A navigation rather than a reload, so step 4
        // can still say the pause arrived without one.
        await gm.goto(`/world/${table.worldId}/play`);
        await expect(gm.locator("canvas:visible")).toHaveCount(1, {
          timeout: 90_000,
        });
        await waitForEngineReady(gm);
        await switchToScene(gm, sideName);
        await gm.waitForTimeout(3_000);
        await switchToScene(gm, "Starting Scene");
        await expect(gm).toHaveURL(new RegExp(`/world/${table.worldId}/play$`));
        await expect(player).toHaveURL(
          new RegExp(`/world/${table.worldId}/play$`),
        );
      });

      await openPortal(operator);
      await markDocument(operator);
      await markDocument(gm);
      await markDocument(player);

      let caseId = "";
      await test.step("2. a claimant files a notice against the second scene", async () => {
        caseId = await fileSceneNotice(browser, sideSceneId, sideName);
        expect(caseId).not.toBe("");
      });

      const note = `Capstone approval ${uniqueSuffix()}: the guild's map`;
      await test.step("3. the operator reviews the request and approves it", async () => {
        const row = operator.locator(
          `[data-testid="play-pause-request"][data-world-id="${table.worldId}"]`,
        );
        await expect(row).toContainText(table.worldName, {
          timeout: PORTAL_REFRESH_BUDGET_MS,
        });
        await expect(
          row.getByTestId("play-pause-request-played-now"),
        ).toHaveText("Being played");
        await expect(
          row.getByTestId("play-pause-request-trigger"),
        ).toContainText("Takedown on a scene");
        await row.getByRole("button", { name: /^Approve / }).click();
        const dialog = operator.getByRole("dialog");
        await expect(dialog).toBeVisible();
        await dialog.getByRole("textbox", { name: "Note" }).fill(note);
        await dialog.getByTestId("play-pause-decide-submit").click();
        await expect(dialog).toBeHidden({ timeout: 15_000 });
        await expect(operator.getByTestId("play-pause-outcome")).toHaveText(
          `Play in ${table.worldName} is paused.`,
        );
      });

      await test.step("4. the table sees the notice, without reloading", async () => {
        for (const page of [gm, player]) {
          await page.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
          await expectPausedNotice(page, table.worldName, {
            grounds: note,
            marked: true,
          });
        }
      });

      await test.step("5. the player tries a refresh and a new tab", async () => {
        await player.reload();
        await expectOnlyTheNotice(player, table.worldId, table.worldName);
        const newTab = await player.context().newPage();
        await newTab.goto(`/world/${table.worldId}/play`);
        await expectOnlyTheNotice(newTab, table.worldId, table.worldName);
        await newTab.close();
        // The refreshed page is a new document; mark it again, so step 7 can
        // tell the lift reached it without another reload.
        await markDocument(player);
      });

      const liftGrounds = `Capstone lift ${uniqueSuffix()}: settled with the guild`;
      let liftedAt = 0;
      await test.step("6. the operator lifts the pause", async () => {
        const active = operator.locator(
          `[data-testid="play-pause-active"][data-world-id="${table.worldId}"]`,
        );
        await expect(active).toBeVisible({ timeout: PORTAL_REFRESH_BUDGET_MS });
        await active
          .getByRole("button", {
            name: `Lift the pause on ${table.worldName}`,
          })
          .click();
        const dialog = operator.getByRole("dialog");
        await expect(dialog).toBeVisible();
        await dialog
          .getByRole("textbox", { name: "Grounds" })
          .fill(liftGrounds);
        await dialog.getByRole("button", { name: "Lift pause" }).click();
        await expect(dialog).toBeHidden({ timeout: 15_000 });
        liftedAt = Date.now();
        await expect(operator.getByTestId("play-pause-outcome")).toHaveText(
          `Play in ${table.worldName} is no longer paused.`,
        );
      });

      await test.step("7. the table returns, and a token move reaches the other browser", async () => {
        for (const page of [gm, player]) {
          const back = page.getByRole("link", { name: "Return to the world" });
          await expect(back).toBeVisible({
            timeout: Math.max(
              1_000,
              RETURN_WITHIN_MS - (Date.now() - liftedAt),
            ),
          });
          expect(
            await page.evaluate(
              () =>
                (window as unknown as { __e2eSameDocument?: boolean })
                  .__e2eSameDocument === true,
            ),
            "the notice must have noticed the lift without a reload",
          ).toBe(true);
          await back.click();
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

        await waitForEngineReady(gm);
        const tokenId = await createToken(gm);
        await waitForTokenTrafficToSettle(gm);
        await gm.keyboard.press("Escape");
        await gm.keyboard.press("Escape");
        const before = await tokenPosition(gm, tokenId);
        expect(before, "the Game Master's board has the token").not.toBeNull();
        expect(await dragToken(gm, tokenId, { dx: 192, dy: 96 })).toBe(true);
        await waitForTokenTrafficToSettle(gm);
        const moved = await tokenPosition(gm, tokenId);
        expect(moved).not.toEqual(before);

        await expect
          .poll(() => tokenPosition(player, tokenId), {
            timeout: 30_000,
            message: "the move must reach the player's browser",
          })
          .toEqual(moved);
        await expect(player).toHaveURL(
          new RegExp(`/world/${table.worldId}/play$`),
        );
      });

      await test.step("8. the Game Master reads the history", async () => {
        const settings = await gm.context().newPage();
        await settings.goto(`/world/${table.worldId}/settings/system`);
        const history = settings.getByTestId("play-pause-history-card");
        await expect(history).toBeVisible({ timeout: 20_000 });
        const rows = history.getByTestId("play-pause-history-row");
        await expect(rows).toHaveCount(1);
        await expect(rows.first().locator("time")).toHaveCount(2);
        const body = settings.locator("body");
        for (const secret of [note, liftGrounds, caseId]) {
          await expect(body).not.toContainText(secret);
        }
        await expect(body).not.toContainText(/takedown|dmca|infring/i);
        await settings.close();
      });

      await test.step("9. the operator reads the record", async () => {
        const recorded = operator.locator(
          `[data-testid="play-pause-record-pause"][data-world-id="${table.worldId}"]`,
        );
        await expect(recorded).toHaveAttribute("data-lifted", "true", {
          timeout: PORTAL_REFRESH_BUDGET_MS,
        });
        await expect(
          recorded.getByTestId("play-pause-record-grounds"),
        ).toHaveText(note);
        const trigger = recorded.getByTestId("play-pause-record-trigger");
        await expect(trigger).toHaveCount(1);
        await expect(trigger).toHaveAttribute("data-kind", "TAKEDOWN");
        await expect(trigger).toHaveAttribute("data-entity-id", sideSceneId);
        await expect(
          trigger.getByTestId("play-pause-record-case-link"),
        ).toContainText(caseId.slice(0, 8));
        const lift = recorded.getByTestId("play-pause-record-lift");
        await expect(lift).toContainText(/^Lifted .+ by \S/);
        await expect(lift).toContainText(liftGrounds);

        const request = operator.locator(
          `[data-testid="play-pause-record-request"][data-world-id="${table.worldId}"]`,
        );
        await expect(request).toHaveAttribute("data-state", "APPROVED");
        await expect(request).toContainText(note);
      });
    } finally {
      await operator.context().close();
      for (const context of table.contexts) await context.close();
    }
  });
});
