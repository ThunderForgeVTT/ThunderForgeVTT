import fs from "node:fs";
import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Locator,
  type Page,
} from "@playwright/test";
import { expectNoAxeViolations } from "../fixtures/axe";
import { ADMIN_SECOND_FACTOR_PATH, ADMIN_USER } from "../fixtures/global-setup";
import {
  freshCredentials,
  graphql,
  loginAsAdmin,
  register,
  uniqueSuffix,
} from "../fixtures/helpers";
import { expectPausedNotice } from "../fixtures/playPause";
import { totpAt } from "../fixtures/totp";

/**
 * Spec 051 T067, US1 as people live it: an operator pauses a table through
 * the portal, and the table lands on the notice.
 *
 * # UI only
 *
 * Everything a person does here is done through the product: the Game Master
 * registers, creates the world, turns on player-created characters and makes a
 * join link; the player registers, follows the link, joins and makes a
 * character; both press Play; the operator signs in with their second factor
 * on the login page, reaches the portal from the admin navigation, searches by
 * name and confirms. GraphQL appears only to **check** what the server holds
 * — that a cancelled dialog paused nothing, that a confirmed one did, that a
 * non-operator is refused by the server as well as by the route.
 *
 * `play-pause.spec.ts` covers the same story for timing and for the roads a
 * pause travels, and takes shortcuts to get there. This file is the other
 * half: the path, the keyboard, and what the page lets a person get wrong.
 */

interface Table {
  worldId: string;
  worldName: string;
  gmPage: Page;
  playerPage: Page;
  contexts: BrowserContext[];
}

/**
 * A Game Master and a player at one world, set up the way they would do it.
 *
 * `play` decides whether both then press Play. The non-operator journey needs
 * the two people, not the playfield, and an engine load is the slowest thing a
 * journey does.
 */
async function seatATable(
  browser: Browser,
  label: string,
  { play }: { play: boolean },
): Promise<Table> {
  const suffix = uniqueSuffix();
  const worldName = `Journey ${label} ${suffix}`;
  // Clipboard permission because "Generate Join Link" also copies the link,
  // and a headless context without it throws inside that click.
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const playerContext = await browser.newContext();
  const gmPage = await gmContext.newPage();
  const playerPage = await playerContext.newPage();

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

  let joinLink = "";
  await test.step("the Game Master lets players make characters, and makes a join link", async () => {
    // The dashboard, where the campaign settings live. Reached by address:
    // no navigation in the world's hub links to it.
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
    await register(playerPage, freshCredentials("jplayer"));
    await playerPage.goto(new URL(joinLink).pathname);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 20_000,
    });
    await playerPage.locator("#new-character-name").fill(`Wanderer ${suffix}`);
    await playerPage
      .getByRole("button", { name: /create and play as this character/i })
      .click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), {
      timeout: 20_000,
    });
  });

  if (play) {
    await test.step("both press Play", async () => {
      for (const page of [gmPage, playerPage]) {
        if (!new URL(page.url()).pathname.endsWith("/staging")) {
          await page.goto(`/world/${worldId}`);
          await page.getByRole("link", { name: "Enter world" }).click();
          await page.waitForURL(/\/staging$/, { timeout: 15_000 });
        }
        await page.getByTestId("play-button").click();
        await page.waitForURL(new RegExp(`/world/${worldId}/play$`), {
          timeout: 15_000,
        });
      }
      await expect(gmPage.locator("canvas")).toBeVisible({ timeout: 90_000 });
      await expect(playerPage.locator("canvas")).toBeVisible({
        timeout: 90_000,
      });
    });
  }

  return {
    worldId,
    worldName,
    gmPage,
    playerPage,
    contexts: [gmContext, playerContext],
  };
}

interface ActivePause {
  worldId: string;
  grounds: string;
}

/** Server state: the pauses in force on `worldId`, as the operator reads them. */
async function activePausesOn(
  operator: Page,
  worldId: string,
): Promise<ActivePause[]> {
  const answer = await graphql<{
    data?: { playPauses?: { nodes: ActivePause[] } };
    errors?: { message: string }[];
  }>(
    operator,
    `
      query ($worldId: UUID) {
        playPauses(active: true, worldId: $worldId, first: 10) {
          nodes {
            worldId
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

/** Server state: whether a member of `worldId` is told its play is paused. */
async function worldIsPaused(member: Page, worldId: string): Promise<boolean> {
  const answer = await graphql<{
    data?: { worldPlayState?: { paused: boolean } };
    errors?: unknown;
  }>(
    member,
    `
      query ($worldId: UUID!) {
        worldPlayState(worldId: $worldId) {
          paused
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.worldPlayState) {
    throw new Error(`worldPlayState did not answer: ${JSON.stringify(answer)}`);
  }
  return answer.data.worldPlayState.paused;
}

async function expectNothingPaused(
  operator: Page,
  table: Table,
): Promise<void> {
  expect(
    await activePausesOn(operator, table.worldId),
    "no pause may be in force",
  ).toEqual([]);
  expect(await worldIsPaused(table.gmPage, table.worldId)).toBe(false);
  await expect(table.gmPage).toHaveURL(/\/play$/);
  await expect(table.playerPage).toHaveURL(/\/play$/);
}

/**
 * The focused element is on screen and visibly marked as focused.
 *
 * "Visibly marked" is read from the computed style of the element while it
 * holds focus: an outline (the browser's own `auto` ring counts) or a box
 * shadow, which is how the design system's `focus-visible:ring-*` draws. A
 * border colour change alone is not accepted, because it is not an indicator
 * a person can rely on seeing. `:focus-visible` must match too — the page
 * must treat this as keyboard focus, not as a click.
 */
async function expectFocusVisible(page: Page, where: string): Promise<void> {
  const focus = await page.evaluate(() => {
    const el = document.activeElement as HTMLElement | null;
    if (!el || el === document.body) return null;
    const rect = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    const outline =
      style.outlineStyle !== "none" && parseFloat(style.outlineWidth) > 0;
    const ring = style.boxShadow !== "none" && style.boxShadow !== "";
    return {
      description: `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ""} "${(
        el.getAttribute("aria-label") ??
        el.textContent ??
        ""
      )
        .trim()
        .slice(0, 40)}"`,
      onScreen:
        rect.width > 0 &&
        rect.height > 0 &&
        rect.bottom > 0 &&
        rect.right > 0 &&
        rect.top < window.innerHeight &&
        rect.left < window.innerWidth,
      shown: el.checkVisibility({
        visibilityProperty: true,
        opacityProperty: true,
      }),
      focusVisible: el.matches(":focus-visible"),
      indicator: outline || ring,
    };
  });
  expect(
    focus,
    `${where}: focus must be on an element, not lost`,
  ).not.toBeNull();
  const f = focus!;
  expect(
    f.shown && f.onScreen,
    `${where}: ${f.description} must be visible`,
  ).toBe(true);
  expect(
    f.focusVisible,
    `${where}: ${f.description} must match :focus-visible`,
  ).toBe(true);
  expect(
    f.indicator,
    `${where}: ${f.description} must draw a focus indicator`,
  ).toBe(true);
}

/**
 * Press Tab until `target` holds focus, checking focus is visible at every
 * stop on the way. Bounded, so a target the keyboard cannot reach fails as
 * exactly that rather than as a timeout.
 */
async function tabTo(
  page: Page,
  target: Locator,
  where: string,
  maxStops = 60,
): Promise<void> {
  for (let stop = 0; stop < maxStops; stop += 1) {
    await page.keyboard.press("Tab");
    await expectFocusVisible(page, `${where} (Tab stop ${stop + 1})`);
    if (await target.evaluate((el) => el === document.activeElement)) return;
  }
  throw new Error(`${where}: not reachable with Tab in ${maxStops} stops`);
}

/** The operator's current code, on the login page's challenge, by keyboard. */
async function signInAsOperatorByKeyboard(page: Page): Promise<void> {
  const info = test.info();
  info.setTimeout(info.timeout + 120_000);
  await page.goto("/login");
  const identifier = page.locator("#login-identifier");
  await expect(identifier).toBeVisible({ timeout: 20_000 });
  await tabTo(page, identifier, "login: identifier");
  await page.keyboard.type(ADMIN_USER.identifier);
  await tabTo(page, page.locator("#login-password"), "login: password");
  await page.keyboard.type(ADMIN_USER.password);
  await page.keyboard.press("Enter");

  const code = page.locator("#login-two-factor");
  await expect(code, "an operator is challenged (FR-027)").toBeVisible({
    timeout: 20_000,
  });
  const { secret } = JSON.parse(
    fs.readFileSync(ADMIN_SECOND_FACTOR_PATH, "utf-8"),
  ) as { secret: string };
  // The same spent-step retry `loginAsAdmin` explains: an earlier journey in
  // this run may have used this step's code.
  for (let attempt = 0; attempt < 3; attempt += 1) {
    if (!(await code.evaluate((el) => el === document.activeElement))) {
      await tabTo(page, code, "login: second factor");
    }
    await page.keyboard.type(totpAt(secret, Date.now() / 1000));
    await page.keyboard.press("Enter");
    try {
      await page.waitForURL((url) => !url.pathname.startsWith("/login"), {
        timeout: 8_000,
        waitUntil: "commit",
      });
      return;
    } catch {
      const intoStep = (Date.now() / 1000) % 30;
      await page.waitForTimeout((30 - intoStep + 1) * 1000);
      // Clear what was typed, with the keyboard's own editing key. Focus is
      // still in the field: a refused code leaves the challenge as it was.
      for (let i = 0; i < 6; i += 1) await page.keyboard.press("Backspace");
    }
  }
  throw new Error("the operator's second factor was refused three times");
}

/** The portal's row for `worldId` under Active pauses. */
function activeRow(operator: Page, worldId: string): Locator {
  return operator.locator(
    `[data-testid="play-pause-active"][data-world-id="${worldId}"]`,
  );
}

async function closeAll(table: Table, ...pages: Page[]): Promise<void> {
  for (const page of pages) await page.context().close();
  for (const context of table.contexts) await context.close();
}

test.describe("spec 051 US1 journey: an operator pauses a table through the portal", () => {
  test("by pointer: sign in with 2FA, find the world by name, refuse blank grounds, cancel, confirm", async ({
    browser,
  }) => {
    test.setTimeout(480_000);
    const table = await seatATable(browser, "Pointer", { play: true });
    const operatorContext = await browser.newContext();
    const operator = await operatorContext.newPage();

    try {
      await test.step("the operator signs in through the login page, second factor and all", async () => {
        // `loginAsAdmin` is the login page driven as a person drives it: the
        // identifier, the password, Sign in, then the six-digit code.
        await loginAsAdmin(operator);
        await operator.waitForURL(/\/admin$/, { timeout: 20_000 });
      });

      await test.step("and reaches Play pauses from the admin navigation", async () => {
        await operator.getByTestId("admin-nav-play-pauses").click();
        await operator.waitForURL(/\/admin\/play-pauses$/);
        await expect(
          operator.getByRole("heading", { name: "Play pauses", level: 1 }),
        ).toBeVisible({ timeout: 20_000 });
        await expect(
          operator.getByTestId("admin-nav-play-pauses"),
        ).toHaveAttribute("aria-current", "page");
        await expectNoAxeViolations(operator);
      });

      const candidate = operator.locator(
        `[data-testid="play-pause-candidate"][data-world-id="${table.worldId}"]`,
      );
      await test.step("searches for the world by its name", async () => {
        await operator
          .getByRole("textbox", { name: "World name or id" })
          .fill(table.worldName);
        await operator.getByRole("button", { name: "Find world" }).click();
        await expect(candidate).toContainText(table.worldName, {
          timeout: 15_000,
        });
      });

      const dialog = operator.getByRole("dialog");
      const confirm = dialog.getByRole("button", { name: "Pause play" });
      const grounds = dialog.getByRole("textbox", { name: "Grounds" });

      await test.step("the dialog refuses blank and whitespace-only grounds", async () => {
        await candidate.getByRole("button", { name: "Pause play" }).click();
        await expect(dialog).toBeVisible();
        await expect(dialog).toContainText(`Pause play in ${table.worldName}?`);
        await expect(grounds).toHaveAttribute("aria-required", "true");
        await expectNoAxeViolations(
          operator,
          '[data-testid="play-pause-confirm"]',
        );

        await expect(confirm, "blank grounds").toBeDisabled();
        await grounds.fill("   \n\t  ");
        await expect(confirm, "whitespace-only grounds").toBeDisabled();
        await confirm.click({ force: true }).catch(() => undefined);
        await expect(
          dialog,
          "a refused confirm leaves the dialog open",
        ).toBeVisible();
        expect(await activePausesOn(operator, table.worldId)).toEqual([]);
      });

      await test.step("Cancel closes it and pauses nothing", async () => {
        await grounds.fill("Written, and then thought better of.");
        await expect(confirm).toBeEnabled();
        await dialog.getByRole("button", { name: "Cancel" }).click();
        await expect(dialog).toBeHidden();
        await expect(operator.getByTestId("play-pause-outcome")).toHaveCount(0);
        await expect(activeRow(operator, table.worldId)).toHaveCount(0);
        await expectNothingPaused(operator, table);
      });

      const written = `Journey grounds ${uniqueSuffix()}: live abuse at the table`;
      await test.step("Confirm lists it under Active pauses", async () => {
        await candidate.getByRole("button", { name: "Pause play" }).click();
        await expect(dialog).toBeVisible();
        // Reopening starts from nothing, not from the cancelled draft.
        await expect(grounds).toHaveValue("");
        await grounds.fill(written);
        await confirm.click();
        await expect(dialog).toBeHidden({ timeout: 15_000 });
        await expect(operator.getByTestId("play-pause-outcome")).toHaveText(
          `Play in ${table.worldName} is paused.`,
        );
        const active = activeRow(operator, table.worldId);
        await expect(active).toContainText(table.worldName);
        await expect(active).toContainText(written);
        await expect(candidate).toContainText("Paused");

        const onServer = await activePausesOn(operator, table.worldId);
        expect(onServer).toHaveLength(1);
        expect(onServer[0].grounds).toBe(written);
      });

      await test.step("both playing tables land on the notice", async () => {
        for (const page of [table.gmPage, table.playerPage]) {
          await page.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
          await expectPausedNotice(page, table.worldName, { grounds: written });
        }
        expect(await worldIsPaused(table.playerPage, table.worldId)).toBe(true);
      });
    } finally {
      await closeAll(table, operator);
    }
  });

  test("by keyboard alone: Tab, Enter and Escape, with focus visible at every step", async ({
    browser,
  }) => {
    test.setTimeout(540_000);
    const table = await seatATable(browser, "Keyboard", { play: true });
    const operatorContext = await browser.newContext();
    const operator = await operatorContext.newPage();

    try {
      await test.step("the operator signs in by keyboard", async () => {
        await signInAsOperatorByKeyboard(operator);
        await operator.waitForURL(/\/admin$/, { timeout: 20_000 });
      });

      await test.step("and reaches Play pauses from the admin navigation", async () => {
        const nav = operator.getByTestId("admin-nav-play-pauses");
        await expect(nav).toBeVisible({ timeout: 20_000 });
        await tabTo(operator, nav, "admin navigation");
        await operator.keyboard.press("Enter");
        await operator.waitForURL(/\/admin\/play-pauses$/);
        await expect(
          operator.getByRole("heading", { name: "Play pauses", level: 1 }),
        ).toBeVisible({ timeout: 20_000 });
        await expectNoAxeViolations(operator);
      });

      const search = operator.getByRole("textbox", {
        name: "World name or id",
      });
      const candidate = operator.locator(
        `[data-testid="play-pause-candidate"][data-world-id="${table.worldId}"]`,
      );
      const pauseButton = candidate.getByRole("button", { name: "Pause play" });
      const dialog = operator.getByRole("dialog");
      const confirm = dialog.getByRole("button", { name: "Pause play" });
      const grounds = dialog.getByRole("textbox", { name: "Grounds" });

      await test.step("searches by name, and Enter submits the search", async () => {
        await tabTo(operator, search, "search field");
        await operator.keyboard.type(table.worldName);
        await operator.keyboard.press("Enter");
        await expect(candidate).toContainText(table.worldName, {
          timeout: 15_000,
        });
      });

      await test.step("opens the dialog with Enter; focus moves into it", async () => {
        await tabTo(operator, pauseButton, "the world's Pause play button");
        await operator.keyboard.press("Enter");
        await expect(dialog).toBeVisible();
        await expect
          .poll(() =>
            dialog.evaluate((el) => el.contains(document.activeElement)),
          )
          .toBe(true);
        await expectNoAxeViolations(
          operator,
          '[data-testid="play-pause-confirm"]',
        );
      });

      await test.step("whitespace grounds leave Confirm out of reach", async () => {
        if (!(await grounds.evaluate((el) => el === document.activeElement))) {
          await tabTo(operator, grounds, "grounds field");
        }
        await operator.keyboard.type("   ");
        await expect(confirm).toBeDisabled();
        // Round the whole dialog once: the disabled confirm is never a stop,
        // and focus never escapes the dialog while it is open.
        for (let stop = 0; stop < 6; stop += 1) {
          await operator.keyboard.press("Tab");
          await expectFocusVisible(operator, `dialog Tab stop ${stop + 1}`);
          expect(
            await dialog.evaluate((el) => el.contains(document.activeElement)),
            "focus stays inside the open dialog",
          ).toBe(true);
          expect(
            await confirm.evaluate((el) => el === document.activeElement),
          ).toBe(false);
        }
      });

      await test.step("Escape cancels, returns focus to the button, and pauses nothing", async () => {
        await operator.keyboard.press("Escape");
        await expect(dialog).toBeHidden();
        await expect(pauseButton).toBeFocused();
        await expectFocusVisible(operator, "after Escape");
        await expectNothingPaused(operator, table);
      });

      const written = `Journey grounds ${uniqueSuffix()}: by keyboard`;
      await test.step("Enter reopens it, and Enter on Pause play confirms", async () => {
        await operator.keyboard.press("Enter");
        await expect(dialog).toBeVisible();
        await expect(grounds).toHaveValue("");
        if (!(await grounds.evaluate((el) => el === document.activeElement))) {
          await tabTo(operator, grounds, "grounds field, reopened");
        }
        await operator.keyboard.type(written);
        await tabTo(operator, confirm, "the dialog's Pause play");
        await operator.keyboard.press("Enter");
        await expect(dialog).toBeHidden({ timeout: 15_000 });
        await expect(activeRow(operator, table.worldId)).toContainText(written);
        // Focus is not dropped on the page body when the dialog closes: it
        // goes to the sentence saying what happened, since the list the
        // dialog was opened from has been re-fetched underneath it.
        await expect(operator.getByTestId("play-pause-outcome")).toBeFocused();
        await expectFocusVisible(operator, "after confirming");
        const onServer = await activePausesOn(operator, table.worldId);
        expect(onServer.map((p) => p.grounds)).toEqual([written]);
      });

      await test.step("both playing tables land on the notice", async () => {
        for (const page of [table.gmPage, table.playerPage]) {
          await page.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
          await expectPausedNotice(page, table.worldName, { grounds: written });
        }
      });
    } finally {
      await closeAll(table, operator);
    }
  });

  test("a Game Master and a player get the ordinary refusal, and see no world list", async ({
    browser,
  }) => {
    test.setTimeout(240_000);
    const table = await seatATable(browser, "Refusal", { play: false });

    try {
      for (const [who, page] of [
        ["the Game Master", table.gmPage],
        ["the player", table.playerPage],
      ] as const) {
        await test.step(`${who} goes to /admin/play-pauses`, async () => {
          await page.goto("/admin/play-pauses");
          // `RequireAdmin`'s refusal: a signed-in non-administrator is sent
          // to /welcome, exactly as from every other admin page.
          await page.waitForURL(/\/welcome$/, {
            timeout: 20_000,
          });
          await expect(
            page.getByRole("heading", { name: "Play pauses" }),
          ).toHaveCount(0);
          await expect(page.getByTestId("admin-sidebar-nav")).toHaveCount(0);
          await expect(page.getByTestId("play-pause-search")).toHaveCount(0);
          await expect(page.getByTestId("play-pause-candidates")).toHaveCount(
            0,
          );
          await expect(page.getByTestId("play-pauses-active")).toHaveCount(0);

          // And the server holds the same line behind the route.
          const asked = await graphql<{
            data?: { playPauseCandidates?: unknown } | null;
            errors?: { message: string }[];
          }>(
            page,
            `
              query ($search: String!) {
                playPauseCandidates(search: $search, first: 5) {
                  id
                  name
                }
              }
            `,
            { search: table.worldName },
          );
          expect(asked.data?.playPauseCandidates ?? null).toBeNull();
          expect(asked.errors?.[0]?.message).toBe("Admin privileges required");
        });
      }
    } finally {
      await closeAll(table);
    }
  });
});
