import { expect, test, type Page } from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import {
  ensureSidebarOpen,
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  expectPausedNotice,
  liftPauseAsOperator,
  markDocument,
  pauseWorldAsOperator,
  pauseWorldPlayRaw,
} from "./fixtures/playPause";

/**
 * Spec 051 User Story 1 (T018, quickstart scenario 1): an operator pauses a
 * world's play, and every browser playing it leaves the playfield.
 *
 * What is proven here, in a real browser against the real stack:
 *
 *  - two people on **different scenes** of one world are both removed within
 *    5 seconds of the operator confirming, without a reload, and both see the
 *    notice (SC-001). The time is measured per browser and logged;
 *  - a third browser playing another world is untouched;
 *  - a person who is not an operator, the world's own Owner included, is
 *    refused when calling `pauseWorldPlay` directly (FR-006);
 *  - an operator who is also a member of the world is removed like anyone
 *    else. The gate is blind to `is_admin` (ADR-100 decision 2).
 *
 * Which scene each browser is on is read from what it tells the server: the
 * heartbeat carries the scene a client is showing, every five seconds. That
 * is the server's own view of where a client is, rather than a guess from the
 * page.
 */

/** SC-001: removed "within seconds", which the plan pins at five. */
const REMOVAL_BUDGET_MS = 5_000;

/**
 * The budget for a browser that missed the event: one server liveness tick
 * (`LIVENESS_POLL`, five seconds in `session_lifetime.rs`) plus the round trip
 * and a route change. That is the slowest road left to such a browser, so it
 * bounds whichever road actually gets there first.
 */
const STREAM_ROAD_BUDGET_MS = 6_500;

/** The scene each heartbeat this page sends names, newest last. */
function recordHeartbeatScenes(page: Page): string[] {
  const scenes: string[] = [];
  page.on("request", (request) => {
    if (!request.url().includes("/api/graphql")) return;
    const body = request.postDataJSON() as {
      query?: string;
      variables?: { sceneId?: string | null };
    } | null;
    if (!body?.query?.includes("heartbeat(")) return;
    scenes.push(body.variables?.sceneId ?? "");
  });
  return scenes;
}

/** When each answered heartbeat came back, and whether it was accepted. */
function recordHeartbeatAnswers(page: Page): { at: number; ok: boolean }[] {
  const answers: { at: number; ok: boolean }[] = [];
  page.on("response", async (response) => {
    const request = response.request();
    if (!request.url().includes("/api/graphql")) return;
    const query = (request.postDataJSON() as { query?: string } | null)?.query;
    if (!query?.includes("heartbeat(")) return;
    const at = Date.now();
    try {
      const body = (await response.json()) as {
        data?: { heartbeat?: boolean } | null;
        errors?: unknown[];
      };
      answers.push({
        at,
        ok: !body.errors?.length && body.data?.heartbeat != null,
      });
    } catch {
      answers.push({ at, ok: false });
    }
  });
  return answers;
}

async function createSceneViaGraphql(
  page: Page,
  worldId: string,
  name: string,
): Promise<string> {
  const created = await graphql<{
    data?: { createScene?: { sceneId: string } };
    errors?: unknown;
  }>(
    page,
    `
      mutation CreateScene($input: GraphQLCreateSceneInput!) {
        createScene(input: $input) {
          sceneId
        }
      }
    `,
    { input: { worldId, name } },
  );
  const sceneId = created.data?.createScene?.sceneId;
  if (!sceneId) {
    throw new Error(`createScene failed: ${JSON.stringify(created.errors)}`);
  }
  return sceneId;
}

async function activeSceneOf(page: Page, worldId: string): Promise<string> {
  const answer = await graphql<{
    data?: { world?: { activeSceneId: string | null } };
  }>(
    page,
    `
      query W($id: UUID!) {
        world(id: $id) {
          activeSceneId
        }
      }
    `,
    { id: worldId },
  );
  const sceneId = answer.data?.world?.activeSceneId;
  if (!sceneId) throw new Error("the world has no active scene");
  return sceneId;
}

async function enterPlay(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
}

/** Wait until this page's heartbeat reports it is showing `sceneId`. */
async function expectHeartbeatScene(
  scenes: string[],
  sceneId: string,
  who: string,
): Promise<void> {
  await expect
    .poll(() => scenes.at(-1), {
      timeout: 30_000,
      message: `${who}'s heartbeat must name the scene it is showing`,
    })
    .toBe(sceneId);
}

/**
 * Open `/admin/play-pauses`, find `worldId`, and pause it with `grounds`.
 * Returns the moment the confirm was clicked, which is what removal is
 * measured from.
 */
async function pauseFromPortal(
  adminPage: Page,
  worldId: string,
  worldName: string,
  grounds: string,
): Promise<number> {
  await adminPage.goto("/admin/play-pauses");
  await expect(
    adminPage.getByRole("heading", { name: "Play pauses", level: 1 }),
  ).toBeVisible({ timeout: 20_000 });
  await adminPage.getByTestId("play-pause-search").fill(worldId);
  await adminPage.getByRole("button", { name: "Find world" }).click();

  const candidate = adminPage.locator(
    `[data-testid="play-pause-candidate"][data-world-id="${worldId}"]`,
  );
  await expect(candidate).toContainText(worldName, { timeout: 15_000 });
  await candidate.getByRole("button", { name: "Pause play" }).click();

  const dialog = adminPage.getByTestId("play-pause-confirm");
  await expect(dialog).toBeVisible();
  const confirm = dialog.getByTestId("play-pause-confirm-submit");
  // Grounds are required: the confirm stays shut until they are given.
  await expect(confirm).toBeDisabled();
  await dialog.getByLabel("Grounds").fill(grounds);
  await expect(confirm).toBeEnabled();

  const clickedAt = Date.now();
  await confirm.click();
  await expect(dialog).toBeHidden({ timeout: 15_000 });
  await expect(adminPage.getByTestId("play-pause-outcome")).toContainText(
    `Play in ${worldName} is paused.`,
  );
  await expect(
    adminPage.locator(
      `[data-testid="play-pause-active"][data-world-id="${worldId}"]`,
    ),
  ).toContainText(grounds);
  return clickedAt;
}

/**
 * Start watching for `page` to reach the notice, before the pause is made.
 * Resolves to the moment it arrived.
 */
function arrivalAtNotice(page: Page): Promise<number> {
  const arrival = page
    .waitForURL(/\/world\/[^/]+\/paused$/, {
      timeout: 30_000,
      waitUntil: "commit",
    })
    .then(() => Date.now());
  // Handled here as well as where it is awaited, so a failure earlier in the
  // test does not surface as an unhandled rejection after the contexts close.
  arrival.catch(() => undefined);
  return arrival;
}

/**
 * Words no member-facing surface about a pause may carry (FR-011, FR-050).
 * Checked against the pause's own elements, not the whole page, because the
 * app's chrome has other honest uses for some of them.
 */
const BLAME_WORDS = /takedown|report|violation|reason|grounds|abuse|infring/i;

/**
 * Spec 051 US5 (T056, T059): what the Game Master sees of a pause outside
 * the notice — that and when, and no reason — then the lift (T053) and the
 * history it leaves in the world's settings.
 */
async function expectMembersToldThatAndWhen(
  gmPage: Page,
  adminPage: Page,
  worldId: string,
  grounds: string,
): Promise<void> {
  // The world list: a status on the world's card.
  await gmPage.goto("/worlds");
  const card = gmPage.getByTestId("world-card-play-paused");
  await expect(card).toContainText(/paused by an operator since \S/, {
    timeout: 20_000,
  });
  await expect(card).not.toContainText(BLAME_WORDS);
  await expect(gmPage.locator("body")).not.toContainText(grounds);

  // The world page a member lands on: a quiet banner.
  await gmPage.goto(`/world/${worldId}/staging`);
  const banner = gmPage.getByTestId("world-play-paused-banner");
  await expect(banner).toContainText(
    /Play in this world has been paused by an operator since \S/,
    { timeout: 20_000 },
  );
  await expect(banner).toHaveAttribute("role", "status");
  await expect(banner).not.toContainText(BLAME_WORDS);
  await expect(gmPage.locator("body")).not.toContainText(grounds);

  // The lift reaches the notice without a reload (T053): the page asks again
  // on focus, and offers the way back.
  await gmPage.goto(`/world/${worldId}/paused`);
  await expect(
    gmPage.getByRole("heading", { name: "Play is paused" }),
  ).toBeVisible({
    timeout: 15_000,
  });
  await expect(
    gmPage.getByRole("link", { name: "Return to the world" }),
  ).toHaveCount(0);
  const active = await graphql<{
    data?: { playPauses?: { nodes: { id: string }[] } };
    errors?: unknown;
  }>(
    adminPage,
    `
      query ActivePause($worldId: UUID!) {
        playPauses(active: true, worldId: $worldId, first: 10) {
          nodes {
            id
          }
        }
      }
    `,
    { worldId },
  );
  const pauseId = active.data?.playPauses?.nodes[0]?.id;
  if (!pauseId) {
    throw new Error(`no active pause: ${JSON.stringify(active.errors)}`);
  }
  await liftPauseAsOperator(adminPage, pauseId, "Dealt with; play may resume.");
  await markDocument(gmPage);
  await gmPage.evaluate(() => window.dispatchEvent(new Event("focus")));
  const back = gmPage.getByRole("link", { name: "Return to the world" });
  await expect(back).toBeVisible({ timeout: 15_000 });
  await expect(
    gmPage.getByRole("heading", { name: "Play has resumed" }),
  ).toBeVisible();
  const sameDocument = await gmPage.evaluate(
    () =>
      (window as unknown as { __e2eSameDocument?: boolean })
        .__e2eSameDocument === true,
  );
  expect(sameDocument, "the lift must reach the notice without a reload").toBe(
    true,
  );
  await back.click();
  await expect(gmPage).toHaveURL(new RegExp(`/world/${worldId}/play$`));
  await expect(gmPage.locator("canvas")).toBeVisible({ timeout: 60_000 });
  // Back in play, and staying there: not sent back to the notice.
  await gmPage.waitForTimeout(6_000);
  await expect(gmPage).toHaveURL(new RegExp(`/world/${worldId}/play$`));

  // The world's settings: the history, times only.
  await gmPage.goto(`/world/${worldId}/settings/system`);
  const history = gmPage.getByTestId("play-pause-history-card");
  await expect(history).toBeVisible({ timeout: 20_000 });
  const rows = history.getByTestId("play-pause-history-row");
  await expect(rows).toHaveCount(1);
  await expect(rows.first().locator("time")).toHaveCount(2);
  await expect(history).not.toContainText("Still paused");
  await expect(history).not.toContainText(BLAME_WORDS);
  await expect(gmPage.locator("body")).not.toContainText(grounds);
  await expect(gmPage.getByTestId("world-play-paused-banner")).toHaveCount(0);
}

test.describe("spec 051 US1: pausing a world's play reaches the table", () => {
  test("two browsers on different scenes leave within 5 s; another world plays on; a non-operator is refused", async ({
    browser,
  }) => {
    test.setTimeout(420_000);

    const suffix = uniqueSuffix();
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const worldName = `E2E Paused Table ${suffix}`;
    const worldId = await registerAndCreateWorld(gmPage, worldName, "e2epause");
    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      gmPage,
      worldId,
      "e2epauseplayer",
    );

    // A third browser, a different person, a different world.
    const bystanderContext = await browser.newContext();
    const bystanderPage = await bystanderContext.newPage();
    const otherWorldName = `E2E Unpaused Table ${suffix}`;
    const otherWorldId = await registerAndCreateWorld(
      bystanderPage,
      otherWorldName,
      "e2epausebystander",
    );

    const adminPage = await openAdminPage(browser);

    try {
      // Two scenes: the one world creation launched, and a second.
      const sceneA = await activeSceneOf(gmPage, worldId);
      const sideName = `Side Chamber ${suffix}`;
      const sceneB = await createSceneViaGraphql(gmPage, worldId, sideName);

      const gmScenes = recordHeartbeatScenes(gmPage);
      const playerScenes = recordHeartbeatScenes(playerPage);
      const bystanderAnswers = recordHeartbeatAnswers(bystanderPage);

      await enterPlay(playerPage, worldId);
      await enterPlay(gmPage, worldId);
      await enterPlay(bystanderPage, otherWorldId);

      // The Game Master looks at the side chamber; the player stays on the
      // scene being played.
      await ensureSidebarOpen(gmPage);
      await gmPage.getByTestId("scene-switcher").click();
      await gmPage.getByRole("option", { name: sideName }).click();
      await expect(gmPage.getByTestId("scene-switcher")).toContainText(
        sideName,
        { timeout: 15_000 },
      );
      await expectHeartbeatScene(gmScenes, sceneB, "the Game Master");
      await expectHeartbeatScene(playerScenes, sceneA, "the player");
      expect(sceneA).not.toBe(sceneB);

      // FR-006: nobody but an operator can pause, the world's own Owner
      // included — asked directly, not through a hidden button.
      const byOwner = await pauseWorldPlayRaw(
        gmPage,
        worldId,
        "An owner trying to pause their own table.",
      );
      expect(byOwner.data?.pauseWorldPlay ?? null).toBeNull();
      expect(byOwner.errors?.[0]?.message).toBe("Admin privileges required");
      const byPlayer = await pauseWorldPlayRaw(
        playerPage,
        worldId,
        "A player trying to pause the table.",
      );
      expect(byPlayer.data?.pauseWorldPlay ?? null).toBeNull();
      expect(byPlayer.errors?.[0]?.message).toBe("Admin privileges required");
      // And the refused attempts paused nothing: both are still playing.
      await expect(gmPage).toHaveURL(/\/play$/);
      await expect(playerPage).toHaveURL(/\/play$/);

      await markDocument(gmPage);
      await markDocument(playerPage);
      await markDocument(bystanderPage);

      const grounds = `Operator grounds ${suffix}: live abuse at the table`;
      const gmArrival = arrivalAtNotice(gmPage);
      const playerArrival = arrivalAtNotice(playerPage);
      const confirmedAt = await pauseFromPortal(
        adminPage,
        worldId,
        worldName,
        grounds,
      );
      const gmMs = (await gmArrival) - confirmedAt;
      const playerMs = (await playerArrival) - confirmedAt;
      console.log(
        `[play-pause] removal after confirm: Game Master (scene B) ${gmMs} ms, ` +
          `player (scene A) ${playerMs} ms`,
      );
      test.info().annotations.push({
        type: "removal-ms",
        description: `gm=${gmMs} player=${playerMs}`,
      });
      expect(gmMs).toBeLessThanOrEqual(REMOVAL_BUDGET_MS);
      expect(playerMs).toBeLessThanOrEqual(REMOVAL_BUDGET_MS);

      await expectPausedNotice(gmPage, worldName, { grounds, marked: true });
      await expectPausedNotice(playerPage, worldName, {
        grounds,
        marked: true,
      });

      // Nothing on the playfield keeps beating for a paused world.
      const gmBeatsAtNotice = gmScenes.length;
      const playerBeatsAtNotice = playerScenes.length;

      // FR-022 / acceptance 3: the other world plays on. Give its heartbeat
      // two full beats past the pause, and require it still accepted.
      await bystanderPage.waitForTimeout(11_000);
      await expect(bystanderPage).toHaveURL(
        new RegExp(`/world/${otherWorldId}/play$`),
      );
      await expect(bystanderPage.getByTestId("play-paused-notice")).toHaveCount(
        0,
      );
      expect(
        bystanderAnswers.filter((a) => a.at > confirmedAt && a.ok).length,
        "the other world's heartbeat must keep being accepted",
      ).toBeGreaterThanOrEqual(2);
      expect(
        bystanderAnswers.filter((a) => a.at > confirmedAt && !a.ok),
      ).toEqual([]);
      const sameDocument = await bystanderPage.evaluate(
        () =>
          (window as unknown as { __e2eSameDocument?: boolean })
            .__e2eSameDocument === true,
      );
      expect(sameDocument).toBe(true);

      expect(gmScenes.length).toBe(gmBeatsAtNotice);
      expect(playerScenes.length).toBe(playerBeatsAtNotice);

      await expectMembersToldThatAndWhen(gmPage, adminPage, worldId, grounds);
    } finally {
      await adminPage.context().close();
      await bystanderContext.close();
      await playerPage.context().close();
      await gmContext.close();
    }
  });

  test("an operator who is a member of the world is removed from its play too", async ({
    browser,
  }) => {
    test.setTimeout(300_000);

    const suffix = uniqueSuffix();
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const worldName = `E2E Operator At The Table ${suffix}`;
    const worldId = await registerAndCreateWorld(
      gmPage,
      worldName,
      "e2epausemember",
    );

    const adminPage = await openAdminPage(browser);
    try {
      // The operator joins the world the way anyone does: by invite.
      const invite = await graphql<{
        data?: { generateInviteCode?: { inviteCode: string } };
        errors?: unknown;
      }>(
        gmPage,
        `
          mutation ($input: GenerateInviteCodeInput!) {
            generateInviteCode(input: $input) {
              inviteCode
            }
          }
        `,
        { input: { worldId, maxUses: 1 } },
      );
      const inviteCode = invite.data?.generateInviteCode?.inviteCode;
      if (!inviteCode) {
        throw new Error(`no invite: ${JSON.stringify(invite.errors)}`);
      }
      const joined = await graphql<{
        data?: { joinWorld?: { id: string } };
        errors?: unknown;
      }>(
        adminPage,
        `
          mutation ($input: JoinWorldInput!) {
            joinWorld(input: $input) {
              id
            }
          }
        `,
        { input: { inviteCode } },
      );
      expect(
        joined.data?.joinWorld?.id,
        `the operator must be able to join: ${JSON.stringify(joined.errors)}`,
      ).toBeTruthy();

      // One tab of the operator's session at the table, another in the portal.
      const operatorAtTable = await adminPage.context().newPage();
      await enterPlay(operatorAtTable, worldId);
      await enterPlay(gmPage, worldId);
      await markDocument(operatorAtTable);
      await markDocument(gmPage);

      const grounds = `Operator grounds ${suffix}: a compromised account`;
      const operatorArrival = arrivalAtNotice(operatorAtTable);
      const gmArrival = arrivalAtNotice(gmPage);
      const confirmedAt = await pauseFromPortal(
        adminPage,
        worldId,
        worldName,
        grounds,
      );
      const operatorMs = (await operatorArrival) - confirmedAt;
      const gmMs = (await gmArrival) - confirmedAt;
      console.log(
        `[play-pause] removal after confirm (operator is a member): ` +
          `operator ${operatorMs} ms, Game Master ${gmMs} ms`,
      );
      test.info().annotations.push({
        type: "removal-ms",
        description: `operator=${operatorMs} gm=${gmMs}`,
      });
      expect(operatorMs).toBeLessThanOrEqual(REMOVAL_BUDGET_MS);
      expect(gmMs).toBeLessThanOrEqual(REMOVAL_BUDGET_MS);

      await expectPausedNotice(operatorAtTable, worldName, {
        grounds,
        marked: true,
      });
      await expectPausedNotice(gmPage, worldName, { grounds, marked: true });
    } finally {
      await adminPage.context().close();
      await gmContext.close();
    }
  });

  test("a browser that never receives the pause event still leaves, by a refused request", async ({
    browser,
  }) => {
    test.setTimeout(300_000);

    const suffix = uniqueSuffix();
    const gmContext = await browser.newContext();
    const gmPage = await gmContext.newPage();
    const worldName = `E2E Missed Event ${suffix}`;
    const worldId = await registerAndCreateWorld(
      gmPage,
      worldName,
      "e2epausestream",
    );

    // Drop world event 28 on its way to this page, and pass everything else.
    // What is left is every road that answers `WORLD_PLAY_PAUSED`: a gated
    // HTTP request refused, a subscription refused as it opens, or a stream
    // ended on the server's next tick. The page has to treat each as a pause
    // rather than as a quiet end or a failure.
    //
    // Which road wins here is **not** the stream tick. Logged against a run
    // (2026-09-13, every request and frame timed from the pause): the page
    // has only just reached the playfield, and the world-cache sync it makes
    // on opening a world, `worldSyncPlan`, is answered `WORLD_PLAY_PAUSED`
    // (T039). `WorldPage` reads that out of the sync summary and the page is
    // on `/paused` 46 ms after the pause, before event 28 has even reached
    // the proxy (hence "0 withheld"), and seconds before any tick. That is
    // the 47-77 ms this logs. What this test proves is that a browser denied
    // the event still leaves, promptly, on a refusal. The tick itself — a
    // stream ended by the server with nothing else to go on — is proven in
    // `play-pause-stream-poll.spec.ts`, with a client that has no page logic.
    let droppedPauseEvents = 0;
    await gmPage.routeWebSocket(/\/api\/ws/, (socket) => {
      const server = socket.connectToServer();
      socket.onMessage((message) => server.send(message));
      server.onMessage((message) => {
        if (
          typeof message === "string" &&
          /"eventCode"\s*:\s*28\b/.test(message)
        ) {
          droppedPauseEvents += 1;
          return;
        }
        socket.send(message);
      });
    });

    const adminPage = await openAdminPage(browser);
    try {
      await enterPlay(gmPage, worldId);
      await markDocument(gmPage);

      const arrival = arrivalAtNotice(gmPage);
      const pausedAt = Date.now();
      await pauseWorldAsOperator(
        adminPage,
        worldId,
        `Operator grounds ${suffix}: stream road`,
      );
      const ms = (await arrival) - pausedAt;
      console.log(
        `[play-pause] removal with event 28 withheld: ${ms} ms ` +
          `(${droppedPauseEvents} pause event(s) withheld from the page)`,
      );
      test.info().annotations.push({
        type: "removal-ms",
        description: `stream-road=${ms}`,
      });
      // Not asserted on the count. The page may have left, and closed its
      // streams, before the event reached the proxy at all; that is the
      // refused sync winning outright (see above), not the filter missing.
      // The filter's match against the real frame (`{"type":"next",...,"eventCode":28,...}`)
      // was checked by hand against a logged run.
      expect(ms).toBeLessThanOrEqual(STREAM_ROAD_BUDGET_MS);
      await expectPausedNotice(gmPage, worldName, { marked: true });
    } finally {
      await adminPage.context().close();
      await gmContext.close();
    }
  });
});
