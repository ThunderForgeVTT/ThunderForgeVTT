import { expect, test, type Page, type WebSocketRoute } from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import { expectNoAxeViolations } from "./fixtures/axe";
import {
  graphql,
  inviteAndJoinAsPlayer,
  uniqueSuffix,
} from "./fixtures/helpers";
import {
  createToken,
  createWorldAndPlay,
  currentUserId,
  dragToken,
  firstSceneId,
  giveTokenTo,
  register,
  serverTokenPosition,
  severableLink,
  tokenPosition,
  waitForEngineReady,
  waitForOffline,
  waitForTokenTrafficToSettle,
} from "./fixtures/offline";
import {
  expectPausedNotice,
  markDocument,
  pauseWorldAsOperator,
  type GqlAnswer,
} from "./fixtures/playPause";

/**
 * Spec 051 User Story 2 (T028, quickstart scenario 2): a pause holds.
 *
 * US1 proved an honest browser at the table leaves when a pause lands. This
 * proves the pause cannot be walked around:
 *
 *  - a browser opening a paused world's play is sent to the notice, not left
 *    on a playfield whose requests fail one by one (acceptance 1);
 *  - the player's own session, asking the server directly for every road into
 *    play, is refused `WORLD_PLAY_PAUSED` each time (acceptance 2, FR-021);
 *  - a browser that was offline when the pause landed is refused on its return
 *    and shown the notice (acceptance 3, FR-022), and the move it queued
 *    offline is refused `PLAY_PAUSED`, not applied, and the person is told
 *    (acceptance 4, FR-023);
 *  - the world's Owner cannot lift it (acceptance 5).
 *
 * # What "offline" is here
 *
 * `severableLink` cuts only the heartbeat, which is what the client's sense of
 * being connected is built on. On its own that leaves the page's WebSocket
 * and every other request working, and a page that still hears world event 28
 * and still has its streams ended was never offline when the pause landed: it
 * would leave by US1's road, and this test would prove US1 again. So for as
 * long as the link is cut, this page's `/api/ws` is refused with its open
 * sockets closed, and every other
 * GraphQL request it makes is aborted too. The direct calls are made from a
 * second tab of the same session, which none of that touches.
 */

type GqlError = { message: string; extensions?: Record<string, unknown> };

function codesOf(errors: GqlError[] | undefined): unknown[] {
  return (errors ?? []).map((error) => error.extensions?.code);
}

/**
 * Cut a page off from the server completely, heartbeat included, and let it
 * back. See the file comment for why the heartbeat alone is not enough here.
 *
 * Install before the page opens play: sockets are routed from the start.
 */
async function severablePage(page: Page) {
  const link = severableLink(page);
  await link.install();
  let severed = false;
  // Registered after `link.install()`, so it runs first. The heartbeat falls
  // through to the link, which aborts it and counts it: that count is what
  // `waitForOffline` waits on.
  await page.route("**/api/graphql", async (route) => {
    const body = route.request().postData() ?? "";
    if (severed && !body.includes("Heartbeat")) {
      await route.abort("internetdisconnected");
      return;
    }
    await route.fallback();
  });

  // Every socket goes through a route, so a cut can close the ones that are
  // open and refuse new ones until the link is restored. Proxied from the
  // start because a route only sees sockets opened after it is installed.
  // Closing sockets from inside the page and blocking `/api/ws` over CDP, as
  // live-sync.spec.ts does, was tried first here and did not hold: the client
  // was back on a socket within seconds, and heard the pause through it.
  const open = new Set<WebSocketRoute>();
  await page.routeWebSocket(/\/api\/ws/, (socket) => {
    if (severed) {
      void socket.close({ code: 4499, reason: "e2e sever" });
      return;
    }
    open.add(socket);
    const server = socket.connectToServer();
    socket.onMessage((message) => server.send(message));
    server.onMessage((message) => socket.send(message));
    socket.onClose((code, reason) => {
      open.delete(socket);
      void server.close({ code, reason });
    });
    server.onClose((code, reason) => {
      open.delete(socket);
      void socket.close({ code, reason });
    });
  });

  return {
    link,
    async cut() {
      severed = true;
      link.cut();
      // 4499 is graphql-ws's "Terminated": retried, not taken as final.
      for (const socket of [...open]) {
        open.delete(socket);
        await socket.close({ code: 4499, reason: "e2e sever" });
      }
    },
    restore() {
      severed = false;
      link.restore();
    },
  };
}

/**
 * The world subscriptions a page holds open, read off its own sockets.
 *
 * Every socket the page opens is proxied, and each `subscribe` the page sends
 * is kept until something ends it: the page's own `complete` (it let go), the
 * server's `error` or `complete` (the server ended it), or the socket closing.
 * Install before the page first opens play: a route only sees sockets opened
 * after it.
 */
async function trackWorldSubscriptions(page: Page) {
  type Held = { field: string; worldId: string | null; openedAt: number };
  const open = new Map<string, Held>();
  const ended: { field: string; by: string; heldMs: number }[] = [];
  let sockets = 0;

  const end = (key: string, by: string) => {
    const held = open.get(key);
    if (!held) return;
    open.delete(key);
    ended.push({ field: held.field, by, heldMs: Date.now() - held.openedAt });
  };
  const parse = (message: string | Buffer) => {
    try {
      return JSON.parse(String(message)) as {
        id?: string;
        type?: string;
        payload?: {
          query?: string;
          variables?: { worldId?: string };
          errors?: unknown[];
        };
      };
    } catch {
      return null;
    }
  };

  await page.routeWebSocket(/\/api\/ws/, (socket) => {
    const tag = `s${(sockets += 1)}`;
    const server = socket.connectToServer();
    socket.onMessage((message) => {
      const frame = parse(message);
      if (frame?.type === "subscribe" && frame.id) {
        const field =
          /\{\s*(\w+)\s*\(/.exec(frame.payload?.query ?? "")?.[1] ?? "?";
        open.set(`${tag}:${frame.id}`, {
          field,
          worldId: frame.payload?.variables?.worldId ?? null,
          openedAt: Date.now(),
        });
      } else if (frame?.type === "complete" && frame.id) {
        end(`${tag}:${frame.id}`, "page");
      }
      server.send(message);
    });
    server.onMessage((message) => {
      const frame = parse(message);
      if (
        frame?.id &&
        (frame.type === "error" ||
          frame.type === "complete" ||
          (frame.type === "next" && frame.payload?.errors?.length))
      ) {
        end(`${tag}:${frame.id}`, `server ${frame.type}`);
      }
      socket.send(message);
    });
    const closeAll = (by: string) => {
      for (const key of [...open.keys()]) {
        if (key.startsWith(`${tag}:`)) end(key, by);
      }
    };
    socket.onClose((code, reason) => {
      closeAll("page closed the socket");
      void server.close({ code, reason });
    });
    server.onClose((code, reason) => {
      closeAll("server closed the socket");
      void socket.close({ code, reason });
    });
  });

  return {
    /** `worldEventsCreated` subscriptions for `worldId` still open. */
    openWorldEvents: (worldId: string) =>
      [...open.values()].filter(
        (held) =>
          held.field === "worldEventsCreated" && held.worldId === worldId,
      ),
    ended: () => [...ended],
  };
}

/** Every `reconcileQueuedChanges` answer this page receives. */
function recordReconcileAnswers(
  page: Page,
): { localId: string; applied: boolean; reason: string | null }[][] {
  const answers: {
    localId: string;
    applied: boolean;
    reason: string | null;
  }[][] = [];
  page.on("response", async (response) => {
    const request = response.request();
    if (!request.url().includes("/api/graphql")) return;
    if (!(request.postData() ?? "").includes("ReconcileQueuedChanges")) return;
    try {
      const body = (await response.json()) as {
        data?: {
          reconcileQueuedChanges?: {
            localId: string;
            applied: boolean;
            reason: string | null;
          }[];
        } | null;
      };
      if (body.data?.reconcileQueuedChanges) {
        answers.push(body.data.reconcileQueuedChanges);
      }
    } catch {
      // A response the page abandoned has no body to read; nothing to record.
    }
  });
  return answers;
}

/**
 * Open `worldEventsCreated` directly, as whoever `page` is signed in as, and
 * return every `extensions.code` the server answers with before it ends the
 * subscription. Speaks `graphql-transport-ws` by hand, so nothing of the app's
 * own client (its retries, its handling of a pause) is in between.
 */
async function openWorldEventsDirectly(
  page: Page,
  worldId: string,
): Promise<{ codes: unknown[]; frames: unknown[]; ended: string }> {
  return page.evaluate(async (world) => {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const socket = new WebSocket(
      `${protocol}//${window.location.host}/api/ws`,
      "graphql-transport-ws",
    );
    const frames: unknown[] = [];
    const codes: unknown[] = [];
    const collect = (errors: unknown) => {
      if (!Array.isArray(errors)) return;
      for (const error of errors) {
        codes.push(
          (error as { extensions?: { code?: unknown } })?.extensions?.code,
        );
      }
    };
    return new Promise<{ codes: unknown[]; frames: unknown[]; ended: string }>(
      (resolve) => {
        let done = false;
        const finish = (ended: string) => {
          if (done) return;
          done = true;
          clearTimeout(timer);
          try {
            socket.close();
          } catch {
            // Already closed.
          }
          resolve({ codes, frames, ended });
        };
        const timer = setTimeout(() => finish("timeout"), 15_000);
        socket.onopen = () =>
          socket.send(JSON.stringify({ type: "connection_init", payload: {} }));
        socket.onmessage = (message) => {
          const frame = JSON.parse(String(message.data)) as {
            type: string;
            payload?: unknown;
          };
          frames.push(frame);
          if (frame.type === "connection_ack") {
            socket.send(
              JSON.stringify({
                id: "1",
                type: "subscribe",
                payload: {
                  query: `subscription ($worldId: String!) {
                    worldEventsCreated(worldId: $worldId) { id eventCode }
                  }`,
                  variables: { worldId: world },
                },
              }),
            );
          } else if (frame.type === "ping") {
            socket.send(JSON.stringify({ type: "pong" }));
          } else if (frame.type === "error") {
            collect(frame.payload);
            finish("error");
          } else if (frame.type === "next") {
            collect((frame.payload as { errors?: unknown })?.errors);
          } else if (frame.type === "complete") {
            finish("complete");
          }
        };
        socket.onclose = (event) => finish(`closed ${event.code}`);
      },
    );
  }, worldId);
}

test.describe("spec 051 US2: a pause holds", () => {
  test("a paused world refuses the page, the session, the offline browser and its queue, and its Owner", async ({
    browser,
  }) => {
    test.setTimeout(480_000);

    const suffix = uniqueSuffix();
    // The invite flow writes to the clipboard (see world-cache-offline.spec).
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();
    const worldName = `E2E Pause Holds ${suffix}`;
    const gmSubscriptions = await trackWorldSubscriptions(gmPage);

    await register(gmPage, "e2eholdgm");
    const worldId = await createWorldAndPlay(gmPage, worldName);
    await waitForEngineReady(gmPage);
    const sceneId = await firstSceneId(gmPage, worldId);
    const tokenId = await createToken(gmPage);

    const playerPage = await inviteAndJoinAsPlayer(
      browser,
      gmPage,
      worldId,
      "e2eholdpl",
    );
    const playerContext = playerPage.context();
    await giveTokenTo(gmPage, tokenId, await currentUserId(playerPage));

    const adminPage = await openAdminPage(browser);

    try {
      const player = await severablePage(playerPage);
      const reconcileAnswers = recordReconcileAnswers(playerPage);
      await playerPage.goto(`/world/${worldId}/play`);
      await waitForEngineReady(playerPage);
      await waitForTokenTrafficToSettle(playerPage);

      const before = await serverTokenPosition(gmPage, sceneId, tokenId);
      expect(
        before,
        "the token exists server-side before the cut",
      ).toBeTruthy();

      // 1. The player is cut off, and moves their token while cut off.
      await player.cut();
      await waitForOffline(playerPage, player.link);
      await dragToken(playerPage, tokenId, { dx: -170, dy: 110 });
      const intent = await tokenPosition(playerPage, tokenId);
      expect(
        intent,
        "the offline drag must move the player's view",
      ).toBeTruthy();
      expect(
        intent!.x,
        "the offline drag must actually move the token",
      ).not.toBe(before!.x);
      expect(
        (await serverTokenPosition(gmPage, sceneId, tokenId))!.x,
        "an offline move is queued, not written through",
      ).toBe(before!.x);
      await markDocument(playerPage);

      // 2. The operator pauses the world while the player cannot hear of it.
      const { pause } = await pauseWorldAsOperator(
        adminPage,
        worldId,
        `Operator grounds ${suffix}: holding`,
      );
      // The connected Game Master leaves by US1's road; not under test here,
      // but it must not be mistaken for the player having heard.
      await expect(gmPage).toHaveURL(/\/paused$/, { timeout: 15_000 });
      // T023: leaving play lets go of the world's event streams. The page
      // closes them itself; the server's five-second tick is the backstop for
      // a page that does not, not the mechanism for one that does. So the
      // budget is well inside a tick.
      const gmArrivedAt = Date.now();
      try {
        await expect
          .poll(() => gmSubscriptions.openWorldEvents(worldId).length, {
            timeout: 1_000,
            intervals: [50],
            message:
              "the notice must hold no worldEventsCreated subscription for the world",
          })
          .toBe(0);
      } catch (error) {
        console.log(
          `[play-pause-holds] still open at the notice: ${JSON.stringify(
            gmSubscriptions.openWorldEvents(worldId),
          )}`,
        );
        throw error;
      }
      console.log(
        `[play-pause-holds] Game Master's streams at the notice: none open ` +
          `${Date.now() - gmArrivedAt} ms after arrival; ended ` +
          JSON.stringify(
            gmSubscriptions
              .ended()
              .filter((entry) => entry.field === "worldEventsCreated"),
          ),
      );
      await playerPage.waitForTimeout(2_000);
      await expect(
        playerPage,
        "the cut-off player must not have heard of the pause",
      ).toHaveURL(/\/play$/);

      // 3. A connected browser opens the paused world's play: the notice, not
      // a playfield and not a load error.
      const opener = await gmContext.newPage();
      // Which roads the opener met, for the log. Not asserted: the heartbeat,
      // the event stream and the sync plan are all refused, and whichever
      // answers first sends the page on, often before the engine has asked
      // for its sync plan at all. Each road is proven on its own elsewhere
      // (the direct calls below, and the vitest cases for the sync summary).
      let syncPlanRequests = 0;
      const syncPlanCodes: unknown[] = [];
      opener.on("request", (request) => {
        if ((request.postData() ?? "").includes("worldSyncPlan")) {
          syncPlanRequests += 1;
        }
      });
      opener.on("response", async (response) => {
        if (!response.url().includes("/api/graphql")) return;
        if (!(response.request().postData() ?? "").includes("worldSyncPlan")) {
          return;
        }
        try {
          const body = (await response.json()) as { errors?: GqlError[] };
          syncPlanCodes.push(...codesOf(body.errors));
        } catch {
          // The page left before reading it; nothing to record.
        }
      });
      await opener.goto(`/world/${worldId}/play`);
      await expectPausedNotice(opener, worldName);
      await expect(opener).toHaveURL(new RegExp(`/world/${worldId}/paused$`));
      await expect(opener.getByTestId("play-paused-not-kept")).toHaveCount(0);
      console.log(
        `[play-pause-holds] opening play: ${syncPlanRequests} worldSyncPlan request(s), answered ${JSON.stringify(syncPlanCodes)}`,
      );

      // 4. The player's own session, straight at the server. A second tab of
      // the same session, which the cut does not touch.
      const direct = await playerContext.newPage();
      await direct.goto("/worlds");

      const heartbeat = await graphql<GqlAnswer<{ heartbeat: boolean }>>(
        direct,
        `
          mutation Heartbeat($worldId: UUID!, $sceneId: UUID) {
            heartbeat(worldId: $worldId, sceneId: $sceneId)
          }
        `,
        { worldId, sceneId },
      );
      expect(heartbeat.data?.heartbeat ?? null).toBeNull();
      expect(codesOf(heartbeat.errors)).toContain("WORLD_PLAY_PAUSED");

      const syncPlan = await graphql<GqlAnswer<{ worldSyncPlan: unknown }>>(
        direct,
        `
          query ($worldId: UUID!, $held: [HeldItemInput!]!) {
            worldSyncPlan(worldId: $worldId, held: $held) {
              evict
              canonicalVersion
            }
          }
        `,
        { worldId, held: [] },
      );
      expect(syncPlan.data?.worldSyncPlan ?? null).toBeNull();
      expect(codesOf(syncPlan.errors)).toContain("WORLD_PLAY_PAUSED");

      const move = await graphql<GqlAnswer<{ moveOwnToken: unknown }>>(
        direct,
        `
          mutation ($tokenId: UUID!, $x: Float!, $y: Float!) {
            moveOwnToken(tokenId: $tokenId, x: $x, y: $y) {
              tokenId
              x
              y
            }
          }
        `,
        { tokenId, x: before!.x + 320, y: before!.y + 320 },
      );
      expect(move.data?.moveOwnToken ?? null).toBeNull();
      expect(codesOf(move.errors)).toContain("WORLD_PLAY_PAUSED");

      const events = await openWorldEventsDirectly(direct, worldId);
      expect(
        events.codes,
        `worldEventsCreated must be refused as it opens: ${JSON.stringify(events.frames)}`,
      ).toContain("WORLD_PLAY_PAUSED");

      expect(
        (await serverTokenPosition(direct, sceneId, tokenId))!,
        "the refused direct move changed nothing",
      ).toEqual(before);

      // 5. The link comes back.
      player.restore();
      // Not `waitForOnline`: that waits for a heartbeat to get through, and
      // whether one ever does is a race the test does not own. The socket
      // reconnecting is refused too, and a page sent to the notice by that
      // road first stops beating before its next heartbeat is due. Either
      // road ends on the notice, which is what is waited for.

      await expect(playerPage).toHaveURL(
        new RegExp(`/world/${worldId}/paused$`),
        { timeout: 30_000 },
      );
      await expectPausedNotice(playerPage, worldName, { marked: true });

      await expect
        .poll(() => reconcileAnswers.flat().length, {
          timeout: 30_000,
          message: "the queued move must be submitted, and answered",
        })
        .toBeGreaterThan(0);
      const outcomes = reconcileAnswers.flat();
      console.log(
        `[play-pause-holds] reconcile answered ${JSON.stringify(outcomes)}`,
      );
      for (const outcome of outcomes) {
        expect(outcome.applied).toBe(false);
        expect(outcome.reason).toBe("PLAY_PAUSED");
      }

      await expect(playerPage.getByTestId("play-paused-not-kept")).toHaveText(
        "1 change you made while offline wasn't kept.",
        { timeout: 30_000 },
      );

      // Not applied, now or a moment later.
      await playerPage.waitForTimeout(3_000);
      expect(
        (await serverTokenPosition(direct, sceneId, tokenId))!,
        "nothing the offline browser queued may be applied",
      ).toEqual(before);

      // 6. The Owner cannot lift it. Only an operator can (FR-008).
      const lift = await graphql<GqlAnswer<{ liftWorldPlayPause: unknown }>>(
        gmPage,
        `
          mutation ($pauseId: UUID!, $grounds: String!) {
            liftWorldPlayPause(pauseId: $pauseId, grounds: $grounds) {
              id
              liftedAt
            }
          }
        `,
        { pauseId: pause.id, grounds: "An owner lifting their own pause." },
      );
      expect(lift.data?.liftWorldPlayPause ?? null).toBeNull();
      expect(lift.errors?.length ?? 0).toBeGreaterThan(0);
      const liftMessage = lift.errors?.[0]?.message ?? "";
      // Before US4 serves `liftWorldPlayPause` the schema has no such field,
      // and validation refuses the call before any resolver runs. Once it is
      // served, the refusal is the admin guard's. Either is a refusal of the
      // Owner; which one this run met is logged, so the day US4 lands the log
      // says the guard (not validation) is what refused.
      const unknownField = /Unknown field/i.test(liftMessage);
      if (!unknownField) {
        expect(liftMessage).toBe("Admin privileges required");
      }
      console.log(
        `[play-pause-holds] Owner's liftWorldPlayPause refused by ` +
          `${unknownField ? "schema validation (field not served yet)" : "the admin guard"}: ${liftMessage}`,
      );
      await expect(playerPage).toHaveURL(/\/paused$/);

      // 7. The notice, as the returning player sees it, passes axe.
      await expectNoAxeViolations(
        playerPage,
        '[data-testid="play-paused-notice"]',
      );
    } finally {
      await adminPage.context().close();
      await playerContext.close();
      await gmContext.close();
    }
  });
});
