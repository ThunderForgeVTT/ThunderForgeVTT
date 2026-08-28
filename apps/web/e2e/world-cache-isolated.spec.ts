import {
  expect,
  test,
  type Browser,
  type BrowserContext,
  type Page,
} from "@playwright/test";
import {
  graphql,
  inviteAndJoinAsPlayer,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { peerCounters, sceneIds } from "./fixtures/world-cache";
import {
  createToken,
  createWorldAndPlay,
  currentUserId,
  dragToken,
  firstSceneId,
  register,
  serverTokenPosition,
  severableLink,
  tokenPosition,
  waitForEngineReady,
  waitForOffline,
  waitForOnline,
  waitForTokenTrafficToSettle,
} from "./fixtures/offline";

/**
 * Peer-adjudicated play (spec 028 Phase 10a, T105–T108c, ADR-052 §4).
 *
 * # Producing a server-isolated client is an ordering problem
 *
 * "Server-isolated" means three things at once: the server is unreachable,
 * **every** peer is reachable, and the Game Master is among them. Only one
 * ordering produces it. The clients connect normally first, so signaling —
 * which rides GraphQL, i.e. the server — can form the WebRTC data channels;
 * the test waits until those channels are actually open; and only then is
 * GraphQL severed. The channels are direct peer-to-peer and survive it.
 *
 * `context.setOffline(true)` is the wrong lever twice over: it breaks Vite's
 * dynamic module fetches and crashes the page with an unrelated error, and it
 * would take the peer connections down with it — which is the exact opposite
 * of the state under test. The heartbeat route from `fixtures/offline.ts` is
 * the right one, and counting refused beats means these tests measure the
 * same quantity the client's own verdict is built on.
 *
 * Refusing beats is not on its own enough to move the client's *reported*
 * state, and `severableTransport` documents what is added and why — including
 * why the obvious answer, routing the WebSocket too, cannot be used here.
 *
 * # What these tests need from the build, and what they say when it is missing
 *
 * All but one of the tests below need a build that has Phase 10a in it, and
 * both ways of not having one are easy to end up with — a rebuilt server with
 * a stale engine bundle looks exactly like a working stack until a client is
 * asked to adjudicate:
 *
 * - Every peer-adjudication binding (`begin_peer_adjudication`,
 *   `peer_adjudication_active`, `propose_token_transform`, …) is
 *   optional-chained in `engine/bevy/index.ts` and answers `false` when
 *   `dist/engine` does not export it. Nothing errors; `gmReachable` is simply
 *   never true and `server-isolated` is never reached, so T105–T107 fail on
 *   the state assertion. Its message says so.
 * - `attributedToUserId`, `reportedOutcome` and `discrepancy` are schema, so a
 *   server built before T102/T102c refuses the query outright — which is at
 *   least an honest error, and the one T108–T108c report.
 *
 * # One thing the product does not yet connect
 *
 * The server computes a discrepancy in `reconcileQueuedChanges`, and
 * `RollResult.tsx` renders one. Nothing carries it between them:
 * `api/reconcile.ts` does not select the field, `ReconcileOutcome` has no
 * place for it, and `rollDice` has no such field at all. So the last test here
 * puts the comparison on the wire itself and asserts on the display and the
 * role gate, which is the half that exists; the transport is noted rather than
 * pretended.
 */

/** The connectivity state the app is showing, `"live"` when it shows none. */
async function syncStatus(page: Page): Promise<string> {
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
async function expectAdjudicationEnded(page: Page, message: string) {
  await expect
    .poll(() => syncStatus(page), { timeout: 90_000, message })
    .not.toBe("server-isolated");
  await expect
    .poll(() => syncStatus(page), { timeout: 90_000, message })
    .not.toBe("live");
}

async function expectSyncStatus(page: Page, status: string, message: string) {
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
async function installPeerCutter(context: BrowserContext): Promise<void> {
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
async function severPeers(page: Page): Promise<number> {
  const closed = await page.evaluate(() =>
    (window as unknown as { __severPeers?: () => number }).__severPeers?.() ?? 0,
  );
  expect(closed, "the cutter must have had connections to cut").toBeGreaterThan(0);
  return closed;
}

/**
 * Assert that a token does *not* move on this client over a real interval.
 *
 * Written as a sustained check rather than a single read because the failure
 * it guards against — an adjudicated move arriving after play should have
 * stopped — is a race, and a race that a single well-timed read would miss.
 */
async function expectNoMovement(
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

interface Seat {
  context: BrowserContext;
  page: Page;
  /** Installed before this seat's last navigation — see `seatTable`. */
  link: ReturnType<typeof severableTransport>;
}

interface Table {
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
async function seatTable(browser: Browser, prefix: string, playerCount: number): Promise<Table> {
  // The invite flow writes to the clipboard, and a context without that
  // permission throws before the new invite is stored — see
  // scene-live-launch.spec.ts, which hit the same thing.
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gmPage = await gmContext.newPage();
  await register(gmPage, `${prefix}gm`);
  const worldId = await createWorldAndPlay(gmPage, `E2E Isolated ${uniqueSuffix()}`);
  await waitForEngineReady(gmPage);
  const sceneId = await firstSceneId(gmPage, worldId);
  const tokenId = await createToken(gmPage);

  const players: Seat[] = [];
  for (let i = 0; i < playerCount; i += 1) {
    const page = await inviteAndJoinAsPlayer(browser, gmPage, worldId, `${prefix}p${i}`);
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
        message: "every client must hold an open channel to every other before the cut",
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
function severableTransport(page: Page) {
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
async function isolateTable(seats: Seat[]): Promise<void> {
  for (const seat of seats) seat.link.cutHeartbeat();
  for (const seat of seats) await waitForOffline(seat.page, seat.link);
  for (const seat of seats) await seat.link.nudge();
}

async function closeTable(table: Table) {
  for (const player of table.players) {
    await player.context.close().catch(() => {});
  }
  await table.gm.context.close().catch(() => {});
}


// ---------------------------------------------------------------------------
// The trust model (T108–T108c)
// ---------------------------------------------------------------------------

/**
 * Everything below submits to `reconcileQueuedChanges` directly, over a real
 * signed-in session, rather than driving a browser through an outage.
 *
 * That is not a shortcut around end-to-end: it is the only way to reach what
 * SC-021 and SC-021a–c are about. The rules under test — who may attribute a
 * change to whom, and when a reported outcome counts as a discrepancy — live
 * entirely in the server's answer to this one mutation, and the two inputs
 * that decide them (`attributedToUserId`, `reportedOutcome`) cannot be
 * produced from the UI at all: `api/reconcile.ts` sends neither, and no client
 * code path constructs one. Waiting for a queue to drain would exercise a
 * submission that carries none of the fields being tested.
 *
 * What *is* end to end here is everything that matters to the rules: real
 * accounts, real world membership, real roles, real tokens, real dice records
 * the server rolled itself, and the real authorization the mutation performs.
 */

/** What the server says became of one submitted change. */
interface ReconcileOutcomeWire {
  localId: string;
  applied: boolean;
  reason: string | null;
  supersededByRole: string | null;
  discrepancy: {
    userId: string;
    recordId: string;
    reportedValue: number;
    determinedValue: number;
  } | null;
}

interface QueuedChange {
  localId: string;
  command: unknown;
  attributedToUserId?: string;
  reportedOutcome?: {
    kind: string;
    version: number;
    recordId?: string | null;
    value?: number | null;
  };
}

const RECONCILE = `
  mutation ReconcileQueuedChanges($worldId: UUID!, $changes: [QueuedChangeInput!]!) {
    reconcileQueuedChanges(worldId: $worldId, changes: $changes) {
      localId
      applied
      reason
      supersededByRole
      discrepancy { userId recordId reportedValue determinedValue }
    }
  }
`;

/** Submit queued changes as whoever `page` is signed in as. */
async function reconcile(
  page: Page,
  worldId: string,
  changes: QueuedChange[],
): Promise<ReconcileOutcomeWire[]> {
  const res = await graphql<{
    data?: { reconcileQueuedChanges?: ReconcileOutcomeWire[] };
    errors?: unknown;
  }>(page, RECONCILE, { worldId, changes });
  expect(
    res.errors,
    `reconcileQueuedChanges failed: ${JSON.stringify(res.errors)}`,
  ).toBe(undefined);
  const outcomes = res.data?.reconcileQueuedChanges;
  expect(outcomes, "the server must answer with one outcome per change").toHaveLength(
    changes.length,
  );
  return outcomes!;
}

/** A move command in exactly the shape the outbox stores and replays. */
function moveCommand(tokenId: string, x: number, y: number) {
  return { type: "upsert_token", token: { id: tokenId, x, y } };
}

/** Create a token straight through the mutation, with no engine in the way. */
async function newToken(page: Page, sceneId: string, x = 0, y = 0): Promise<string> {
  const res = await graphql<{
    data?: { createToken?: { tokenId: string } };
    errors?: unknown;
  }>(
    page,
    `mutation ($input: GraphQLCreateTokenInput!) {
       createToken(input: $input) { tokenId }
     }`,
    { input: { sceneId, x, y } },
  );
  expect(res.errors, `createToken failed: ${JSON.stringify(res.errors)}`).toBe(undefined);
  const tokenId = res.data?.createToken?.tokenId;
  expect(tokenId, "the fixture must actually create a token").toBeTruthy();
  return tokenId!;
}

/** Hand a token to a user, so a non-GM submitter is entitled to move it. */
async function ownToken(page: Page, tokenId: string, userId: string): Promise<void> {
  const res = await graphql<{
    data?: { updateToken?: { ownerUserId: string | null } };
    errors?: unknown;
  }>(
    page,
    `mutation ($tokenId: UUID!, $input: GraphQLUpdateTokenInput!) {
       updateToken(tokenId: $tokenId, input: $input) { tokenId ownerUserId }
     }`,
    { tokenId, input: { ownerUserId: userId } },
  );
  expect(res.errors, `updateToken failed: ${JSON.stringify(res.errors)}`).toBe(undefined);
  expect(res.data?.updateToken?.ownerUserId).toBe(userId);
}

/**
 * Roll dice for real, and read back what the server recorded.
 *
 * The determined value has to come from the server's own record rather than
 * from anything the test computes: a discrepancy is by definition a
 * disagreement with `world_roll_records`, and a test that supplied both
 * numbers would be checking subtraction.
 */
async function rollAndRecord(
  roller: Page,
  gm: Page,
  worldId: string,
): Promise<{ recordId: string; determined: number }> {
  const rolled = await graphql<{
    data?: { rollDice?: { resultValue: number } };
    errors?: unknown;
  }>(
    roller,
    `mutation ($input: RollDiceInput!) {
       rollDice(input: $input) { formula resultValue }
     }`,
    { input: { worldId, formula: "3d20+7" } },
  );
  expect(rolled.errors, `rollDice failed: ${JSON.stringify(rolled.errors)}`).toBe(
    undefined,
  );
  const determined = rolled.data?.rollDice?.resultValue;
  expect(determined, "the server must have resolved the roll").toBeGreaterThan(0);

  // The record id is only readable by the Game Master (spec 014 FR-014), which
  // is also the only session that would ever be submitting one of these.
  const records = await graphql<{
    data?: { worldRollRecords?: { id: string; resolution: { resultValue: number } }[] };
    errors?: unknown;
  }>(
    gm,
    `query ($worldId: UUID!) {
       worldRollRecords(worldId: $worldId, limit: 5) {
         id
         resolution { resultValue }
       }
     }`,
    { worldId },
  );
  expect(
    records.errors,
    `worldRollRecords failed: ${JSON.stringify(records.errors)}`,
  ).toBe(undefined);
  const match = (records.data?.worldRollRecords ?? []).find(
    (record) => record.resolution.resultValue === determined,
  );
  expect(match, "the roll the server resolved must be in its own records").toBeTruthy();
  return { recordId: match!.id, determined: determined! };
}

/** The token's position as the server holds it. */
async function serverToken(
  page: Page,
  sceneId: string,
  tokenId: string,
): Promise<{ x: number; y: number } | null> {
  const res = await graphql<{
    data?: { tokens?: { tokenId: string; x: number; y: number }[] };
  }>(page, `query ($sceneId: UUID!) { tokens(sceneId: $sceneId) { tokenId x y } }`, {
    sceneId,
  });
  return res.data?.tokens?.find((token) => token.tokenId === tokenId) ?? null;
}

interface TrustTable {
  gmContext: BrowserContext;
  gm: Page;
  gmUserId: string;
  players: Page[];
  playerIds: string[];
  worldId: string;
  sceneId: string;
}

/** A Game Master and `playerCount` players, with no engine anywhere. */
async function trustTable(
  browser: Browser,
  prefix: string,
  playerCount: number,
): Promise<TrustTable> {
  const gmContext = await browser.newContext({
    permissions: ["clipboard-read", "clipboard-write"],
  });
  const gm = await gmContext.newPage();
  const worldId = await registerAndCreateWorld(gm, `E2E Trust ${uniqueSuffix()}`, prefix);
  const [sceneId] = await sceneIds(gm, worldId);
  const gmUserId = await currentUserId(gm);

  const players: Page[] = [];
  const playerIds: string[] = [];
  for (let i = 0; i < playerCount; i += 1) {
    const page = await inviteAndJoinAsPlayer(browser, gm, worldId, `${prefix}p${i}`);
    players.push(page);
    playerIds.push(await currentUserId(page));
  }
  return { gmContext, gm, gmUserId, players, playerIds, worldId, sceneId };
}

async function closeTrustTable(table: TrustTable) {
  for (const player of table.players) await player.context().close().catch(() => {});
  await table.gmContext.close().catch(() => {});
}

test.describe("Client world cache — peer-adjudicated play (US7, Phase 10a)", () => {
  test.setTimeout(600_000);

  /**
   * SC-019. The server goes away, the table does not, and play continues —
   * then the server comes back and takes what happened.
   *
   * Both halves are load-bearing and the second is the one a weaker test
   * would skip. A client that kept moving tokens on its own screen while the
   * rest of the table saw nothing would satisfy "kept playing" and would be
   * useless as a game; a table that played on and then lost all of it on
   * reconnection would be worse than having stopped. So this asserts the move
   * reaches the *other* client over the data channel, that the server does not
   * have it while the link is down, and that it does once the link is back.
   */
  test("a server-isolated table keeps playing, and the server takes the result on reconnection (SC-019, T105)", async ({
    browser,
  }) => {
    const table = await seatTable(browser, "iso105", 1);
    const gm = table.gm.page;
    const player = table.players[0].page;

    const before = await serverTokenPosition(gm, table.sceneId, table.tokenId);
    expect(before, "the token should exist server-side before the server goes away").toBeTruthy();

    await isolateTable(table.seats);

    // The state itself, on both clients. Not an intermediate detail: it is
    // what tells the players their changes are provisional (FR-063), and
    // reaching it is what licenses everything below.
    await expectSyncStatus(gm, "server-isolated", "the GM should see the table, not an outage");
    await expectSyncStatus(
      player,
      "server-isolated",
      "the player should see the table, not an outage",
    );

    await dragToken(gm, table.tokenId, { dx: 200, dy: -140 });
    const intent = await tokenPosition(gm, table.tokenId);
    expect(intent, "the GM's drag must move their own view").toBeTruthy();
    expect(intent!.x, "the GM's drag must actually move the token").not.toBe(before!.x);

    // The half that makes it play rather than solitaire: the move crossed the
    // data channel, was adjudicated, and landed on the other person's screen
    // with no server involved at all.
    await expect
      .poll(() => tokenPosition(player, table.tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "an adjudicated move must reach the rest of the table",
      })
      .toBeCloseTo(intent!.x, 0);

    // And the server has heard none of it. HTTP is still up — only heartbeats
    // are refused — so a client that wrote through anyway would show here.
    expect(
      (await serverTokenPosition(gm, table.sceneId, table.tokenId))!.x,
      "nothing adjudicated among peers may be written through to the server",
    ).toBe(before!.x);

    for (const seat of table.seats) seat.link.restore();
    for (const seat of table.seats) await waitForOnline(seat.page, seat.link);

    // SC-019's second half: accepted on reconnection. The submission rides the
    // GM's own session, which is the whole of the trust model.
    await expect
      .poll(() => serverTokenPosition(gm, table.sceneId, table.tokenId).then((p) => p?.x), {
        timeout: 120_000,
        message: "what the table agreed should reach the server once it is back",
      })
      .toBeCloseTo(intent!.x, 0);

    await closeTable(table);
  });


  /**
   * SC-020 / FR-058, first half: losing *any* peer stops adjudicated play at
   * once, for everyone who is left.
   *
   * The peer lost here is an ordinary player — not the Game Master, not a
   * majority, one person out of three. Under a quorum rule the remaining two
   * (with the GM among them) would carry on, and that is exactly the outcome
   * this asserts against: full connectivity is the rule, so two thirds and the
   * GM is not enough.
   *
   * The badge is checked, and then so is the thing the badge is about. A test
   * that only watched the indicator would pass on a client that said
   * "disconnected" while still applying moves from the table.
   */
  test("losing any peer stops adjudicated play at once, even with the GM still present (SC-020, T106)", async ({
    browser,
  }) => {
    const table = await seatTable(browser, "iso106a", 2);
    const gm = table.gm.page;
    const [alice, bob] = table.players.map((seat) => seat.page);

    await isolateTable(table.seats);
    for (const seat of table.seats) {
      await expectSyncStatus(seat.page, "server-isolated", "the whole table is here");
    }

    // The control: while everyone is present, an adjudicated move reaches
    // everyone. Without this, "the move did not arrive" below would be
    // indistinguishable from a table that never worked.
    await dragToken(gm, table.tokenId, { dx: 180, dy: -120 });
    const together = await tokenPosition(gm, table.tokenId);
    for (const page of [alice, bob]) {
      await expect
        .poll(() => tokenPosition(page, table.tokenId).then((p) => p?.x), {
          timeout: 90_000,
          message: "a full table adjudicates, and everyone sees it",
        })
        .toBeCloseTo(together!.x, 0);
    }

    // One player goes. Not the Game Master, and not a majority.
    await severPeers(bob);

    await expectAdjudicationEnded(
      gm,
      "the Game Master is still here and still must stop — a quorum is not the rule",
    );
    await expectAdjudicationEnded(alice, "and so must everybody else");

    // And play really has stopped: a move the Game Master makes now is their
    // own offline edit, queued for the server, and reaches nobody.
    const beforeAlice = await tokenPosition(alice, table.tokenId);
    await dragToken(gm, table.tokenId, { dx: -160, dy: 130 });
    expect(
      (await tokenPosition(gm, table.tokenId))!.x,
      "the Game Master may still edit — offline authoring is unaffected",
    ).not.toBeCloseTo(together!.x, 0);
    await expectNoMovement(
      alice,
      table.tokenId,
      beforeAlice!,
      "no change may be adjudicated to a table that is no longer whole",
    );

    await closeTable(table);
  });

  /**
   * SC-020 / FR-059, second half: losing the **Game Master** stops play, and
   * nobody is promoted in their place.
   *
   * The two survivors still hold an open channel to each other, so a design
   * that elected a replacement would have everything it needed to carry on —
   * which is why the assertion that matters is not the badge but the move: one
   * player's drag must not reach the other. A promotion would be a second
   * adjudicator in one session and the end of a single chain of authority, and
   * it would show up here as the token moving on a screen it must not move on.
   */
  test("losing the Game Master stops play, and no peer is promoted in their place (SC-020, T106)", async ({
    browser,
  }) => {
    const table = await seatTable(browser, "iso106b", 2);
    const gm = table.gm.page;
    const [alice, bob] = table.players.map((seat) => seat.page);

    await isolateTable(table.seats);
    for (const seat of table.seats) {
      await expectSyncStatus(seat.page, "server-isolated", "the whole table is here");
    }

    // The control again, and this time it also establishes that a *player's*
    // move is adjudicated and distributed — which is precisely the capability
    // that must disappear when the Game Master does.
    await dragToken(alice, table.tokenId, { dx: 170, dy: -110 });
    const adjudicated = await tokenPosition(alice, table.tokenId);
    await expect
      .poll(() => tokenPosition(bob, table.tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "with a Game Master present, a player's move reaches the table",
      })
      .toBeCloseTo(adjudicated!.x, 0);

    // The arbiter leaves.
    await severPeers(gm);

    await expectAdjudicationEnded(alice, "play stops rather than electing anyone");
    await expectAdjudicationEnded(bob, "for both of them");

    // The two of them can still reach each other. That is the whole point:
    // the rule is not "enough people", it is "the Game Master".
    const beforeBob = await tokenPosition(bob, table.tokenId);
    await dragToken(alice, table.tokenId, { dx: -150, dy: 120 });
    await expectNoMovement(
      bob,
      table.tokenId,
      beforeBob!,
      "no peer may be promoted to adjudicate in the Game Master's absence",
    );

    await closeTable(table);
  });

  /**
   * FR-058, the split-brain guarantee: a partition leaves **both** halves
   * stopped.
   *
   * The tempting version of this test asserts that one half keeps playing.
   * That test passing would mean split-brain shipped — two subsets, two
   * histories, and no rule that could merge them afterwards without destroying
   * somebody's work. So both halves are asserted, and the larger one is
   * asserted first because it is the one with every excuse: it holds the Game
   * Master and two of the three participants.
   *
   * The smaller half is one client rather than two, because a fourth headless
   * engine at ~190MB of wasm is a real cost and the property does not depend on
   * the minority's size — "everyone, or nobody" is indifferent to how the
   * remainder is divided.
   */
  test("a peer partition leaves both halves stopped, and neither progresses (FR-058, T107)", async ({
    browser,
  }) => {
    const table = await seatTable(browser, "iso107", 2);
    const gm = table.gm.page;
    const [alice, bob] = table.players.map((seat) => seat.page);

    await isolateTable(table.seats);
    for (const seat of table.seats) {
      await expectSyncStatus(seat.page, "server-isolated", "the whole table is here");
    }

    await dragToken(gm, table.tokenId, { dx: 160, dy: -100 });
    const agreed = await tokenPosition(gm, table.tokenId);
    for (const page of [alice, bob]) {
      await expect
        .poll(() => tokenPosition(page, table.tokenId).then((p) => p?.x), {
          timeout: 90_000,
          message: "before the split, one table and one history",
        })
        .toBeCloseTo(agreed!.x, 0);
    }

    // The split: {Game Master, Alice} on one side, Bob on the other.
    await severPeers(bob);

    for (const page of [gm, alice, bob]) {
      await expectAdjudicationEnded(page, "a partition stops everybody");
    }

    // The majority half, with the Game Master in it, makes no progress.
    const aliceBefore = await tokenPosition(alice, table.tokenId);
    await dragToken(gm, table.tokenId, { dx: -140, dy: 110 });
    await expectNoMovement(
      alice,
      table.tokenId,
      aliceBefore!,
      "the half holding the Game Master and the majority must stop too — this is the assertion that would fail if split-brain shipped",
    );

    // And the minority half makes none either, in the other direction.
    const gmBefore = await tokenPosition(gm, table.tokenId);
    await dragToken(bob, table.tokenId, { dx: 130, dy: -90 });
    await expectNoMovement(
      gm,
      table.tokenId,
      gmBefore!,
      "nor may the other side of the split reach the table it lost",
    );

    // Both sides kept their own edits, which is the point of stopping this
    // way rather than refusing the edit: each has an outbox, and the server
    // adjudicates between them when it returns (FR-040, FR-062).
    expect(
      (await tokenPosition(gm, table.tokenId))!.x,
      "each half still edits locally, for the server to settle later",
    ).not.toBeCloseTo(agreed!.x, 0);
    expect((await tokenPosition(bob, table.tokenId))!.x).not.toBeCloseTo(agreed!.x, 0);

    await closeTable(table);
  });

  /**
   * SC-021 / FR-061a. The one thing the trust model refuses.
   *
   * Attribution is made the *only* difference between the accepted and the
   * refused submission, because anything else would prove something weaker.
   * The same command, attributed to the same third person, is refused from a
   * player and accepted from a Game Master — and, so that "refused" cannot be
   * read as "that change was never going to work", the same player's own
   * unattributed submission of the same move is accepted immediately before.
   *
   * That middle step is what makes the negative mean something: without it a
   * server that rejected every replay from a player would pass.
   */
  test("a non-GM may not submit a change attributed to another player, and a GM may (SC-021, T108)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108", 2);
    const [alice, bob] = table.players;
    const [aliceId, bobId] = table.playerIds;

    // Alice owns both tokens, so nothing below can be refused for want of
    // permission to move the thing — only for attribution.
    const own = await newToken(table.gm, table.sceneId);
    const relayed = await newToken(table.gm, table.sceneId);
    await ownToken(table.gm, own, aliceId);
    await ownToken(table.gm, relayed, aliceId);

    // 1. Alice, claiming to relay Bob's change. The whole of FR-061a.
    const impersonation = await reconcile(alice, table.worldId, [
      { localId: "as-bob", command: moveCommand(relayed, 210, -140), attributedToUserId: bobId },
    ]);
    expect(impersonation[0].applied, "a player may never submit on someone else's behalf").toBe(
      false,
    );
    expect(impersonation[0].reason).toBe("PERMISSION_DENIED");
    expect(
      await serverToken(table.gm, table.sceneId, relayed),
      "a refused attribution must leave the token untouched",
    ).toMatchObject({ x: 0, y: 0 });

    // 2. The same player, the same kind of change, her own name on it. This
    //    is the control: the refusal above is about *whose* change it is, not
    //    about the change or the submitter.
    const ownSubmission = await reconcile(alice, table.worldId, [
      { localId: "as-self", command: moveCommand(own, 90, 60) },
    ]);
    expect(
      ownSubmission[0].applied,
      "a player replaying their own change must still be accepted",
    ).toBe(true);
    expect(await serverToken(table.gm, table.sceneId, own)).toMatchObject({ x: 90, y: 60 });

    // 3. The identical submission from the Game Master. Same command, same
    //    attribution, different submitter — and that difference is the
    //    entire trust model (ADR-052, "The trust model, stated plainly").
    const relayedByGm = await reconcile(table.gm, table.worldId, [
      { localId: "gm-as-bob", command: moveCommand(relayed, 210, -140), attributedToUserId: bobId },
    ]);
    expect(
      relayedByGm[0].applied,
      "a Game Master relaying a player's peer-adjudicated change is the ordinary case",
    ).toBe(true);
    expect(
      await serverToken(table.gm, table.sceneId, relayed),
      "the relayed change must actually take effect",
    ).toMatchObject({ x: 210, y: -140 });

    await bob.context().close();
    await closeTrustTable(table);
  });

  /**
   * SC-021a / SC-021b / FR-064 to FR-067. A client reports a number the
   * server determined differently.
   *
   * Four things have to be true at once, and three of them are absences:
   * the change is **applied**, the outcome is **unaltered**, both numbers are
   * **inspectable by the GM**, and **nobody else is told**. The last is a
   * requirement rather than a detail — a mark against a player visible to the
   * table is the harm the whole design is arranged to avoid — so it is
   * asserted from the player's own session, where the identical submission
   * must come back with nothing attached.
   */
  test("a reported dice value the server determined differently is applied and disclosed to the GM alone (SC-021a, SC-021b, T108a)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108a", 1);
    const [alice] = table.players;
    const [aliceId] = table.playerIds;

    // A real roll, resolved by the server, which is what makes there be
    // something to disagree with (ADR-044).
    const { recordId, determined } = await rollAndRecord(alice, table.gm, table.worldId);
    const claimed = determined + 13;

    const moved = await newToken(table.gm, table.sceneId);
    const outcome = (
      await reconcile(table.gm, table.worldId, [
        {
          localId: "disputed",
          command: moveCommand(moved, 175, -95),
          attributedToUserId: aliceId,
          reportedOutcome: { kind: "dice", version: 1, recordId, value: claimed },
        },
      ])
    )[0];

    // Never auto-rejected, never interrupted (FR-066).
    expect(outcome.applied, "a discrepancy must not reject the change").toBe(true);
    expect(outcome.reason, "nor turn it into a failure of some other kind").toBeNull();
    // Never altered: the move lands exactly where it was asked to.
    expect(
      await serverToken(table.gm, table.sceneId, moved),
      "the outcome stands as reported",
    ).toMatchObject({ x: 175, y: -95 });

    // Both numbers, inspectable, attributed to the originator rather than to
    // the Game Master who relayed it.
    expect(outcome.discrepancy, "the GM must be shown the difference").not.toBeNull();
    expect(outcome.discrepancy!.reportedValue).toBe(claimed);
    expect(outcome.discrepancy!.determinedValue).toBe(determined);
    expect(outcome.discrepancy!.recordId).toBe(recordId);
    expect(
      outcome.discrepancy!.userId,
      "the disclosure names whose outcome it was, not who relayed it",
    ).toBe(aliceId);

    // FR-067, from the other side. The identical submission made by the
    // player herself comes back with nothing attached — the disclosure is
    // computed for a Game Master submitter and for nobody else, so there is
    // no session in which a player can be shown a mark against a player.
    const asPlayer = (
      await reconcile(alice, table.worldId, [
        {
          localId: "disputed-by-player",
          command: moveCommand(moved, 176, -96),
          reportedOutcome: { kind: "dice", version: 1, recordId, value: claimed },
        },
      ])
    )[0];
    expect(
      asPlayer.discrepancy,
      "no discrepancy display may ever reach anyone but the Game Master",
    ).toBeNull();

    await closeTrustTable(table);
  });

  /**
   * FR-061b. A Game Master acting on a player's behalf produces no flag and
   * no notification to anyone.
   *
   * The absence is the requirement, so the same test also makes the *presence*
   * happen: an identical relayed change, differing only in that its reported
   * outcome genuinely disagrees with the server, does produce a disclosure.
   * Without that half, a server that had never learned to flag anything at
   * all would pass this test.
   */
  test("a Game Master acting on a player's behalf produces no flag and no notice (FR-061b, T108b)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108b", 1);
    const [alice] = table.players;
    const [aliceId] = table.playerIds;

    const quiet = await newToken(table.gm, table.sceneId);
    const flagged = await newToken(table.gm, table.sceneId);

    // Exactly what a GM adjudicating for a player produces: an attributed
    // token move, with no outcome the server could have determined for itself
    // (FR-068 — most of what this mutation carries).
    const onBehalf = (
      await reconcile(table.gm, table.worldId, [
        {
          localId: "on-behalf",
          command: moveCommand(quiet, 120, 45),
          attributedToUserId: aliceId,
        },
      ])
    )[0];
    expect(onBehalf.applied, "acting for a player is ordinary and unremarkable").toBe(true);
    expect(onBehalf.reason).toBeNull();
    expect(
      onBehalf.discrepancy,
      "a GM acting on a player's behalf is not something to flag",
    ).toBeNull();

    // Nor is the player told. The change reaches them as an ordinary token
    // event and nothing else: their own reconcile call — the only channel
    // that carries a verdict — has nothing to report.
    const playerSees = await reconcile(alice, table.worldId, []);
    expect(playerSees, "the player has nothing owing and hears nothing").toHaveLength(0);
    expect(
      await serverToken(alice, table.sceneId, quiet),
      "the player simply sees the token where the table put it",
    ).toMatchObject({ x: 120, y: 45 });

    // And the control: the machinery is running, and does flag a genuine
    // mismatch on an otherwise identical relayed change.
    const { recordId, determined } = await rollAndRecord(alice, table.gm, table.worldId);
    const control = (
      await reconcile(table.gm, table.worldId, [
        {
          localId: "on-behalf-mismatch",
          command: moveCommand(flagged, 121, 46),
          attributedToUserId: aliceId,
          reportedOutcome: {
            kind: "dice",
            version: 1,
            recordId,
            value: determined + 5,
          },
        },
      ])
    )[0];
    expect(
      control.discrepancy,
      "the same relay with a genuinely different number is disclosed — so the silence above is a decision, not a gap",
    ).not.toBeNull();

    await closeTrustTable(table);
  });

  /**
   * SC-021c / FR-067a. Every ambiguity is silence.
   *
   * This is the test that matters more than T108a, and the reason is not
   * symmetry: a missed discrepancy costs nothing, because the GM runs their
   * table either way, while a false one puts an innocent player under
   * suspicion in front of the only person who can act on it. So each of these
   * cases must produce *no* disclosure rather than a plausible-looking one.
   *
   * The genuine mismatch is submitted first, from the same session against the
   * same record, so every `toBeNull` below is measured against a detector that
   * has just been seen working.
   *
   * One ambiguity is not reachable from here: a **database failure or
   * statement timeout** while reading the determined value. There is no way to
   * induce one through a signed-in session, and contriving one would mean
   * building a failure mode into the server to test it. It is covered where it
   * can be — `determination_from_lookup` in `mutations_reconcile.rs` maps an
   * `Err` and an absent row to the same `None`, which is the same code path
   * the "unknown record" case below takes.
   */
  test("timeout, parse failure, version mismatch and missing determination each produce no discrepancy (SC-021c, T108c)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108c", 1);
    const [alice] = table.players;
    const [aliceId] = table.playerIds;

    const { recordId, determined } = await rollAndRecord(alice, table.gm, table.worldId);
    const claimed = determined + 11;

    // A separate world the Game Master also owns, for the "the record is not
    // one of ours" case — the lookup is scoped by world, so a real record id
    // from elsewhere must read as no such row rather than as a value to
    // compare against.
    const otherWorldId = await (async () => {
      await table.gm.goto("/worlds/create");
      await table.gm.locator("#world-name").fill(`E2E Trust Other ${uniqueSuffix()}`);
      await table.gm.getByRole("button", { name: /create world/i }).click();
      await table.gm.waitForURL(/\/world\/[^/]+\/staging$/, { timeout: 20_000 });
      const match = /\/world\/([^/]+)\/staging$/.exec(new URL(table.gm.url()).pathname);
      return match![1];
    })();

    const token = async () => newToken(table.gm, table.sceneId);
    const submit = async (
      localId: string,
      reportedOutcome: QueuedChange["reportedOutcome"],
    ) =>
      (
        await reconcile(table.gm, table.worldId, [
          {
            localId,
            command: moveCommand(await token(), 30, 30),
            attributedToUserId: aliceId,
            reportedOutcome,
          },
        ])
      )[0];

    // The positive control, first. Everything after this is the same session,
    // the same record and the same numbers, changed one field at a time.
    const genuine = await submit("genuine", {
      kind: "dice",
      version: 1,
      recordId,
      value: claimed,
    });
    expect(
      genuine.discrepancy,
      "a genuine determined-value mismatch is the one case that speaks",
    ).not.toBeNull();

    const ambiguities: [string, QueuedChange["reportedOutcome"], string][] = [
      [
        "version",
        { kind: "dice", version: 2, recordId, value: claimed },
        "a client one release ahead is telling us something we cannot read",
      ],
      [
        "kind",
        { kind: "initiative", version: 1, recordId, value: claimed },
        "an outcome the server has no independent basis for is not a disagreement",
      ],
      [
        "no-value",
        { kind: "dice", version: 1, recordId, value: null },
        "half a report is not a report",
      ],
      [
        "no-record",
        { kind: "dice", version: 1, recordId: null, value: claimed },
        "a value with nothing to compare it against determines nothing",
      ],
      [
        "unknown-record",
        {
          kind: "dice",
          version: 1,
          recordId: "00000000-0000-4000-8000-000000000000",
          value: claimed,
        },
        "a record that is not there is an absent determination, not a differing one",
      ],
    ];

    for (const [name, reported, why] of ambiguities) {
      const outcome = await submit(`ambiguous-${name}`, reported);
      expect(outcome.discrepancy, why).toBeNull();
      expect(
        outcome.applied,
        "and an ambiguity must not cost the change either — silence is not refusal",
      ).toBe(true);
    }

    // The same shape again, with a record that exists but belongs to another
    // world. The lookup is scoped, so this is an absent determination — and
    // it is the case a scoping mistake would turn into a fabricated mismatch.
    const elsewhere = await rollAndRecord(table.gm, table.gm, otherWorldId);
    const crossWorld = await submit("ambiguous-other-world", {
      kind: "dice",
      version: 1,
      recordId: elsewhere.recordId,
      value: claimed,
    });
    expect(
      crossWorld.discrepancy,
      "a record from another world must read as no such row",
    ).toBeNull();

    await closeTrustTable(table);
  });


  /**
   * FR-065 / FR-067, at the surface the requirement is actually about: what
   * the Game Master *sees*, and what everybody else does not.
   *
   * The two numbers are put on the wire by the test rather than by the
   * server, and that is a deliberate, documented limit. `rollDice` has no
   * `discrepancy` field and `api/reconcile.ts` does not select the one the
   * reconcile mutation returns, so nothing in the shipped client ever carries
   * a comparison to the component that renders it (see this suite's report).
   * Stubbing the response reaches the half that does exist — `RollResult`,
   * `discrepancyToShow`, and the role gate between them — running inside the
   * real page, with a real GM session and a real player session.
   *
   * The GM half is asserted first so the player half means something: the
   * marker demonstrably renders for exactly the same payload, so its absence
   * on the player's screen is the gate doing its work rather than the feature
   * failing to run.
   */
  test("a discrepancy is rendered distinctly for the Game Master, with both values, and not shown to players (SC-021a, FR-067, T108a)", async ({
    browser,
  }) => {
    const table = await trustTable(browser, "iso108d", 1);
    const [player] = table.players;

    const CLAIMED = 41;
    const DETERMINED = 17;

    // Answer the app's own roll with the same result the server gave, plus
    // the comparison the server would attach once the field exists. Only the
    // roll is touched; every other GraphQL call passes through untouched.
    const attachDiscrepancy = async (page: Page) => {
      await page.route("**/api/graphql", async (route) => {
        const body = route.request().postData() ?? "";
        if (!body.includes("RollDice")) {
          await route.fallback();
          return;
        }
        const response = await route.fetch();
        const json = (await response.json()) as {
          data?: { rollDice?: Record<string, unknown> };
        };
        if (json.data?.rollDice) {
          json.data.rollDice.resultValue = CLAIMED;
          json.data.rollDice.discrepancy = {
            claimedValue: CLAIMED,
            determinedValue: DETERMINED,
          };
        }
        await route.fulfill({ response, json });
      });
    };

    await attachDiscrepancy(table.gm);
    await attachDiscrepancy(player);

    const roll = async (page: Page) => {
      await page.goto(`/world/${table.worldId}/play`);
      await expect(page.getByTestId("dice-roller-panel")).toBeVisible({ timeout: 60_000 });
      await page.getByTestId("dice-formula-input").fill("1d20");
      await page.getByTestId("dice-roll-button").click();
      await expect(page.getByTestId("dice-roll-result")).toBeVisible({ timeout: 30_000 });
    };

    await roll(table.gm);
    // Rendered distinctly, and the result itself stands unaltered — the note
    // is a difference observed, never a correction applied (FR-066).
    await expect(table.gm.getByTestId("roll-discrepancy-marker")).toBeVisible({
      timeout: 15_000,
    });
    await expect(table.gm.getByTestId("roll-result-total")).toHaveText(String(CLAIMED));

    // Both values inspectable, and nothing offered to do about them: the
    // popover has two numbers and a sentence, and no control that would make
    // the software the arbiter of a question it cannot answer (FR-065a).
    await table.gm.getByTestId("roll-discrepancy-marker").click();
    await expect(table.gm.getByTestId("roll-discrepancy-claimed")).toHaveText(
      String(CLAIMED),
    );
    await expect(table.gm.getByTestId("roll-discrepancy-determined")).toHaveText(
      String(DETERMINED),
    );
    const details = table.gm.getByTestId("roll-discrepancy-details");
    await expect(details).toContainText("stands as rolled");
    await expect(
      details.getByRole("button"),
      "there is no resolution workflow, deliberately",
    ).toHaveCount(0);

    // And the player, given the identical payload, sees an ordinary roll.
    await roll(player);
    await expect(player.getByTestId("roll-result-total")).toHaveText(String(CLAIMED));
    await expect(
      player.getByTestId("roll-discrepancy-marker"),
      "a mark against a player must never be visible to the table (FR-067)",
    ).toHaveCount(0);

    await closeTrustTable(table);
  });

});
