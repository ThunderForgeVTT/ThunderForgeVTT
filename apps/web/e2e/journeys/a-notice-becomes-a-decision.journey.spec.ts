import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Locator,
  type Page,
} from "@playwright/test";
import { expectNoAxeViolations } from "../fixtures/axe";
import { createNpcViaCompendium } from "../fixtures/content";
import {
  freshCredentials,
  graphql,
  loginAsAdmin,
  register,
  uniqueSuffix,
} from "../fixtures/helpers";
import { expectPausedNotice, markDocument } from "../fixtures/playPause";

/**
 * Spec 051 T070, US3 as people live it: a claimant's notice becomes an
 * operator's decision.
 *
 * # UI only
 *
 * The claimant files through the real `/legal/dmca` form, signed out, typing
 * an id they read off the page where the content is. The Game Master and the
 * player register, create, join and press Play. The operator signs in with
 * their second factor on the login page and decides in the portal, opening
 * the moderation case from the request. GraphQL appears only to **check**
 * what the server holds.
 *
 * # What it holds the product to
 *
 * 1. A request appears on the operator's open page by the page's own refresh:
 *    the operator does not reload. It says the world is being played, and
 *    links to the moderation case, which opens.
 * 2. Approving pauses: the table lands on the notice.
 * 3. Declining changes nothing any member can see: the table's page does not
 *    move, and the Game Master's own pages say nothing of it.
 * 4. Two operator tabs deciding one request at once, one approving and one
 *    declining: one decision stands, and the other tab is told who decided and
 *    when, as an outcome and not as an error.
 */

/** How long the portal takes to show what changed, plus a round trip. */
const PORTAL_REFRESH_BUDGET_MS = 15_000;

interface Seat {
  worldId: string;
  worldName: string;
  gmPage: Page;
  playerPage: Page | null;
  contexts: BrowserContext[];
}

/** Press Play from wherever a member is in the world, and wait for the canvas. */
async function pressPlay(page: Page, worldId: string): Promise<void> {
  if (!new URL(page.url()).pathname.endsWith("/staging")) {
    await page.goto(`/world/${worldId}`);
    await page.getByRole("link", { name: "Enter world" }).click();
    await page.waitForURL(/\/staging$/, { timeout: 15_000 });
  }
  await page.getByTestId("play-button").click();
  await page.waitForURL(new RegExp(`/world/${worldId}/play$`), {
    timeout: 15_000,
  });
  await expect(page.locator("canvas")).toBeVisible({ timeout: 90_000 });
}

/**
 * A Game Master's world, and optionally a player at it, set up the way they
 * would do it. `beforePlay` runs on the Game Master's page before anyone
 * presses Play: it is where content gets made.
 */
async function seatATable(
  browser: Browser,
  label: string,
  {
    withPlayer,
    beforePlay,
  }: {
    withPlayer: boolean;
    beforePlay?: (gmPage: Page, worldId: string) => Promise<void>;
  },
): Promise<Seat> {
  const suffix = uniqueSuffix();
  const worldName = `Journey ${label} ${suffix}`;
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  const contexts = [gmContext];

  let worldId = "";
  await test.step(`the Game Master creates "${worldName}"`, async () => {
    await register(gmPage, freshCredentials("jgm"));
    await gmPage.waitForURL(/\/worlds\/create$/, { timeout: 15_000 });
    await gmPage.locator("#world-name").fill(worldName);
    await gmPage.getByRole("button", { name: /create world/i }).click();
    await gmPage.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 15_000 });
    worldId = /\/world\/([^/]+)\/staging$/.exec(
      new URL(gmPage.url()).pathname,
    )![1];
  });

  let playerPage: Page | null = null;
  if (withPlayer) {
    const playerContext = await browser.newContext();
    contexts.push(playerContext);
    playerPage = await playerContext.newPage();
    const player = playerPage;

    let joinLink = "";
    await test.step("the Game Master lets players make characters, and makes a join link", async () => {
      await gmPage.goto(`/world/${worldId}`);
      const toggle = gmPage.getByTestId("allow-player-created-actors-toggle");
      await toggle.click();
      await expect(toggle.locator("input")).toBeChecked({ timeout: 10_000 });
      await gmPage.getByRole("button", { name: "Generate Join Link" }).click();
      const link = gmPage.getByRole("textbox", { name: "Invite link" }).first();
      await expect(link).toBeVisible({ timeout: 10_000 });
      joinLink = await link.inputValue();
      expect(joinLink).toMatch(/\/join\/[^/]+$/);
    });

    await test.step("the player registers, follows the link, joins and makes a character", async () => {
      await register(player, freshCredentials("jplayer"));
      await player.goto(new URL(joinLink).pathname);
      await player.getByRole("button", { name: "Join Campaign" }).click();
      await player.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
        timeout: 20_000,
      });
      await player.locator("#new-character-name").fill(`Wanderer ${suffix}`);
      await player
        .getByRole("button", { name: /create and play as this character/i })
        .click();
      await player.waitForURL(new RegExp(`/world/${worldId}$`), {
        timeout: 20_000,
      });
    });
  }

  if (beforePlay) await beforePlay(gmPage, worldId);

  await test.step(
    withPlayer ? "both press Play" : "the Game Master presses Play",
    async () => {
      await pressPlay(gmPage, worldId);
      if (playerPage) await pressPlay(playerPage, worldId);
    },
  );

  return { worldId, worldName, gmPage, playerPage, contexts };
}

/**
 * The id of the world's starting scene, read the way a person reads it: open
 * the Scenes section, follow the scene's link, and take the id from the
 * address. Done in a tab of its own so the playfield is left alone.
 */
async function startingSceneId(seat: Seat): Promise<string> {
  const tab = await seat.gmPage.context().newPage();
  try {
    // Staging, where the world's sidebar is: the dashboard has none.
    await tab.goto(`/world/${seat.worldId}/staging`);
    await tab.getByTestId("world-nav-scenes").click();
    await tab.waitForURL(new RegExp(`/world/${seat.worldId}/scenes$`), {
      timeout: 15_000,
    });
    await tab.getByRole("link", { name: "Starting Scene" }).click();
    await tab.waitForURL(new RegExp(`/world/${seat.worldId}/scenes/[^/]+$`), {
      timeout: 15_000,
    });
    return /\/scenes\/([^/]+)$/.exec(new URL(tab.url()).pathname)![1];
  } finally {
    await tab.close();
  }
}

type ContentType = "Scene" | "Actor / NPC / character";

/** A signed-out claimant files a complete notice through `/legal/dmca`. */
async function fileNotice(
  browser: Browser,
  contentType: ContentType,
  entityId: string,
  describedAs: string,
): Promise<string> {
  const context = await browser.newContext();
  const claimant = await context.newPage();
  try {
    await claimant.goto("/legal/dmca");
    await expect(claimant.getByTestId("takedown-notice-form")).toBeVisible({
      timeout: 20_000,
    });
    await claimant.getByLabel("Content type").click();
    await claimant.getByRole("option", { name: contentType }).click();
    await claimant.locator("#dmca-entity-id").fill(entityId);
    await claimant.locator("#dmca-claimant-name").fill("Cartography Guild");
    await claimant
      .locator("#dmca-claimant-contact")
      .fill("notices@cartography.example.test");
    await claimant
      .locator("#dmca-work-description")
      .fill("An original battle map, published and registered.");
    await claimant
      .locator("#dmca-infringing-location")
      .fill(`${describedAs}, in a ThunderForge world.`);
    await claimant.locator("#dmca-good-faith").click();
    await claimant.locator("#dmca-accuracy").click();
    await claimant.locator("#dmca-signature").fill("Cartography Guild");
    await claimant.getByTestId("takedown-notice-submit").click();
    const accepted = claimant.getByTestId("takedown-notice-accepted");
    await expect(accepted).toBeVisible({ timeout: 20_000 });
    await expect(accepted).toContainText("Case reference");
    return (await accepted.locator("code").innerText()).trim();
  } finally {
    await context.close();
  }
}

/** The operator, signed in through the login page, on `/admin/play-pauses`. */
async function operatorInThePortal(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const operator = await context.newPage();
  await loginAsAdmin(operator);
  await operator.waitForURL(/\/admin$/, { timeout: 20_000 });
  await operator.getByTestId("admin-nav-play-pauses").click();
  await operator.waitForURL(/\/admin\/play-pauses$/);
  await expect(
    operator.getByRole("heading", { name: "Play pauses", level: 1 }),
  ).toBeVisible({ timeout: 20_000 });
  return operator;
}

function requestRow(operator: Page, worldId: string): Locator {
  return operator.locator(
    `[data-testid="play-pause-request"][data-world-id="${worldId}"]`,
  );
}

/** Whether this page is still the document `markDocument` marked. */
async function sameDocument(page: Page): Promise<boolean> {
  return page.evaluate(
    () =>
      (window as unknown as { __e2eSameDocument?: boolean })
        .__e2eSameDocument === true,
  );
}

/** Server state, as a member reads it. */
async function playState(
  member: Page,
  worldId: string,
): Promise<{ paused: boolean; history: unknown[] }> {
  const answer = await graphql<{
    data?: { worldPlayState?: { paused: boolean; history: unknown[] } };
  }>(
    member,
    `
      query ($worldId: UUID!) {
        worldPlayState(worldId: $worldId) {
          paused
          history {
            pausedAt
            liftedAt
          }
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.worldPlayState) {
    throw new Error(`worldPlayState did not answer: ${JSON.stringify(answer)}`);
  }
  return answer.data.worldPlayState;
}

/** Server state, as the operator reads it: pauses in force on `worldId`. */
async function activePauses(
  operator: Page,
  worldId: string,
): Promise<{ requestId: string | null; grounds: string }[]> {
  const answer = await graphql<{
    data?: {
      playPauses?: { nodes: { requestId: string | null; grounds: string }[] };
    };
  }>(
    operator,
    `
      query ($worldId: UUID) {
        playPauses(active: true, worldId: $worldId, first: 10) {
          nodes {
            requestId
            grounds
          }
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.playPauses) {
    throw new Error(`playPauses did not answer: ${JSON.stringify(answer)}`);
  }
  return answer.data.playPauses.nodes;
}

/** Open Approve or Decline on a row, and check the note is required. */
async function openDecision(
  operator: Page,
  worldId: string,
  decision: "Approve" | "Decline",
  note: string,
): Promise<Locator> {
  const row = requestRow(operator, worldId);
  await row.getByRole("button", { name: new RegExp(`^${decision} `) }).click();
  const dialog = operator.getByRole("dialog");
  await expect(dialog).toBeVisible();
  const submit = dialog.getByTestId("play-pause-decide-submit");
  const field = dialog.getByRole("textbox", { name: "Note" });
  await expect(field).toHaveAttribute("aria-required", "true");
  await expect(submit, "a blank note").toBeDisabled();
  await field.fill("  \n ");
  await expect(submit, "a whitespace note").toBeDisabled();
  await field.fill(note);
  await expect(submit).toBeEnabled();
  return dialog;
}

async function closeAll(pages: Page[], seats: Seat[]): Promise<void> {
  for (const page of pages) await page.context().close();
  for (const seat of seats) {
    for (const context of seat.contexts) await context.close();
  }
}

test.describe("spec 051 US3 journey: a notice becomes a decision", () => {
  test("a notice on a played scene reaches the portal by itself; the case opens; approving pauses the table", async ({
    browser,
  }) => {
    test.setTimeout(600_000);
    const seat = await seatATable(browser, "Noticed", { withPlayer: true });
    const operator = await operatorInThePortal(browser);

    try {
      const sceneId = await startingSceneId(seat);
      // The operator has the portal open before there is anything to decide,
      // and never reloads it.
      await markDocument(operator);
      await expect(requestRow(operator, seat.worldId)).toHaveCount(0);

      let caseId = "";
      await test.step("a claimant files against the scene being played", async () => {
        caseId = await fileNotice(
          browser,
          "Scene",
          sceneId,
          `The scene "Starting Scene"`,
        );
      });

      const row = requestRow(operator, seat.worldId);
      await test.step("the request appears on the operator's page, being played", async () => {
        await expect(row).toContainText(seat.worldName, {
          timeout: PORTAL_REFRESH_BUDGET_MS,
        });
        expect(
          await sameDocument(operator),
          "the operator must not have reloaded",
        ).toBe(true);
        await expect(
          row.getByTestId("play-pause-request-played-now"),
        ).toHaveText("Being played");
        const trigger = row.getByTestId("play-pause-request-trigger");
        await expect(trigger).toHaveCount(1);
        await expect(trigger).toContainText("Takedown on a scene");
        await expectNoAxeViolations(operator);
      });

      await test.step("the link to the moderation case opens that case", async () => {
        await row.getByRole("link", { name: /Moderation case/ }).click();
        await operator.waitForURL(/\/admin\/moderation\?case=/);
        const opened = operator.getByTestId("opened-case");
        await expect(opened).toContainText(caseId, { timeout: 15_000 });
        await expect(opened).toContainText("SCENE");
        await operator.goBack();
        await operator.waitForURL(/\/admin\/play-pauses$/);
        await expect(requestRow(operator, seat.worldId)).toBeVisible({
          timeout: PORTAL_REFRESH_BUDGET_MS,
        });
      });

      const note = `Journey approval ${uniqueSuffix()}: the guild's map`;
      await test.step("the operator approves, with a note", async () => {
        const dialog = await openDecision(
          operator,
          seat.worldId,
          "Approve",
          note,
        );
        await expect(dialog).toContainText(
          `Approve, and pause play in ${seat.worldName}?`,
        );
        await expectNoAxeViolations(
          operator,
          '[data-testid="play-pause-decide"]',
        );
        await dialog.getByTestId("play-pause-decide-submit").click();
        await expect(dialog).toBeHidden({ timeout: 15_000 });
        const outcome = operator.getByTestId("play-pause-outcome");
        await expect(outcome).toHaveText(
          `Play in ${seat.worldName} is paused.`,
        );
        await expect(outcome).toBeFocused();
        await expect(requestRow(operator, seat.worldId)).toHaveCount(0);
        await expect(
          operator.locator(
            `[data-testid="play-pause-active"][data-world-id="${seat.worldId}"]`,
          ),
        ).toContainText(note);
      });

      await test.step("the table lands on the notice", async () => {
        for (const page of [seat.gmPage, seat.playerPage!]) {
          await page.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
          await expectPausedNotice(page, seat.worldName, { grounds: note });
        }
        const pauses = await activePauses(operator, seat.worldId);
        expect(pauses).toHaveLength(1);
        expect(pauses[0].grounds).toBe(note);
        expect(pauses[0].requestId).not.toBeNull();
      });
    } finally {
      await closeAll([operator], [seat]);
    }
  });

  test("declining in a second world: the table's page never moves, and the Game Master's pages show nothing", async ({
    browser,
  }) => {
    test.setTimeout(480_000);
    const npcName = `Borrowed Wyvern ${uniqueSuffix()}`;
    let npcId = "";
    const seat = await seatATable(browser, "Quiet", {
      withPlayer: false,
      // A creature in the compendium, not on the map: whatever the takedown
      // itself does to what a table shows cannot be mistaken for the decision.
      beforePlay: async (gmPage, worldId) => {
        await test.step("the Game Master makes an NPC in the compendium", async () => {
          npcId = await createNpcViaCompendium(gmPage, worldId, npcName);
        });
      },
    });
    const operator = await operatorInThePortal(browser);

    try {
      await test.step("a claimant files against the NPC", async () => {
        await fileNotice(
          browser,
          "Actor / NPC / character",
          npcId,
          `The creature "${npcName}"`,
        );
      });

      const row = requestRow(operator, seat.worldId);
      await expect(row).toContainText(seat.worldName, {
        timeout: PORTAL_REFRESH_BUDGET_MS,
      });
      await expect(row.getByTestId("play-pause-request-trigger")).toContainText(
        "Takedown on an actor",
      );

      const table = seat.gmPage;
      const addressBefore = table.url();
      await markDocument(table);
      const pauseFrames: string[] = [];
      table.on("websocket", (socket) => {
        socket.on("framereceived", ({ payload }) => {
          const text =
            typeof payload === "string" ? payload : payload.toString();
          if (/"eventCode"\s*:\s*28\b/.test(text)) pauseFrames.push(text);
        });
      });

      const note = `Journey decline ${uniqueSuffix()}: not the guild's work`;
      await test.step("the operator declines, with a note", async () => {
        const dialog = await openDecision(
          operator,
          seat.worldId,
          "Decline",
          note,
        );
        await expect(dialog).toContainText(
          `Decline the request to pause ${seat.worldName}?`,
        );
        await expectNoAxeViolations(
          operator,
          '[data-testid="play-pause-decide"]',
        );
        await dialog.getByTestId("play-pause-decide-submit").click();
        await expect(dialog).toBeHidden({ timeout: 15_000 });
        await expect(operator.getByTestId("play-pause-outcome")).toHaveText(
          `The request to pause ${seat.worldName} was declined. Nothing in the world changed.`,
        );
        await expect(requestRow(operator, seat.worldId)).toHaveCount(0);
      });

      await test.step("for fifteen seconds, the table's page does not move", async () => {
        // Three heartbeats and three server ticks, any of which would carry a
        // pause to this page if there were one.
        const until = Date.now() + 15_000;
        while (Date.now() < until) {
          expect(table.url()).toBe(addressBefore);
          await expect(table.getByTestId("play-paused-notice")).toHaveCount(0);
          await table.waitForTimeout(1_000);
        }
        await expect(table.locator("canvas")).toBeVisible();
        expect(await sameDocument(table), "the table must not reload").toBe(
          true,
        );
        expect(pauseFrames, "no pause event may reach the table").toEqual([]);
      });

      await test.step("the Game Master's own pages say nothing of it", async () => {
        const tab = await table.context().newPage();
        try {
          for (const path of [
            `/world/${seat.worldId}`,
            `/world/${seat.worldId}/staging`,
            `/world/${seat.worldId}/compendium`,
          ]) {
            await tab.goto(path);
            await expect(tab.locator("main").first()).toBeVisible({
              timeout: 20_000,
            });
            await tab.waitForLoadState("networkidle").catch(() => undefined);
            const body = tab.locator("body");
            await expect(body).not.toContainText(note);
            await expect(body).not.toContainText(
              /paused|pause request|declined/i,
            );
          }
        } finally {
          await tab.close();
        }
        const state = await playState(table, seat.worldId);
        expect(state.paused).toBe(false);
        expect(state.history).toEqual([]);
        expect(await activePauses(operator, seat.worldId)).toEqual([]);
      });
    } finally {
      await closeAll([operator], [seat]);
    }
  });

  test("two operator tabs, one approving and one declining at once: one decision stands, the other is told who and when", async ({
    browser,
  }) => {
    test.setTimeout(480_000);
    const seat = await seatATable(browser, "Contested", { withPlayer: false });
    const operator = await operatorInThePortal(browser);
    // The same operator's second tab: a person with the portal open twice.
    const secondTab = await operator.context().newPage();

    try {
      const sceneId = await startingSceneId(seat);
      await secondTab.goto("/admin/play-pauses");
      await expect(
        secondTab.getByRole("heading", { name: "Play pauses", level: 1 }),
      ).toBeVisible({ timeout: 20_000 });

      await fileNotice(browser, "Scene", sceneId, `The scene "Starting Scene"`);
      for (const tab of [operator, secondTab]) {
        await expect(requestRow(tab, seat.worldId)).toContainText(
          seat.worldName,
          { timeout: PORTAL_REFRESH_BUDGET_MS },
        );
      }

      const approveNote = `Journey race approve ${uniqueSuffix()}`;
      const declineNote = `Journey race decline ${uniqueSuffix()}`;
      const approving = await openDecision(
        operator,
        seat.worldId,
        "Approve",
        approveNote,
      );
      const declining = await openDecision(
        secondTab,
        seat.worldId,
        "Decline",
        declineNote,
      );

      await test.step("both confirm at once", async () => {
        await Promise.all([
          approving.getByTestId("play-pause-decide-submit").click(),
          declining.getByTestId("play-pause-decide-submit").click(),
        ]);
        await expect(approving).toBeHidden({ timeout: 15_000 });
        await expect(declining).toBeHidden({ timeout: 15_000 });
      });

      const approveText = (
        await operator.getByTestId("play-pause-outcome").innerText()
      ).trim();
      const declineText = (
        await secondTab.getByTestId("play-pause-outcome").innerText()
      ).trim();
      const approvedHere =
        approveText === `Play in ${seat.worldName} is paused.`;
      const declinedHere =
        declineText ===
        `The request to pause ${seat.worldName} was declined. Nothing in the world changed.`;
      expect(
        [approvedHere, declinedHere].filter(Boolean),
        `exactly one decision stands: ${JSON.stringify([approveText, declineText])}`,
      ).toHaveLength(1);

      console.log(
        `[journey] race: ${approvedHere ? "approve" : "decline"} stood; the other tab read: ${approvedHere ? declineText : approveText}`,
      );
      await test.step("the other tab is told who decided and when, with no error", async () => {
        const [loser, loserText, how] = approvedHere
          ? [secondTab, declineText, "approved"]
          : [operator, approveText, "declined"];
        expect(loserText).toMatch(
          new RegExp(
            `^Already decided by \\S+ at .*\\d.*\\. The request to pause ${seat.worldName} was ${how}\\.$`,
          ),
        );
        const outcome = loser.getByTestId("play-pause-outcome");
        await expect(outcome).toHaveAttribute("role", "status");
        await expect(outcome).toBeFocused();
        await expect(loser.getByRole("alert")).toHaveCount(0);
        await expect(requestRow(loser, seat.worldId)).toHaveCount(0);
      });

      await test.step("and the world is as the decision that stood says", async () => {
        const pauses = await activePauses(operator, seat.worldId);
        if (approvedHere) {
          expect(pauses.map((p) => p.grounds)).toEqual([approveNote]);
          await seat.gmPage.waitForURL(/\/world\/[^/]+\/paused$/, {
            timeout: 15_000,
          });
          await expectPausedNotice(seat.gmPage, seat.worldName, {
            grounds: approveNote,
          });
        } else {
          expect(pauses).toEqual([]);
          await seat.gmPage.waitForTimeout(10_000);
          await expect(seat.gmPage).toHaveURL(/\/play$/);
          expect((await playState(seat.gmPage, seat.worldId)).history).toEqual(
            [],
          );
        }
      });
    } finally {
      await closeAll([operator], [seat]);
    }
  });
});
