import { expect, type Browser, type BrowserContext, type Page } from "./test";
import { inviteAndJoinAsPlayer, uniqueSuffix } from "./helpers";
import { peerCounters } from "./world-cache";
import {
  createToken,
  createWorldAndPlay,
  firstSceneId,
  register,
  severableLink,
  tokenPosition,
  waitForEngineReady,
  waitForOffline,
  waitForTokenTrafficToSettle,
} from "./offline";

/**
 * A table of real clients with open peer channels, and the levers that take
 * the server or a peer away from it — shared by the peer-adjudicated play
 * specs (`world-cache-isolated-adjudication.spec.ts`). Why the levers are
 * these and not `context.setOffline` is in that spec's header.
 */

/** The connectivity state the app is showing, `"live"` when it shows none. */
export async function syncStatus(page: Page): Promise<string> {
  const indicator = page.locator("[data-sync-status]");
  if ((await indicator.count()) === 0) return "live";
  return (await indicator.first().getAttribute("data-sync-status")) ?? "live";
}

/**
 * Wait for a client to reach a connectivity state.
 *
 * Polled rather than awaited on a locator, because `live` is the *absence* of
 * the indicator and an assertion phrased as "not visible" would pass while
 * the page was still loading.
 */
/**
 * Assert adjudicated play has ended, without pinning which non-live label
 * the client happens to be wearing.
 *
 * Leaving `server-isolated` is the requirement (FR-058, FR-059). Whether the
 * client then reads `reconnecting` or `disconnected` is a question about how
 * many reconnect attempts it has made, which is a different subject
 * entirely — and "reconnecting" is the honest answer moments after an
 * outage, because it genuinely is still trying. Pinning the exact word made
 * a test fail on correct behaviour.
 *
 * The assertion that carries the requirement is never this one anyway: it is
 * that a move no longer reaches the other clients, which every caller checks
 * immediately after.
 */
export async function expectAdjudicationEnded(page: Page, message: string) {
  await expect
    .poll(() => syncStatus(page), { timeout: 90_000, message })
    .not.toBe("server-isolated");
  await expect
    .poll(() => syncStatus(page), { timeout: 90_000, message })
    .not.toBe("live");
}

export async function expectSyncStatus(
  page: Page,
  status: string,
  message: string,
) {
  await expect
    .poll(() => syncStatus(page), {
      timeout: 90_000,
      message:
        status === "server-isolated"
          ? `${message}. If this client never leaves "reconnecting" or ` +
            '"disconnected", check that the ' +
            "served WebAssembly engine exports `begin_peer_adjudication`: every one " +
            "of the Phase 10a bindings is optional-chained in `engine/bevy/index.ts` " +
            "and answers `false` when the bundle predates them, which leaves " +
            "`gmReachable` permanently false and this state unreachable"
          : message,
    })
    .toBe(status);
}

/**
 * Give a context a way to lose its peers without losing anything else.
 *
 * Installed as an init script, deliberately outside the application: nothing
 * in the shipped code has a "drop every peer" switch, and adding one to make a
 * test possible would put the failure mode being tested into the product. This
 * is the same technique `world-cache-peer.spec.ts` uses to serve corrupt bytes,
 * for the same reason.
 *
 * Closing the `RTCPeerConnection` closes its data channel at both ends, so one
 * client calling this is a genuine two-sided partition: the client loses the
 * others, and the others lose it. Nothing else about the client changes —
 * its page, its store and its outbox are all untouched, which is what makes
 * "did play stop?" a question about the rule rather than about the browser.
 */
export async function installPeerCutter(
  context: BrowserContext,
): Promise<void> {
  await context.addInitScript(() => {
    const live = new Set<RTCPeerConnection>();
    const Original = window.RTCPeerConnection;
    class Watched extends Original {
      constructor(...args: ConstructorParameters<typeof RTCPeerConnection>) {
        super(...args);
        live.add(this);
      }
    }
    window.RTCPeerConnection = Watched as unknown as typeof RTCPeerConnection;
    (window as unknown as { __severPeers: () => number }).__severPeers = () => {
      const count = live.size;
      for (const pc of live) {
        try {
          pc.close();
        } catch {
          // Already gone; the count is what the caller wanted anyway.
        }
      }
      live.clear();
      return count;
    };
  });
}

/** Drop every peer connection this page holds, and say how many there were. */
export async function severPeers(page: Page): Promise<number> {
  const closed = await page.evaluate(
    () =>
      (window as unknown as { __severPeers?: () => number }).__severPeers?.() ??
      0,
  );
  expect(closed, "the cutter must have had connections to cut").toBeGreaterThan(
    0,
  );
  return closed;
}

/**
 * Assert that a token does *not* move on this client over a real interval.
 *
 * Written as a sustained check rather than a single read because the failure
 * it guards against — an adjudicated move arriving after play should have
 * stopped — is a race, and a race that a single well-timed read would miss.
 */
export async function expectNoMovement(
  page: Page,
  tokenId: string,
  at: { x: number; y: number },
  message: string,
): Promise<void> {
  for (let i = 0; i < 8; i += 1) {
    await page.waitForTimeout(1_500);
    const now = await tokenPosition(page, tokenId);
    expect(now?.x, message).toBeCloseTo(at.x, 0);
    expect(now?.y, message).toBeCloseTo(at.y, 0);
  }
}

export interface Seat {
  context: BrowserContext;
  page: Page;
  /** Installed before this seat's last navigation — see `seatTable`. */
  link: ReturnType<typeof severableTransport>;
}

export interface Table {
  gm: Seat;
  players: Seat[];
  worldId: string;
  sceneId: string;
  tokenId: string;
  /** Every seat, the Game Master first. */
  seats: Seat[];
}

/**
 * A Game Master and `playerCount` players, all in the same world, all on the
 * scene, all holding an open data channel to each other.
 *
 * The last part is the precondition every test here depends on and none of
 * them could establish afterwards: peer signaling rides GraphQL, so a channel
 * that has not formed before the link is cut will never form.
 */
export async function seatTable(
  browser: Browser,
  prefix: string,
  playerCount: number,
): Promise<Table> {
  // The invite flow writes to the clipboard, and a context without that
  // permission throws before the new invite is stored — see
  // scene-live-launch.spec.ts, which hit the same thing.
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  await register(gmPage, `${prefix}gm`);
  const worldId = await createWorldAndPlay(
    gmPage,
    `E2E Isolated ${uniqueSuffix()}`,
  );
  await waitForEngineReady(gmPage);
  const sceneId = await firstSceneId(gmPage, worldId);
  const tokenId = await createToken(gmPage);

  const players: Seat[] = [];
  for (let i = 0; i < playerCount; i += 1) {
    const page = await inviteAndJoinAsPlayer(
      browser,
      gmPage,
      worldId,
      `${prefix}p${i}`,
    );
    players.push({
      context: page.context(),
      page,
      link: severableTransport(page),
    });
  }

  const seats: Seat[] = [
    { context: gmContext, page: gmPage, link: severableTransport(gmPage) },
    ...players,
  ];
  // Before the navigation below, so the wrapper is in place when the peer
  // fabric builds its connections.
  for (const seat of seats) await installPeerCutter(seat.context);
  for (const seat of seats) await seat.link.install();

  const everyone = seats.map((seat) => seat.page);
  for (const page of everyone) {
    await page.goto(`/world/${worldId}/play`);
    await waitForEngineReady(page);
  }
  for (const page of everyone) {
    await waitForTokenTrafficToSettle(page);
  }

  // The channels, before anything is severed. Everybody must see everybody:
  // `connectedPeers` is what the world page turns into the participant roster,
  // and a roster short by one makes full connectivity true by accident.
  for (const page of everyone) {
    await expect
      .poll(() => peerCounters(page).then((c) => c.peers), {
        timeout: 120_000,
        message:
          "every client must hold an open channel to every other before the cut",
      })
      .toBeGreaterThanOrEqual(everyone.length - 1);
  }

  return { gm: seats[0], players, worldId, sceneId, tokenId, seats };
}

/**
 * Take the server away from one client while leaving its peers untouched.
 *
 * # Why the heartbeat alone is not enough, and what is added
 *
 * Blocking the heartbeat is what makes the client *believe* the server is
 * gone: `connectivityFor`'s `serverUnreachable` comes from `heartbeat.ts` and
 * from nowhere else. But believing it is not the same as reporting it.
 * `refreshLiveSyncState` may only enter or leave `server-isolated`, deferring
 * every other transition to the socket's own callbacks — so with a healthy
 * socket the reported state stays `live`, `WorldPage`'s peer publisher keeps
 * taking its `live` branch, `beginPeerAdjudication` is never called, and the
 * third state cannot be reached however long the beats are refused.
 *
 * A real outage takes the socket with it and the `closed` callback does the
 * nudging. Here it cannot: routing `/api/ws` through Playwright's WebSocket
 * proxy was tried and it stops the peer fabric from ever connecting — peer
 * signaling rides that socket, and with it proxied `connectedPeers` never
 * leaves zero. That kills the precondition the whole file depends on.
 *
 * So the nudge is the browser's own `offline` event, dispatched into the page.
 * `subscriptionClient` listens for it and re-judges — and the case it is
 * documented as existing for is precisely this one: "a machine that has lost
 * its route to the internet may still have every peer at the table on the
 * local network, and that case is `server-isolated`". `navigator.onLine` is
 * left alone, so nothing here fakes the *verdict*; the refused heartbeats
 * remain the only evidence the client has, and the event only makes it look.
 */
export function severableTransport(page: Page) {
  const link = severableLink(page);
  return {
    async install() {
      await link.install();
    },
    /** Refuse heartbeats. The client's verdict follows from these alone. */
    cutHeartbeat() {
      link.cut();
    },
    /** Ask the page to re-judge, the way a machine losing its route would. */
    async nudge() {
      await page.evaluate(() => window.dispatchEvent(new Event("offline")));
    },
    restore() {
      link.restore();
    },
    blockedBeats: () => link.blockedBeats(),
    deliveredBeats: () => link.deliveredBeats(),
    reconcileRequests: () => link.reconcileRequests(),
  };
}

/**
 * Cut every seat off from the server and wait for each to notice.
 *
 * Beats are refused first and the nudge comes last, in that order for every
 * seat: the client must already have decided the server is unreachable before
 * it re-judges, or it re-judges to `live` and nothing has happened.
 */
export async function isolateTable(seats: Seat[]): Promise<void> {
  for (const seat of seats) seat.link.cutHeartbeat();
  for (const seat of seats) await waitForOffline(seat.page, seat.link);
  for (const seat of seats) await seat.link.nudge();
}

export async function closeTable(table: Table) {
  for (const player of table.players) {
    await player.context.close().catch(() => {});
  }
  await table.gm.context.close().catch(() => {});
}
