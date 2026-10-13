import { expect, test } from "./fixtures/test";
import {
  dragToken,
  serverTokenPosition,
  tokenPosition,
  waitForOnline,
} from "./fixtures/offline";
import {
  closeTable,
  expectAdjudicationEnded,
  expectNoMovement,
  expectSyncStatus,
  isolateTable,
  seatTable,
  severPeers,
} from "./fixtures/world-cache-adjudication";

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
 * The trust model (T108–T108c) is in `world-cache-isolated-trust.spec.ts`,
 * and the discrepancy display in `world-cache-isolated-discrepancy.spec.ts`.
 */

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
    expect(
      before,
      "the token should exist server-side before the server goes away",
    ).toBeTruthy();

    await isolateTable(table.seats);

    // The state itself, on both clients. Not an intermediate detail: it is
    // what tells the players their changes are provisional (FR-063), and
    // reaching it is what licenses everything below.
    await expectSyncStatus(
      gm,
      "server-isolated",
      "the GM should see the table, not an outage",
    );
    await expectSyncStatus(
      player,
      "server-isolated",
      "the player should see the table, not an outage",
    );

    await dragToken(gm, table.tokenId, { dx: 200, dy: -140 });
    const intent = await tokenPosition(gm, table.tokenId);
    expect(intent, "the GM's drag must move their own view").toBeTruthy();
    expect(intent!.x, "the GM's drag must actually move the token").not.toBe(
      before!.x,
    );

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
      .poll(
        () =>
          serverTokenPosition(gm, table.sceneId, table.tokenId).then(
            (p) => p?.x,
          ),
        {
          timeout: 120_000,
          message:
            "what the table agreed should reach the server once it is back",
        },
      )
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
      await expectSyncStatus(
        seat.page,
        "server-isolated",
        "the whole table is here",
      );
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
      await expectSyncStatus(
        seat.page,
        "server-isolated",
        "the whole table is here",
      );
    }

    // The control again, and this time it also establishes that a *player's*
    // move is adjudicated and distributed — which is precisely the capability
    // that must disappear when the Game Master does.
    await dragToken(alice, table.tokenId, { dx: 170, dy: -110 });
    const adjudicated = await tokenPosition(alice, table.tokenId);
    await expect
      .poll(() => tokenPosition(bob, table.tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message:
          "with a Game Master present, a player's move reaches the table",
      })
      .toBeCloseTo(adjudicated!.x, 0);

    // The arbiter leaves.
    await severPeers(gm);

    await expectAdjudicationEnded(
      alice,
      "play stops rather than electing anyone",
    );
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
      await expectSyncStatus(
        seat.page,
        "server-isolated",
        "the whole table is here",
      );
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
    expect((await tokenPosition(bob, table.tokenId))!.x).not.toBeCloseTo(
      agreed!.x,
      0,
    );

    await closeTable(table);
  });
});
