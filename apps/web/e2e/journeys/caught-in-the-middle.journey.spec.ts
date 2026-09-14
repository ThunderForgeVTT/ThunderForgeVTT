import { expect, test, type Page } from "@playwright/test";
import { openDockTab, uniqueSuffix } from "../fixtures/helpers";
import {
  AIM_OFFSETS,
  canvasBox,
  createToken,
  tokenPosition,
  waitForEngineReady,
  waitForTokenTrafficToSettle,
} from "../fixtures/offline";
import {
  expectOnlyTheNotice,
  pausedOnServer,
  pauseThroughThePortal,
  readRows,
  seatATable,
  signInTheOperator,
  uuid,
  watchConsole,
  type ConsoleWatch,
  type JourneyTable,
} from "../fixtures/journeyTable";

/**
 * Spec 051 T068, US2 as people live it: a pause lands while somebody at the
 * table is half-way through something.
 *
 * Four moments, each on its own table because a world is paused once:
 *
 * - the Game Master has a token held on the pointer, pressed and moved, not
 *   released;
 * - a character's sheet is open, and an edit has been typed and not kept;
 * - the Game Master has pressed "advance the turn" and the server has not
 *   answered yet;
 * - a player has typed a chat message and not sent it.
 *
 * Every time, the notice replaces the playfield with nothing of the playfield
 * left over it (no dialog, no overlay, no inert body), no console error and
 * no uncaught exception, and focus on the notice's heading. And the server
 * proves the half-done thing did not land: the rows are read back with their
 * `updated_at`, so a write that happened to store the same value is caught as
 * well as one that stored a new one.
 *
 * # UI only
 *
 * The table is set up through the product and the operator pauses through the
 * portal in their own browser (`../fixtures/journeyTable.ts`). `psql` reads
 * the server's rows; nothing here writes to the server except through a page.
 * The one piece of test machinery on a page is in the combat moment, and is
 * explained there.
 */

/** Long enough for a heartbeat (5 s) and a stream's liveness tick (5 s) to pass. */
const SETTLE_AFTER_PAUSE_MS = 6_000;

let operator: Page;

test.beforeAll(async ({ browser }) => {
  operator = await signInTheOperator(browser);
});

test.afterAll(async () => {
  await operator?.context().close();
});

/** Watches on both pages of a table, begun as soon as the pages exist. */
function watchTable(table: JourneyTable): ConsoleWatch[] {
  return [watchConsole(table.gmPage), watchConsole(table.playerPage)];
}

/**
 * The console errors a pause landing may legitimately produce.
 *
 * Empty on purpose. Anything that turns up here must be named, with the
 * reason it is not a defect, rather than filtered by a broad pattern.
 */
const ALLOWED_CONSOLE_ERRORS: RegExp[] = [];

function expectNoConsoleErrors(watches: ConsoleWatch[], where: string): void {
  const errors = watches
    .flatMap((watch) => watch.errors())
    .filter((line) => !ALLOWED_CONSOLE_ERRORS.some((ok) => ok.test(line)));
  expect(errors, `${where}: no console errors while the pause lands`).toEqual(
    [],
  );
}

/** Pause the table's world through the portal, and wait for both to hear. */
async function pauseAndLand(table: JourneyTable, label: string): Promise<void> {
  await test.step("the operator pauses the world through the portal", async () => {
    await pauseThroughThePortal(
      operator,
      table.worldId,
      table.worldName,
      `Journey grounds ${uniqueSuffix()}: ${label}`,
    );
    expect(pausedOnServer(table.worldId)).toBe(true);
  });
  await test.step("both at the table land on the notice, and only the notice", async () => {
    for (const page of [table.gmPage, table.playerPage]) {
      await page.waitForURL(/\/world\/[^/]+\/paused$/, { timeout: 15_000 });
    }
    for (const page of [table.gmPage, table.playerPage]) {
      await expectOnlyTheNotice(page, table.worldId, table.worldName);
    }
  });
}

async function closeTable(table: JourneyTable): Promise<void> {
  for (const context of table.contexts) await context.close();
}

/**
 * Press on `tokenId` and move it by `delta`, and **do not release**.
 *
 * `dragToken`'s aim, without its release: the press is searched outward from
 * where this client's store has the token, and confirmed by the selection it
 * produced while the button is still down, so the held drag is known to be a
 * drag of this token and not a press on empty canvas.
 */
async function pressAndHoldToken(
  page: Page,
  tokenId: string,
  delta: { dx: number; dy: number },
): Promise<void> {
  const box = await canvasBox(page);
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  for (let round = 0; round < 4; round += 1) {
    const at = await tokenPosition(page, tokenId);
    if (!at) throw new Error(`token ${tokenId} is not in this client's store`);
    for (const offset of AIM_OFFSETS) {
      const from = { x: cx + at.x + offset.dx, y: cy - at.y + offset.dy };
      if (
        from.x < box.x + 8 ||
        from.x > box.x + box.width - 8 ||
        from.y < box.y + 8 ||
        from.y > box.y + box.height - 8
      ) {
        continue;
      }
      await page.mouse.move(from.x, from.y);
      await page.mouse.down();
      // A press and a move in one frame make a zero drag offset; see
      // `dragCanvas` in ../fixtures/offline.ts.
      await page.waitForTimeout(250);
      const grabbed = await page.evaluate(
        () => window.__worldProbe?.state().selectedTokenId ?? null,
      );
      if (grabbed !== tokenId) {
        await page.mouse.up();
        await page.waitForTimeout(80);
        continue;
      }
      await page.mouse.move(from.x + delta.dx, from.y + delta.dy, {
        steps: 5,
      });
      await page.waitForTimeout(250);
      return;
    }
    await page.waitForTimeout(2_000);
  }
  throw new Error(`no press within a grid cell of ${tokenId} grabbed it`);
}

test.describe("spec 051 US2 journey: a pause catches the table in the middle of something", () => {
  test("a token is mid-drag: the notice replaces the playfield and the token stays where the server had it", async ({
    browser,
  }) => {
    test.setTimeout(420_000);
    const table = await seatATable(browser, "Drag");
    const watches = watchTable(table);
    try {
      const gm = table.gmPage;
      let tokenId = "";
      let before: string[] = [];

      await test.step("the Game Master puts a token on the map and picks it up", async () => {
        await waitForEngineReady(gm);
        tokenId = await createToken(gm);
        await waitForTokenTrafficToSettle(gm);
        before = readRows(
          `select x, y, updated_at from tokens where token_id = '${uuid(tokenId)}'`,
        );
        expect(before, "the token exists on the server").toHaveLength(1);
        await pressAndHoldToken(gm, tokenId, { dx: 192, dy: 96 });
        // Still held: the engine has the token selected under a pressed
        // button, and nothing has been sent, because a move is sent on
        // release.
        expect(
          await gm.evaluate(
            () => window.__worldProbe?.state().selectedTokenId ?? null,
          ),
        ).toBe(tokenId);
      });

      for (const watch of watches) watch.mark();
      await pauseAndLand(table, "mid-drag");

      await test.step("letting go on the notice sends nothing, and the token has not moved", async () => {
        // The person lets go of the button a beat after the page changed
        // under them: on the notice, where there is nothing to drop onto.
        await gm.mouse.up();
        await gm.waitForTimeout(SETTLE_AFTER_PAUSE_MS);
        await expectOnlyTheNotice(gm, table.worldId, table.worldName);
        expect(
          readRows(
            `select x, y, updated_at from tokens where token_id = '${uuid(tokenId)}'`,
          ),
          "position and updated_at are exactly as before the drag",
        ).toEqual(before);
        expectNoConsoleErrors(watches, "mid-drag");
      });
    } finally {
      await closeTable(table);
    }
  });

  /**
   * # There is no character sheet dialog on the playfield
   *
   * The task names "a character sheet dialog open with an unsaved edit". The
   * product has no such thing, and this test does not pretend it does:
   *
   * - A **player's** character sheet on the playfield is
   *   `InPaneCharacterSheet`, inside the dock, and it is mounted read-only
   *   (`canEdit: false`, spec 031 US2). It can be open; it cannot hold an
   *   edit.
   * - A **Game Master's** "View" opens the full actor page in a new tab
   *   (`ActorsPanel.tsx`), which is not the playfield and is not a dialog.
   *
   * So the moment is split into the two real things closest to it, at once:
   * the player has their own sheet open in the dock, and the Game Master has
   * the playfield's one editing dialog open — Token Management, with a
   * token's details popover over it and a photo URL typed into it and not
   * committed (the field commits on blur). That is a dialog and a popover
   * stacked, holding an unsaved edit to a creature on the map, which is what
   * "no stacked dialogs remain" is about.
   */
  test("a sheet is open and an edit is typed but not kept: the notice replaces both, and nothing was saved", async ({
    browser,
  }) => {
    test.setTimeout(420_000);
    const table = await seatATable(browser, "Sheet");
    const watches = watchTable(table);
    try {
      const gm = table.gmPage;
      const player = table.playerPage;
      let tokenId = "";
      let actorId = "";
      let tokenBefore: string[] = [];
      let actorBefore: string[] = [];
      const unsaved = `https://example.test/half-typed-${uniqueSuffix()}.png`;

      await test.step("the player opens their own character sheet in the dock", async () => {
        actorId = readRows(
          `select id from world_actors where world_id = '${uuid(table.worldId)}' and label = '${table.characterName.replace(/'/g, "''")}'`,
        )[0];
        expect(actorId, "the player's character exists").toBeTruthy();
        await waitForEngineReady(player);
        await openDockTab(player, "actors");
        await player.getByTestId(`actor-view-${uuid(actorId)}`).click();
        await expect(player.getByTestId("in-pane-character-sheet")).toBeVisible(
          { timeout: 20_000 },
        );
        actorBefore = readRows(
          `select label, description, updated_at from world_actors where id = '${uuid(actorId)}'`,
        );
      });

      await test.step("the Game Master types a token's photo URL in Token Management, and does not leave the field", async () => {
        await waitForEngineReady(gm);
        tokenId = await createToken(gm);
        await gm.getByTestId("token-panel-toggle-button").click();
        await expect(gm.getByRole("dialog")).toBeVisible({ timeout: 10_000 });
        await gm.getByTestId(`token-list-item-${tokenId}`).click();
        const photo = gm.getByTestId(`token-photo-input-${tokenId}`);
        await expect(photo).toBeVisible({ timeout: 10_000 });
        // Focused, then typed key by key. Not clicked: a pointer press on
        // this field is intercepted before it arrives (Playwright reports
        // `<html> intercepts pointer events` for the whole action timeout),
        // which is why `token-authoring.spec.ts` drives the same panel with
        // `force` and `fill`. That is TokenPanel's own defect, not the
        // pause's, and is reported with this journey rather than fixed here.
        await photo.focus();
        await gm.keyboard.type(unsaved, { delay: 5 });
        await expect(photo).toBeFocused();
        await expect(photo).toHaveValue(unsaved);
        tokenBefore = readRows(
          `select coalesce(photo_url, '<none>'), updated_at from tokens where token_id = '${uuid(tokenId)}'`,
        );
        expect(tokenBefore[0]).toMatch(/^<none>\|/);
      });

      for (const watch of watches) watch.mark();
      await pauseAndLand(table, "unsaved sheet edit");

      await test.step("the typed photo URL and the sheet were not saved", async () => {
        await gm.waitForTimeout(SETTLE_AFTER_PAUSE_MS);
        expect(
          readRows(
            `select coalesce(photo_url, '<none>'), updated_at from tokens where token_id = '${uuid(tokenId)}'`,
          ),
          "the token's photo and updated_at are as before",
        ).toEqual(tokenBefore);
        expect(
          readRows(
            `select label, description, updated_at from world_actors where id = '${uuid(actorId)}'`,
          ),
          "the character is as before",
        ).toEqual(actorBefore);
        await expect(gm.locator("body")).not.toContainText(unsaved);
        expectNoConsoleErrors(watches, "unsaved sheet edit");
      });
    } finally {
      await closeTable(table);
    }
  });

  /**
   * # Holding the Game Master's press in flight
   *
   * "The Game Master is advancing a combat turn" is a moment a few
   * milliseconds long: the click, then the server's answer. A pause landing
   * inside it by luck would be a test that sometimes tests nothing. So the
   * page's own `AdvanceTurn` request — made by the button, with the body the
   * button built — is held at the browser's network layer until the pause is
   * in force on the server, and then let go unchanged. Nothing is forged,
   * rewritten or sent by the test; the hold only decides *when* the page's
   * request reaches the server, which is the one thing a journey cannot ask a
   * person to control.
   */
  test("the Game Master is advancing the turn: the notice replaces the playfield and the turn did not advance", async ({
    browser,
  }) => {
    test.setTimeout(420_000);
    const table = await seatATable(browser, "Combat");
    const watches = watchTable(table);
    // Outside the try, so a failure while the press is held still lets it go
    // before the contexts close.
    let letGo: () => void = () => undefined;
    try {
      const gm = table.gmPage;
      const combatRow = () =>
        readRows(
          `select id, round, coalesce(active_combatant_id::text, '<none>'), updated_at from world_combats where world_id = '${uuid(table.worldId)}' and ended_at is null`,
        );
      let before: string[] = [];

      await test.step("the Game Master starts combat, adds the player's character and takes the first turn", async () => {
        await waitForEngineReady(gm);
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
        before = combatRow();
        expect(before, "one running combat").toHaveLength(1);
      });

      const release = new Promise<void>((resolve) => {
        letGo = resolve;
      });
      let heldResolve: () => void = () => undefined;
      const held = new Promise<void>((resolve) => {
        heldResolve = resolve;
      });
      const answered = gm.waitForResponse(
        (response) =>
          response.url().includes("/api/graphql") &&
          (response.request().postData() ?? "").includes(
            "mutation AdvanceTurn",
          ),
        { timeout: 120_000 },
      );

      await test.step("the Game Master presses advance, and the press is in flight", async () => {
        await gm.route("**/api/graphql", async (route) => {
          if (
            (route.request().postData() ?? "").includes("mutation AdvanceTurn")
          ) {
            heldResolve();
            await release;
            await route.continue();
            return;
          }
          await route.fallback();
        });
        await gm.getByTestId("advance-turn-button").click();
        await held;
      });

      for (const watch of watches) watch.mark();
      await pauseAndLand(table, "mid-turn");

      await test.step("the press reaches the server after the pause, is refused, and the turn is where it was", async () => {
        letGo();
        const response = await answered;
        const body = (await response.json()) as {
          data?: { advanceTurn?: unknown } | null;
          errors?: { extensions?: { code?: string } }[];
        };
        expect(body.data?.advanceTurn ?? null).toBeNull();
        expect(body.errors?.[0]?.extensions?.code).toBe("WORLD_PLAY_PAUSED");
        await gm.unroute("**/api/graphql");
        await gm.waitForTimeout(SETTLE_AFTER_PAUSE_MS);
        await expectOnlyTheNotice(gm, table.worldId, table.worldName);
        expect(
          combatRow(),
          "round, active combatant and updated_at are exactly as before",
        ).toEqual(before);
        expectNoConsoleErrors(watches, "mid-turn");
      });
    } finally {
      letGo();
      await closeTable(table);
    }
  });

  test("a player has typed a chat message and not sent it: the notice replaces the playfield and the log is unchanged", async ({
    browser,
  }) => {
    test.setTimeout(420_000);
    const table = await seatATable(browser, "Chat");
    const watches = watchTable(table);
    try {
      const player = table.playerPage;
      const draft = `Half a thought ${uniqueSuffix()}`;
      const chatLog = () =>
        readRows(
          `select count(*), coalesce(max(updated_at)::text, '<none>') from world_chat_messages where world_id = '${uuid(table.worldId)}'`,
        );
      let before: string[] = [];

      await test.step("the player opens chat and types, without sending", async () => {
        await waitForEngineReady(player);
        await openDockTab(player, "chat");
        await expect(player.getByTestId("chat-panel")).toBeVisible({
          timeout: 15_000,
        });
        const input = player.getByTestId("chat-input");
        await input.click();
        await input.pressSequentially(draft, { delay: 10 });
        await expect(input).toHaveValue(draft);
        before = chatLog();
      });

      for (const watch of watches) watch.mark();
      await pauseAndLand(table, "unsent chat");

      await test.step("pressing Enter a beat too late sends nothing, and the log is as it was", async () => {
        // Where a person's Enter would have gone: focus is on the notice's
        // heading now, and Enter there does nothing.
        await player.keyboard.press("Enter");
        await player.waitForTimeout(SETTLE_AFTER_PAUSE_MS);
        await expectOnlyTheNotice(player, table.worldId, table.worldName);
        expect(chatLog(), "message count and last update unchanged").toEqual(
          before,
        );
        expect(
          readRows(
            `select count(*) from world_chat_messages where world_id = '${uuid(table.worldId)}' and body = '${draft.replace(/'/g, "''")}'`,
          ),
        ).toEqual(["0"]);
        expectNoConsoleErrors(watches, "unsent chat");
      });
    } finally {
      await closeTable(table);
    }
  });
});
