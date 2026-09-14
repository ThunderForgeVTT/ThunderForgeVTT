import { execFileSync } from "node:child_process";
import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Page,
} from "@playwright/test";
import {
  freshCredentials,
  loginAsAdmin,
  register,
  uniqueSuffix,
  type Credentials,
} from "./helpers";

/**
 * Spec 051 journeys (T068, T069): a table set up the way people set one up,
 * an operator who pauses it through the portal, and the checks a journey is
 * allowed to make of the server.
 *
 * `operator-pauses-through-the-portal.journey.spec.ts` (T067) wrote the first
 * copy of the table; it is kept there as written, because that journey is
 * *about* the operator's path and asserts every step of it. These journeys
 * need the same table and the same pause as preconditions, and three copies
 * of a join flow would drift.
 *
 * # UI only, except to check
 *
 * Everything a person does is done through the product. `psql` appears only to
 * **read** what the server holds, and reads the rows directly so "nothing
 * half-finished landed" can be answered by `updated_at` as well as by value: a
 * write that set a column to what it already was is still a write.
 */

export interface JourneyTable {
  worldId: string;
  worldName: string;
  /** The player's character, made through actor select. */
  characterName: string;
  gm: Credentials;
  player: Credentials;
  gmPage: Page;
  playerPage: Page;
  contexts: BrowserContext[];
}

/** A Game Master and a player at one world, both on the playfield. */
export async function seatATable(
  browser: Browser,
  label: string,
): Promise<JourneyTable> {
  const suffix = uniqueSuffix();
  const worldName = `Journey ${label} ${suffix}`;
  const characterName = `Wanderer ${suffix}`;
  // Clipboard permission because "Generate Join Link" also copies the link,
  // and a headless context without it throws inside that click.
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const playerContext = await browser.newContext();
  const gmPage = await gmContext.newPage();
  const playerPage = await playerContext.newPage();
  const gm = freshCredentials("jgm");
  const player = freshCredentials("jplayer");

  let worldId = "";
  await test.step(`the Game Master creates "${worldName}"`, async () => {
    await register(gmPage, gm);
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
    await register(playerPage, player);
    await playerPage.goto(new URL(joinLink).pathname);
    await playerPage.getByRole("button", { name: "Join Campaign" }).click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}/actor-select$`), {
      timeout: 20_000,
    });
    await playerPage.locator("#new-character-name").fill(characterName);
    await playerPage
      .getByRole("button", { name: /create and play as this character/i })
      .click();
    await playerPage.waitForURL(new RegExp(`/world/${worldId}$`), {
      timeout: 20_000,
    });
  });

  await test.step("both press Play", async () => {
    for (const page of [gmPage, playerPage]) {
      await enterPlayFromTheWorld(page, worldId);
    }
    await expect(gmPage.locator("canvas")).toBeVisible({ timeout: 90_000 });
    await expect(playerPage.locator("canvas")).toBeVisible({
      timeout: 90_000,
    });
  });

  return {
    worldId,
    worldName,
    characterName,
    gm,
    player,
    gmPage,
    playerPage,
    contexts: [gmContext, playerContext],
  };
}

/** From wherever `page` is, the world's dashboard, Enter world, then Play. */
export async function enterPlayFromTheWorld(
  page: Page,
  worldId: string,
): Promise<void> {
  if (!new URL(page.url()).pathname.endsWith(`/world/${worldId}/staging`)) {
    await page.goto(`/world/${worldId}`);
    await page.getByRole("link", { name: "Enter world" }).click();
    await page.waitForURL(/\/staging$/, { timeout: 15_000 });
  }
  await page.getByTestId("play-button").click();
  await page.waitForURL(new RegExp(`/world/${worldId}/play$`), {
    timeout: 15_000,
  });
}

/** The seeded operator, signed in through the login page with their code. */
export async function signInTheOperator(browser: Browser): Promise<Page> {
  const context = await browser.newContext();
  const operator = await context.newPage();
  await loginAsAdmin(operator);
  await operator.waitForURL(/\/admin$/, { timeout: 20_000 });
  return operator;
}

/**
 * Pause `worldName` through `/admin/play-pauses`, as T067 proves an operator
 * does: the admin navigation, a search by name, Pause play, grounds, confirm.
 *
 * Returns when the portal says so. Whether a table has *heard* is the
 * caller's to wait for.
 */
export async function pauseThroughThePortal(
  operator: Page,
  worldId: string,
  worldName: string,
  grounds: string,
): Promise<void> {
  // Into the portal afresh each time, from the admin home and its
  // navigation: an operator page left open across a whole journey is not
  // what is under test, and a fresh arrival is what a person does.
  await operator.goto("/admin");
  await operator.getByTestId("admin-nav-play-pauses").click();
  await operator.waitForURL(/\/admin\/play-pauses$/);
  await expect(
    operator.getByRole("heading", { name: "Play pauses", level: 1 }),
  ).toBeVisible({ timeout: 20_000 });

  const candidate = operator.locator(
    `[data-testid="play-pause-candidate"][data-world-id="${worldId}"]`,
  );
  await operator
    .getByRole("textbox", { name: "World name or id" })
    .fill(worldName);
  await operator.getByRole("button", { name: "Find world" }).click();
  await expect(candidate).toContainText(worldName, { timeout: 15_000 });
  await candidate.getByRole("button", { name: "Pause play" }).click();

  const dialog = operator.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await dialog.getByRole("textbox", { name: "Grounds" }).fill(grounds);
  await dialog.getByRole("button", { name: "Pause play" }).click();
  await expect(dialog).toBeHidden({ timeout: 15_000 });
  await expect(operator.getByTestId("play-pause-outcome")).toHaveText(
    `Play in ${worldName} is paused.`,
    { timeout: 15_000 },
  );
}

const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

/** A UUID, checked, for interpolation into a read. */
export function uuid(value: string): string {
  if (!UUID_PATTERN.test(value)) {
    throw new Error(`refusing to put a non-UUID into SQL: ${value}`);
  }
  return value;
}

/**
 * One read against this run's Postgres, rows as `|`-separated lines.
 *
 * Reads only. The journey lane's script names the container, so this reaches
 * the throwaway instance and never the dev database.
 */
export function readRows(statement: string): string[] {
  const container = process.env.THUNDERFORGE_POSTGRES_CONTAINER;
  if (!container) {
    throw new Error(
      "THUNDERFORGE_POSTGRES_CONTAINER is not set; use pnpm journeys",
    );
  }
  const output = execFileSync(
    "docker",
    [
      "exec",
      "-i",
      container,
      "psql",
      "-U",
      process.env.THUNDERFORGE_DB_USER ?? "postgres",
      "-d",
      process.env.THUNDERFORGE_DB_NAME ?? "thunderforge",
      "-v",
      "ON_ERROR_STOP=1",
      "-t",
      "-A",
    ],
    { input: statement, encoding: "utf-8", stdio: ["pipe", "pipe", "inherit"] },
  );
  return output
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
}

/** Whether the server holds a pause in force on `worldId`. */
export function pausedOnServer(worldId: string): boolean {
  return (
    readRows(
      `select count(*) from world_play_pauses where world_id = '${uuid(worldId)}' and lifted_at is null`,
    )[0] === "1"
  );
}

export interface ConsoleWatch {
  /** Forget what was seen so far; only what follows is judged. */
  mark(): void;
  /** Console `error` messages and uncaught page errors since the mark. */
  errors(): string[];
}

/**
 * Collect console errors and uncaught exceptions from `page`.
 *
 * Attached for the page's whole life, and read from a mark, so a journey
 * judges the moment a pause lands rather than an engine's first frames, which
 * are other specs' business.
 */
export function watchConsole(page: Page): ConsoleWatch {
  let seen: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error") {
      const where = message.location();
      seen.push(
        `console.error: ${message.text()}${where.url ? ` (${where.url}:${where.lineNumber})` : ""}`,
      );
    }
  });
  page.on("pageerror", (error) => {
    seen.push(`pageerror: ${error.stack ?? error.message}`);
  });
  return {
    mark() {
      seen = [];
    },
    errors() {
      return [...seen];
    },
  };
}

/**
 * `page` is on the notice and nothing of the playfield is left: no canvas, no
 * dialog, no overlay, no body left inert by a dialog that never closed, and
 * the heading holds focus.
 *
 * Deliberately not `expectPausedNotice`, which also asserts the page was not
 * reloaded: several ways back in *are* reloads.
 */
export async function expectOnlyTheNotice(
  page: Page,
  worldId: string,
  worldName: string,
): Promise<void> {
  await expect(page).toHaveURL(new RegExp(`/world/${worldId}/paused$`), {
    timeout: 20_000,
  });
  const notice = page.getByTestId("play-paused-notice");
  const heading = notice.getByRole("heading", {
    name: "Play is paused",
    level: 1,
  });
  await expect(heading).toBeVisible({ timeout: 15_000 });
  await expect(heading).toBeFocused();
  await expect(notice.getByRole("status")).toContainText(worldName, {
    timeout: 15_000,
  });
  await expect(
    page.getByRole("link", { name: "Go to your worlds" }),
  ).toBeVisible();

  // Nothing of the playfield: not the engine's canvas, not its dock.
  //
  // "Not shown", rather than "not in the document", on purpose. Bevy puts its
  // `<canvas>` straight under `<body>`, outside React, and the engine is kept
  // booted for the life of the tab (spec 009 research §1: tearing its canvas
  // handle down is not survivable). `WorldPage` hides it on unmount. So a tab
  // that was on the playfield keeps a hidden canvas, and a tab that never was
  // has none; neither may show one.
  await expect(page.locator("canvas:visible")).toHaveCount(0);
  const shownCanvas = await page.evaluate(() =>
    Array.from(document.querySelectorAll("canvas")).some((canvas) => {
      const rect = canvas.getBoundingClientRect();
      return (
        getComputedStyle(canvas).display !== "none" &&
        rect.width > 0 &&
        rect.height > 0
      );
    }),
  );
  expect(shownCanvas, "no engine canvas may be shown over the notice").toBe(
    false,
  );
  await expect(page.locator('[data-testid^="world-dock-tab-"]')).toHaveCount(0);
  // No stacked dialogs or overlays. The notice is not a dialog; any role=dialog
  // left on the page is one the playfield opened and never closed.
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(page.locator("[data-radix-popper-content-wrapper]")).toHaveCount(
    0,
  );
  await expect(page.locator(".token-panel-overlay")).toHaveCount(0);
  // A Radix dialog makes the rest of the page inert while open (pointer
  // events off the body, `aria-hidden` on its siblings). One unmounted in a
  // way that skipped its cleanup would leave a notice nobody can click.
  const inert = await page.evaluate(() => ({
    bodyPointerEvents: getComputedStyle(document.body).pointerEvents,
    hiddenRoots: Array.from(document.body.children)
      .filter(
        (el) =>
          el.getAttribute("aria-hidden") === "true" ||
          el.hasAttribute("data-aria-hidden") ||
          (el as HTMLElement).inert,
      )
      .map((el) => el.id || el.tagName.toLowerCase()),
  }));
  expect(inert.bodyPointerEvents, "the body must not be left inert").not.toBe(
    "none",
  );
  expect(inert.hiddenRoots, "no page root may be left aria-hidden").toEqual([]);
  // And the way out is actually reachable by a pointer: nothing sits over it.
  await page
    .getByRole("link", { name: "Go to your worlds" })
    .click({ trial: true, timeout: 5_000 });
}
