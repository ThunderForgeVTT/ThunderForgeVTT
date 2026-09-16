import { expect, test, type Page } from "@playwright/test";
import { freshCredentials, register, uniqueSuffix } from "../fixtures/helpers";
import {
  pauseThroughThePortal,
  seatATable,
  signInTheOperator,
} from "../fixtures/journeyTable";

/**
 * Spec 051 T072, US5 as people live it: what each person at a table is told
 * about a pause, and what only an operator is told.
 *
 * # Who is at the table
 *
 * A Game Master, a Trusted Player and a Player, made the way people are made:
 * the Game Master's join link, and a role changed on the Players page. The
 * three are held to the same rule, because ADR-099's Trusted Player runs part
 * of a table but is not told more about a pause than anyone else at it.
 *
 * # What they may be told
 *
 * *That* play is paused, and *since when*: on their world's card in the world
 * list, and in a quiet banner on the world's pages. After the lift, the
 * world's settings show the history as times only.
 *
 * # What they may never be told
 *
 * On every page a member visits, the page's whole text is searched for the
 * grounds, the operator's name, and the words a reason would be written in
 * ("takedown" first among them). The search is the whole `body`, not the
 * pause's own elements, because a reason leaking anywhere on the page is
 * still a reason told.
 *
 * # What the operator is told
 *
 * `/admin/play-pauses` *Record*: the pause, who paused and when, the grounds,
 * what prompted it, and the lift with who lifted, when and why. A pause made
 * in the portal is recorded as prompted by an operator. The trigger kind is
 * the one word members are told anyway ("paused by an operator"), which is
 * why the kinds members may never see are the reason words below.
 */

/** Words a reason would be written in; none may reach a member. */
const REASON_WORDS = /takedown|dmca|infring|copyright claim|violation/i;

interface Seat {
  role: string;
  page: Page;
}

/** Every page a member would look at a paused world through. */
async function expectToldThatAndWhen(
  seat: Seat,
  worldId: string,
  secrets: string[],
): Promise<void> {
  const { page, role } = seat;

  await test.step(`the ${role}'s world list says the world is paused, and since when`, async () => {
    await page.goto("/worlds");
    const card = page.getByTestId("world-card-play-paused");
    await expect(card).toContainText(/paused by an operator since \S/, {
      timeout: 20_000,
    });
    await expectNothingSecret(page, secrets);
  });

  await test.step(`the ${role}'s world pages carry a quiet banner`, async () => {
    for (const path of ["staging", "players", "settings/system"]) {
      await page.goto(`/world/${worldId}/${path}`);
      const banner = page.getByTestId("world-play-paused-banner");
      await expect(banner).toContainText(
        /Play in this world has been paused by an operator since \S/,
        { timeout: 20_000 },
      );
      await expect(banner).toHaveAttribute("role", "status");
      // Still on the page asked for: a member is not sent to the notice for
      // looking at their world (FR-024).
      await expect(page).toHaveURL(new RegExp(`/world/${worldId}/${path}$`));
      await expectNothingSecret(page, secrets);
    }
  });

  await test.step(`the ${role}'s notice says the same, and nothing more`, async () => {
    await page.goto(`/world/${worldId}/paused`);
    await expect(
      page.getByRole("heading", { name: "Play is paused", level: 1 }),
    ).toBeVisible({ timeout: 20_000 });
    await expectNothingSecret(page, secrets);
  });
}

async function expectNothingSecret(
  page: Page,
  secrets: string[],
): Promise<void> {
  const body = page.locator("body");
  for (const secret of secrets) {
    await expect(body).not.toContainText(secret);
  }
  await expect(body).not.toContainText(REASON_WORDS);
}

test.describe("spec 051 US5 journey: what the table is told", () => {
  test("a Game Master, a Trusted Player and a Player are told that and when; only the operator is told who and why", async ({
    browser,
  }) => {
    test.setTimeout(720_000);
    const table = await seatATable(browser, "Told");
    const trustedContext = await browser.newContext();
    const trustedPage = await trustedContext.newPage();
    const trusted = freshCredentials("jtrusted");
    const operator = await signInTheOperator(browser);

    try {
      await test.step("a third person joins by the Game Master's link", async () => {
        await table.gmPage.goto(`/world/${table.worldId}`);
        await table.gmPage
          .getByRole("button", { name: "Generate Join Link" })
          .click();
        const link = table.gmPage
          .getByRole("textbox", { name: "Invite link" })
          .first();
        await expect(link).toBeVisible({ timeout: 10_000 });
        const joinPath = new URL(await link.inputValue()).pathname;

        await register(trustedPage, trusted);
        await trustedPage.goto(joinPath);
        await trustedPage
          .getByRole("button", { name: "Join Campaign" })
          .click();
        await trustedPage.waitForURL(
          new RegExp(`/world/${table.worldId}/actor-select$`),
          { timeout: 20_000 },
        );
      });

      await test.step("the Game Master makes them a Trusted Player on the Players page", async () => {
        await table.gmPage.goto(`/world/${table.worldId}/players`);
        const card = table.gmPage
          .getByTestId("players-list")
          .locator('[data-testid^="player-card-"]')
          .filter({ hasText: trusted.username });
        await expect(card).toHaveCount(1, { timeout: 15_000 });
        const select = card.locator(
          'select[data-testid^="player-role-select-"]',
        );
        await select.selectOption("TrustedPlayer");
        await expect(select).toHaveValue("TrustedPlayer", { timeout: 10_000 });
      });

      const grounds = `Journey told ${uniqueSuffix()}: a takedown under review`;
      await test.step("an operator pauses the world in the portal", async () => {
        await pauseThroughThePortal(
          operator,
          table.worldId,
          table.worldName,
          grounds,
        );
      });

      const pauseRow = operator.locator(
        `[data-testid="play-pause-record-pause"][data-world-id="${table.worldId}"]`,
      );
      const operatorName =
        await test.step("the Record names who paused, and when", async () => {
          await expect(pauseRow).toHaveCount(1, { timeout: 15_000 });
          await expect(pauseRow).toHaveAttribute("data-lifted", "false");
          const said = (await pauseRow.innerText()).match(
            /Paused .+ by (\S+) ·/,
          );
          expect(said, "the Record names the operator").not.toBeNull();
          return said![1];
        });

      const secrets = [grounds, operatorName];
      const seats: Seat[] = [
        { role: "Game Master", page: table.gmPage },
        { role: "Trusted Player", page: trustedPage },
        { role: "Player", page: table.playerPage },
      ];
      for (const seat of seats) {
        await expectToldThatAndWhen(seat, table.worldId, secrets);
      }

      const liftGrounds = `Journey lift ${uniqueSuffix()}: settled`;
      await test.step("the operator lifts the pause, with grounds", async () => {
        await operator.goto("/admin");
        await operator.getByTestId("admin-nav-play-pauses").click();
        await operator.waitForURL(/\/admin\/play-pauses$/);
        const active = operator.locator(
          `[data-testid="play-pause-active"][data-world-id="${table.worldId}"]`,
        );
        await expect(active).toBeVisible({ timeout: 20_000 });
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
        await expect(operator.getByTestId("play-pause-outcome")).toHaveText(
          `Play in ${table.worldName} is no longer paused.`,
        );
      });

      await test.step("the Record shows the pause, its grounds, its trigger, and the lift", async () => {
        await expect(pauseRow).toHaveAttribute("data-lifted", "true", {
          timeout: 15_000,
        });
        await expect(pauseRow).toContainText(table.worldName);
        await expect(pauseRow).toContainText(`by ${operatorName}`);
        await expect(
          pauseRow.getByTestId("play-pause-record-grounds"),
        ).toHaveText(grounds);
        const trigger = pauseRow.getByTestId("play-pause-record-trigger");
        await expect(trigger).toHaveCount(1);
        await expect(trigger).toHaveAttribute("data-kind", "OPERATOR");
        await expect(trigger).toContainText(/^Operator recorded \S/);
        // Paused, the trigger recorded, and lifted.
        await expect(pauseRow.locator("time")).toHaveCount(3);
        const lift = pauseRow.getByTestId("play-pause-record-lift");
        await expect(lift).toContainText(
          new RegExp(`^Lifted .+ by ${operatorName}`),
        );
        await expect(lift).toContainText(liftGrounds);
      });

      for (const { role, page } of seats) {
        await test.step(`the ${role}'s settings show the history, times only`, async () => {
          await page.goto(`/world/${table.worldId}/settings/system`);
          const history = page.getByTestId("play-pause-history-card");
          await expect(history).toBeVisible({ timeout: 20_000 });
          const rows = history.getByTestId("play-pause-history-row");
          await expect(rows).toHaveCount(1);
          await expect(rows.first().locator("time")).toHaveCount(2);
          await expect(history).not.toContainText("Still paused");
          await expect(
            page.getByTestId("world-play-paused-banner"),
          ).toHaveCount(0);
          await expectNothingSecret(page, [...secrets, liftGrounds]);
        });
      }
    } finally {
      await operator.context().close();
      await trustedContext.close();
      for (const context of table.contexts) await context.close();
    }
  });
});
