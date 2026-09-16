import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Locator,
  type Page,
} from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  expectPausedNotice,
  fileSceneTakedown,
  fileTakedown,
  markDocument,
  pendingRequestFor,
} from "./fixtures/playPause";

/**
 * Spec 051 User Story 3 (T043, quickstart scenario 3): a takedown on content
 * of a world being played asks an operator to pause it, and nothing more
 * until one decides.
 *
 * Proven here, in a real browser against the real stack:
 *
 *  1. two notices filed through `/legal/dmca` — a scene the table is on, then
 *     an actor of the same world — gather into **one** pending request with
 *     two triggers, marked as being played (FR-030, FR-031, FR-033);
 *  2. approving it in the portal lands the table on the notice, as a pause
 *     from User Story 1 does (FR-034);
 *  3. declining a request for a second world leaves that table exactly where
 *     it was for ten seconds, sends it no event 28, records none, and leaves
 *     `worldPlayState.history` empty (FR-034, SC-005);
 *  4. two operators approving one request at once make one pause, and the
 *     one who lost is told who decided and when (FR-035);
 *  5. a takedown on a world nobody has played for over a minute raises no
 *     request (US3 acceptance 4).
 *
 * The notices are real notices through the real form, and the decisions are
 * made in the portal. GraphQL is used to set worlds up (a scene, an actor)
 * and to **check** what the server holds.
 */

interface CandidateRow {
  id: string;
  playedNow: boolean;
  paused: boolean;
}

interface WorldEventRow {
  id: string;
  eventCode: number;
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

async function createScene(
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

async function createNpc(
  page: Page,
  worldId: string,
  label: string,
): Promise<string> {
  const created = await graphql<{
    data?: { createActor?: { id: string } };
    errors?: unknown;
  }>(
    page,
    `
      mutation ($input: CreateActorInput!) {
        createActor(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, label, isNpc: true, gameSystemId: "genie" } },
  );
  const id = created.data?.createActor?.id;
  if (!id) {
    throw new Error(`createActor failed: ${JSON.stringify(created.errors)}`);
  }
  return id;
}

async function enterPlay(page: Page, worldId: string): Promise<void> {
  await page.goto(`/world/${worldId}/play`);
  await expect(page.locator("canvas")).toBeVisible({ timeout: 60_000 });
}

/** The operator's view of whether `worldId` is being played right now. */
async function candidate(
  adminPage: Page,
  worldId: string,
): Promise<CandidateRow> {
  const answer = await graphql<{
    data?: { playPauseCandidates?: CandidateRow[] };
    errors?: unknown;
  }>(
    adminPage,
    `
      query ($search: String!) {
        playPauseCandidates(search: $search, first: 5) {
          id
          playedNow
          paused
        }
      }
    `,
    { search: worldId },
  );
  const row = answer.data?.playPauseCandidates?.find((w) => w.id === worldId);
  if (!row) {
    throw new Error(
      `world ${worldId} is not a candidate: ${JSON.stringify(answer)}`,
    );
  }
  return row;
}

/** Wait until the server counts `worldId` as in live play (research R3). */
async function expectPlayedNow(
  adminPage: Page,
  worldId: string,
): Promise<void> {
  await expect
    .poll(async () => (await candidate(adminPage, worldId)).playedNow, {
      timeout: 30_000,
      message: "the table's heartbeat must mark its world as being played",
    })
    .toBe(true);
}

async function worldPlayState(
  member: Page,
  worldId: string,
): Promise<{ paused: boolean; history: unknown[] }> {
  const answer = await graphql<{
    data?: { worldPlayState?: { paused: boolean; history: unknown[] } };
    errors?: unknown;
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

/** Every event the world has recorded, as a member catches up on them. */
async function worldEventCodes(
  member: Page,
  worldId: string,
): Promise<number[]> {
  const answer = await graphql<{
    data?: { worldEventsSince?: { events: WorldEventRow[] } };
    errors?: unknown;
  }>(
    member,
    `
      query ($worldId: UUID!) {
        worldEventsSince(worldId: $worldId, afterId: 0) {
          events {
            id
            eventCode
          }
        }
      }
    `,
    { worldId },
  );
  if (!answer.data?.worldEventsSince) {
    throw new Error(
      `worldEventsSince did not answer: ${JSON.stringify(answer)}`,
    );
  }
  return answer.data.worldEventsSince.events.map((e) => e.eventCode);
}

/** Active pauses on `worldId`, with the request each came from. */
async function activePausesOn(
  adminPage: Page,
  worldId: string,
): Promise<{ id: string; requestId: string | null; grounds: string }[]> {
  const answer = await graphql<{
    data?: {
      playPauses?: {
        nodes: { id: string; requestId: string | null; grounds: string }[];
      };
    };
    errors?: unknown;
  }>(
    adminPage,
    `
      query ($worldId: UUID) {
        playPauses(active: true, worldId: $worldId, first: 10) {
          nodes {
            id
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

/** A decided request, read back from the record. */
async function decidedRequest(
  adminPage: Page,
  requestId: string,
  state: "APPROVED" | "DECLINED",
): Promise<{
  id: string;
  decidedBy: { name: string } | null;
  decisionNote: string | null;
} | null> {
  const answer = await graphql<{
    data?: {
      playPauseRequests?: {
        nodes: {
          id: string;
          decidedBy: { name: string } | null;
          decisionNote: string | null;
        }[];
      };
    };
  }>(
    adminPage,
    `
      query ($state: PauseRequestState) {
        playPauseRequests(state: $state, first: 200) {
          nodes {
            id
            decidedBy {
              name
            }
            decisionNote
          }
        }
      }
    `,
    { state },
  );
  return (
    answer.data?.playPauseRequests?.nodes.find((r) => r.id === requestId) ??
    null
  );
}

/** The portal's row for the pending request on `worldId`. */
function requestRow(adminPage: Page, worldId: string): Locator {
  return adminPage.locator(
    `[data-testid="play-pause-request"][data-world-id="${worldId}"]`,
  );
}

async function openPortal(adminPage: Page): Promise<void> {
  await adminPage.goto("/admin/play-pauses");
  await expect(
    adminPage.getByRole("heading", { name: "Play pauses", level: 1 }),
  ).toBeVisible({ timeout: 20_000 });
}

/** Open Approve or Decline on a request row, and write the note. */
async function openDecision(
  adminPage: Page,
  worldId: string,
  decision: "Approve" | "Decline",
  note: string,
): Promise<Locator> {
  const row = requestRow(adminPage, worldId);
  await expect(row).toBeVisible({ timeout: 20_000 });
  await row.getByRole("button", { name: new RegExp(`^${decision} `) }).click();
  const dialog = adminPage.getByTestId("play-pause-decide");
  await expect(dialog).toBeVisible();
  const submit = dialog.getByTestId("play-pause-decide-submit");
  // A note is required: the submit stays shut until one is written.
  await expect(submit).toBeDisabled();
  await dialog.getByLabel("Note").fill("   ");
  await expect(submit).toBeDisabled();
  await dialog.getByLabel("Note").fill(note);
  await expect(submit).toBeEnabled();
  return dialog;
}

/** Every world-event frame this page's sockets receive carrying code 28. */
function recordPauseEventFrames(page: Page): string[] {
  const frames: string[] = [];
  page.on("websocket", (socket) => {
    socket.on("framereceived", ({ payload }) => {
      const text = typeof payload === "string" ? payload : payload.toString();
      if (/"eventCode"\s*:\s*28\b/.test(text)) frames.push(text);
    });
  });
  return frames;
}

interface PlayedWorld {
  worldId: string;
  worldName: string;
  gmPage: Page;
  context: BrowserContext;
}

/** A Game Master creates a world and plays it. */
async function aWorldBeingPlayed(
  browser: Browser,
  label: string,
  prefix: string,
): Promise<PlayedWorld> {
  const context = await browser.newContext();
  const gmPage = await context.newPage();
  const worldName = `E2E ${label} ${uniqueSuffix()}`;
  const worldId = await registerAndCreateWorld(gmPage, worldName, prefix);
  return { worldId, worldName, gmPage, context };
}

test.describe("spec 051 US3: a takedown asks for a pause", () => {
  test("two takedowns on a played world make one request with both triggers; approving it pauses the table", async ({
    browser,
  }) => {
    test.setTimeout(360_000);
    const table = await aWorldBeingPlayed(browser, "Requested Table", "e2ereq");
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const adminPage = await openAdminPage(browser);

    try {
      const sceneId = await activeSceneOf(table.gmPage, table.worldId);
      const actorName = `Contested Golem ${uniqueSuffix()}`;
      const actorId = await createNpc(table.gmPage, table.worldId, actorName);

      await enterPlay(table.gmPage, table.worldId);
      await expectPlayedNow(adminPage, table.worldId);
      expect(await pendingRequestFor(adminPage, table.worldId)).toBeNull();

      const sceneCase = await fileSceneTakedown(
        claimant,
        sceneId,
        "The scene being played",
      );
      const actorCase = await fileTakedown(
        claimant,
        "Actor / NPC / character",
        actorId,
        actorName,
      );

      // FR-033: one request for the world, carrying both notices.
      const request = await pendingRequestFor(adminPage, table.worldId);
      expect(request, "the takedowns must raise a request").not.toBeNull();
      expect(request!.playedNow).toBe(true);
      expect(
        request!.triggers.map((t) => [t.kind, t.entityId, t.caseId]),
      ).toEqual([
        ["TAKEDOWN", sceneId, sceneCase],
        ["TAKEDOWN", actorId, actorCase],
      ]);

      await openPortal(adminPage);
      const row = requestRow(adminPage, table.worldId);
      await expect(row).toContainText(table.worldName, { timeout: 20_000 });
      await expect(row).toHaveCount(1);
      await expect(row.getByTestId("play-pause-request-played-now")).toHaveText(
        "Being played",
      );
      await expect(row.getByText(/^Raised /)).toBeVisible();
      const triggers = row.getByTestId("play-pause-request-trigger");
      await expect(triggers).toHaveCount(2);
      await expect(triggers.nth(0)).toContainText("Takedown");
      await expect(triggers.nth(0)).toContainText("on a scene");
      await expect(triggers.nth(1)).toContainText("on an actor");
      await expect(
        triggers.nth(0).getByTestId("play-pause-request-case-link"),
      ).toHaveAttribute("href", `/admin/moderation?case=${sceneCase}`);
      await expect(
        triggers.nth(1).getByTestId("play-pause-request-case-link"),
      ).toHaveAttribute("href", `/admin/moderation?case=${actorCase}`);
      await expectNoAxeViolations(adminPage);

      // Still nothing paused: a request pauses nothing (FR-032).
      expect(await activePausesOn(adminPage, table.worldId)).toEqual([]);
      await expect(table.gmPage).toHaveURL(/\/play$/);

      await markDocument(table.gmPage);
      const note = `Approved ${uniqueSuffix()}: the map is the publisher's`;
      const dialog = await openDecision(
        adminPage,
        table.worldId,
        "Approve",
        note,
      );
      await expectNoAxeViolations(
        adminPage,
        '[data-testid="play-pause-decide"]',
      );
      await dialog.getByTestId("play-pause-decide-submit").click();
      await expect(dialog).toBeHidden({ timeout: 15_000 });
      const outcome = adminPage.getByTestId("play-pause-outcome");
      await expect(outcome).toHaveText(`Play in ${table.worldName} is paused.`);
      await expect(outcome).toBeFocused();
      await expect(requestRow(adminPage, table.worldId)).toHaveCount(0);
      await expect(
        adminPage.locator(
          `[data-testid="play-pause-active"][data-world-id="${table.worldId}"]`,
        ),
      ).toContainText(note);

      // As in User Story 1: the table lands on the notice, and the note the
      // operator wrote is nowhere on it.
      await table.gmPage.waitForURL(/\/world\/[^/]+\/paused$/, {
        timeout: 15_000,
      });
      await expectPausedNotice(table.gmPage, table.worldName, {
        grounds: note,
        marked: true,
      });

      const [pause] = await activePausesOn(adminPage, table.worldId);
      expect(pause.requestId).toBe(request!.id);
      expect(pause.grounds).toBe(note);
      expect(await pendingRequestFor(adminPage, table.worldId)).toBeNull();
    } finally {
      await adminPage.context().close();
      await claimantContext.close();
      await table.context.close();
    }
  });

  test("declining a request changes nothing at the table: no navigation, no event 28, no history", async ({
    browser,
  }) => {
    test.setTimeout(300_000);
    const table = await aWorldBeingPlayed(browser, "Declined Table", "e2edecl");
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const adminPage = await openAdminPage(browser);

    try {
      // The notice lands on a scene nobody is looking at, so whatever the
      // takedown itself does to a page cannot be mistaken for the decision.
      const sideName = `Side Vault ${uniqueSuffix()}`;
      const sideScene = await createScene(
        table.gmPage,
        table.worldId,
        sideName,
      );
      const pauseFrames = recordPauseEventFrames(table.gmPage);

      await enterPlay(table.gmPage, table.worldId);
      await expectPlayedNow(adminPage, table.worldId);
      await fileSceneTakedown(claimant, sideScene, sideName);

      const request = await pendingRequestFor(adminPage, table.worldId);
      expect(request, "the takedown must raise a request").not.toBeNull();
      const eventsBefore = await worldEventCodes(table.gmPage, table.worldId);
      const urlBefore = table.gmPage.url();
      await markDocument(table.gmPage);

      await openPortal(adminPage);
      const note = `Declined ${uniqueSuffix()}: not in play at this table`;
      const dialog = await openDecision(
        adminPage,
        table.worldId,
        "Decline",
        note,
      );
      await expectNoAxeViolations(
        adminPage,
        '[data-testid="play-pause-decide"]',
      );
      await dialog.getByTestId("play-pause-decide-submit").click();
      await expect(dialog).toBeHidden({ timeout: 15_000 });
      await expect(adminPage.getByTestId("play-pause-outcome")).toHaveText(
        `The request to pause ${table.worldName} was declined. Nothing in the world changed.`,
      );
      await expect(requestRow(adminPage, table.worldId)).toHaveCount(0);

      // Ten seconds: two heartbeats and two server liveness ticks, each of
      // which would have carried a pause to this page had there been one.
      await table.gmPage.waitForTimeout(10_000);
      expect(table.gmPage.url()).toBe(urlBefore);
      await expect(table.gmPage.locator("canvas")).toBeVisible();
      await expect(table.gmPage.getByTestId("play-paused-notice")).toHaveCount(
        0,
      );
      const sameDocument = await table.gmPage.evaluate(
        () =>
          (window as unknown as { __e2eSameDocument?: boolean })
            .__e2eSameDocument === true,
      );
      expect(sameDocument, "the table's page must not have reloaded").toBe(
        true,
      );
      expect(pauseFrames, "no event 28 may reach the table").toEqual([]);

      // SC-005: nothing a member can reach shows a trace of it.
      const state = await worldPlayState(table.gmPage, table.worldId);
      expect(state.paused).toBe(false);
      expect(state.history).toEqual([]);
      const eventsAfter = await worldEventCodes(table.gmPage, table.worldId);
      expect(eventsAfter).not.toContain(28);
      expect(eventsAfter.length).toBeGreaterThanOrEqual(eventsBefore.length);

      // The operator's record keeps the decision (SC-006).
      expect(await activePausesOn(adminPage, table.worldId)).toEqual([]);
      const declined = await decidedRequest(adminPage, request!.id, "DECLINED");
      expect(declined?.decisionNote).toBe(note);
      expect(declined?.decidedBy?.name).toBeTruthy();

      // …and shows it on the portal's Record (T056): the world, the decline,
      // who decided and the note.
      await openPortal(adminPage);
      const recorded = adminPage.locator(
        `[data-testid="play-pause-record-request"][data-request-id="${request!.id}"]`,
      );
      await expect(recorded).toHaveAttribute("data-state", "DECLINED", {
        timeout: 15_000,
      });
      await expect(recorded).toContainText(table.worldName);
      await expect(recorded).toContainText(`Declined`);
      await expect(recorded).toContainText(`by ${declined!.decidedBy!.name}`);
      await expect(recorded).toContainText(note);
    } finally {
      await adminPage.context().close();
      await claimantContext.close();
      await table.context.close();
    }
  });

  test("two operators approving one request at once make one pause, and the second is told who decided", async ({
    browser,
  }) => {
    test.setTimeout(360_000);
    const table = await aWorldBeingPlayed(browser, "Raced Table", "e2erace");
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const firstOperator = await openAdminPage(browser);
    // A second context carrying the operator's signed-in session, rather than
    // a second sign-in: two second-factor sign-ins inside one TOTP step spend
    // the step, and `loginAsAdmin`'s retry then finds the challenge gone. The
    // race is between two browsers' requests, which this still is.
    const secondOperator = await (
      await browser.newContext({
        storageState: await firstOperator.context().storageState(),
      })
    ).newPage();

    try {
      const sceneId = await activeSceneOf(table.gmPage, table.worldId);
      await enterPlay(table.gmPage, table.worldId);
      await expectPlayedNow(firstOperator, table.worldId);
      await fileSceneTakedown(claimant, sceneId, "The raced scene");
      const request = await pendingRequestFor(firstOperator, table.worldId);
      expect(request, "the takedown must raise a request").not.toBeNull();

      await openPortal(firstOperator);
      await openPortal(secondOperator);
      const notes = [
        `First operator ${uniqueSuffix()}`,
        `Second operator ${uniqueSuffix()}`,
      ];
      const dialogs = [
        await openDecision(firstOperator, table.worldId, "Approve", notes[0]),
        await openDecision(secondOperator, table.worldId, "Approve", notes[1]),
      ];

      await Promise.all(
        dialogs.map((dialog) =>
          dialog.getByTestId("play-pause-decide-submit").click(),
        ),
      );
      for (const dialog of dialogs) {
        await expect(dialog).toBeHidden({ timeout: 15_000 });
      }

      const outcomes = await Promise.all(
        [firstOperator, secondOperator].map(async (page) => {
          const outcome = page.getByTestId("play-pause-outcome");
          await expect(outcome).toBeVisible();
          return (await outcome.innerText()).trim();
        }),
      );
      const won = outcomes.filter(
        (text) => text === `Play in ${table.worldName} is paused.`,
      );
      const lost = outcomes.filter((text) =>
        text.startsWith("Already decided by "),
      );
      expect(won, JSON.stringify(outcomes)).toHaveLength(1);
      expect(lost, JSON.stringify(outcomes)).toHaveLength(1);

      const [pause, ...others] = await activePausesOn(
        firstOperator,
        table.worldId,
      );
      expect(others, "exactly one pause").toEqual([]);
      expect(pause.requestId).toBe(request!.id);
      const approved = await decidedRequest(
        firstOperator,
        request!.id,
        "APPROVED",
      );
      expect(approved).not.toBeNull();
      // The loser is shown the winner's name and the moment, and the winner's
      // note is the pause's grounds.
      expect(notes).toContain(pause.grounds);
      expect(lost[0]).toMatch(
        new RegExp(
          `^Already decided by ${approved!.decidedBy!.name} at .+\\. The request to pause ${table.worldName} was approved\\.$`,
        ),
      );
      const loser = outcomes[0] === lost[0] ? firstOperator : secondOperator;
      await expect(loser.getByTestId("play-pause-outcome")).toHaveAttribute(
        "role",
        "status",
      );
      await expect(loser.getByRole("alert")).toHaveCount(0);

      await table.gmPage.waitForURL(/\/world\/[^/]+\/paused$/, {
        timeout: 15_000,
      });
      await expectPausedNotice(table.gmPage, table.worldName);
    } finally {
      await firstOperator.context().close();
      await secondOperator.context().close();
      await claimantContext.close();
      await table.context.close();
    }
  });

  test("a takedown on a world nobody has played for over a minute raises no request", async ({
    browser,
  }) => {
    test.setTimeout(300_000);
    const table = await aWorldBeingPlayed(browser, "Idle Table", "e2eidle");
    const claimantContext = await browser.newContext();
    const claimant = await claimantContext.newPage();
    const adminPage = await openAdminPage(browser);

    try {
      const sceneId = await activeSceneOf(table.gmPage, table.worldId);

      // The world has been played — so it has a live-play mark — and then
      // everyone left. The last heartbeat is the last moment anyone played.
      let lastBeatAt = 0;
      table.gmPage.on("request", (req) => {
        if (
          req.url().includes("/api/graphql") &&
          (req.postData() ?? "").includes("heartbeat(")
        ) {
          lastBeatAt = Date.now();
        }
      });
      await enterPlay(table.gmPage, table.worldId);
      await expectPlayedNow(adminPage, table.worldId);
      await table.gmPage.goto("about:blank");
      expect(lastBeatAt).toBeGreaterThan(0);

      // Why this waits in real time: "in live play" is a heartbeat mark in
      // the database, under 45 s old (research R3). Nothing a member can do
      // makes it older, and rewriting it with SQL would test the query, not
      // the table leaving. So the test lets the table be gone for over a
      // minute, as the scenario says, and first sees the server agree the
      // world is no longer played.
      await expect
        .poll(
          async () => (await candidate(adminPage, table.worldId)).playedNow,
          {
            timeout: 75_000,
            intervals: [5_000],
            message: "an abandoned world must stop counting as played",
          },
        )
        .toBe(false);
      const idleFor = Date.now() - lastBeatAt;
      if (idleFor < 61_000) {
        await adminPage.waitForTimeout(61_000 - idleFor);
      }

      await fileSceneTakedown(claimant, sceneId, "The abandoned scene");
      expect(await pendingRequestFor(adminPage, table.worldId)).toBeNull();
      expect(await activePausesOn(adminPage, table.worldId)).toEqual([]);

      await openPortal(adminPage);
      await expect(
        adminPage.getByRole("heading", { name: "Requests" }),
      ).toBeVisible();
      await expect(requestRow(adminPage, table.worldId)).toHaveCount(0);
    } finally {
      await adminPage.context().close();
      await claimantContext.close();
      await table.context.close();
    }
  });
});
