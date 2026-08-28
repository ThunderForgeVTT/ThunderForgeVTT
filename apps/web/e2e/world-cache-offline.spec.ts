import { expect, test } from "@playwright/test";
import { inviteAndJoinAsPlayer, uniqueSuffix } from "./fixtures/helpers";
import {
  createToken,
  createWorldAndPlay,
  dragCanvas,
  dragToken,
  currentUserId,
  firstSceneId,
  giveTokenTo,
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
 * Playing on through a lost connection (spec 028 US7, T083–T085,
 * SC-015/SC-016).
 *
 * These are the tests the rest of US7 exists for. Every piece — the outbox,
 * the disconnected state, `conflict::resolve`, the reconcile mutation, the
 * report — is individually covered by unit tests that cannot see the one thing
 * that matters: whether a token a person dragged while offline is still where
 * they put it once the connection comes back.
 *
 * The severing, the drag and the registration flow live in
 * `fixtures/offline.ts`, shared with `world-cache-isolated.spec.ts`; the
 * reasoning behind each is documented there.
 */

test.describe("Client world cache — playing on through a lost connection (US7)", () => {
  test.setTimeout(420_000);

  /**
   * The bug this test found, for anyone reading the history: the token
   * mutation bridge seeded its engine-id map once, from the scene's tokens
   * at start, and only ever extended it from its own `createToken` calls. A
   * token created afterwards through the token panel — which calls the
   * mutation directly — was therefore never in the map, so every later drag
   * of it read as a *first sighting*, and FR-035a's "creations are not
   * queued offline" branch swallowed an ordinary move. Nothing was queued,
   * so there was nothing to replay and no `ReconcileQueuedChanges` request
   * was ever made. The bridge now learns from `sync` dispatches, which is
   * how the client hears about a token in the first place.
   *
   * Worth noting what nearly hid it: the "the server must not have it yet"
   * assertion below passed throughout, and was read as proof that queueing
   * worked. It is not — a silently dropped edit leaves the server unchanged
   * too. The assertion separates "queued" from "sent anyway", and says
   * nothing about "dropped".
   */
  test("a change made offline is applied on reconnect and reported (SC-015, T083)", async ({
    page,
  }) => {
    const link = severableLink(page);
    await link.install();

    await register(page, "e2eoff");
    const worldId = await createWorldAndPlay(page, `E2E Offline ${uniqueSuffix()}`);
    await waitForEngineReady(page);
    const sceneId = await firstSceneId(page, worldId);
    const tokenId = await createToken(page);

    const before = await serverTokenPosition(page, sceneId, tokenId);
    expect(before, "the token should exist server-side before we go offline").toBeTruthy();

    link.cut();
    await waitForOffline(page, link);

    // The edit. A panel-created token starts at the world origin.
    await dragCanvas(page, { dx: 0, dy: 0 }, { dx: 180, dy: -120 });

    const localAfterDrag = await tokenPosition(page, tokenId);
    expect(
      localAfterDrag,
      "the token must move locally — offline play is the point",
    ).toBeTruthy();
    expect(localAfterDrag!.x).not.toBe(before!.x);

    // And the server must not have it yet. This is what distinguishes
    // "queued" from "sent anyway": HTTP is still up, so a client that fired
    // the mutation regardless would have written through.
    const duringOutage = await serverTokenPosition(page, sceneId, tokenId);
    expect(
      duringOutage!.x,
      "an offline edit must be queued, not written through",
    ).toBe(before!.x);

    link.restore();
    await waitForOnline(page, link);

    // SC-015: applied on reconnect, and reported.
    await expect
      .poll(() => serverTokenPosition(page, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "the queued edit should reach the server on reconnect",
      })
      .not.toBe(before!.x);

    await expect(page.getByTestId("reconcile-report")).toBeVisible({ timeout: 30_000 });
  });


  /**
   * SC-016: two people edit the same token with no connection, and the
   * **player reconnects first**.
   *
   * The ordering is the entire test. A player who reconnects first has their
   * change applied — legitimately, since at that moment nothing contradicts
   * it — and only later does the Game Master come back with a conflicting
   * edit that outranks it. So this is the `Applied → Superseded` path
   * (FR-041), the one case where a change is reported as synced and then
   * stops being true.
   *
   * There is no response left to carry that news: the player is long gone
   * from their own reconcile call by the time the GM makes theirs. It can
   * only reach them as an ordinary world event, which is why the server's
   * token event grows `reconciled`/`by_user`/`by_role` (T079). Asserting
   * only that the two clients converge would pass while the player is never
   * told their work was overridden — and being told is the requirement.
   */
  test("a Game Master reconnecting after a player overrides them, and the player is told (SC-016, T084)", async ({
    browser,
  }) => {
    // The invite flow writes to the clipboard, and a context without that
    // permission throws before the new invite is stored — see
    // scene-live-launch.spec.ts, which hit the same thing.
    const gmContext = await browser.newContext({
      permissions: ["clipboard-read", "clipboard-write"],
    });
    const gmPage = await gmContext.newPage();

    await register(gmPage, "e2eogm");
    const worldId = await createWorldAndPlay(gmPage, `E2E Offline Two ${uniqueSuffix()}`);
    await waitForEngineReady(gmPage);
    const sceneId = await firstSceneId(gmPage, worldId);
    const tokenId = await createToken(gmPage);

    const playerPage = await inviteAndJoinAsPlayer(browser, gmPage, worldId, "e2eopl");
    await giveTokenTo(gmPage, tokenId, await currentUserId(playerPage));

    // Both at the table, on the same scene, before either goes offline.
    await gmPage.goto(`/world/${worldId}/play`);
    await waitForEngineReady(gmPage);
    await playerPage.goto(`/world/${worldId}/play`);
    await waitForEngineReady(playerPage);
    await waitForTokenTrafficToSettle(gmPage);
    await waitForTokenTrafficToSettle(playerPage);

    const before = await serverTokenPosition(gmPage, sceneId, tokenId);
    expect(before, "the token should exist server-side before anyone drops").toBeTruthy();

    const gmLink = severableLink(gmPage);
    const playerLink = severableLink(playerPage);
    await gmLink.install();
    await playerLink.install();

    gmLink.cut();
    playerLink.cut();
    await waitForOffline(gmPage, gmLink);
    await waitForOffline(playerPage, playerLink);

    // The same token, two different destinations, neither reaching the
    // server. Only the heartbeat is severed, so each still holds a live
    // subscription — which is what lets the player hear about the GM's
    // replay later, and is also how a real outage of this shape behaves.
    await dragToken(gmPage, tokenId, { dx: 200, dy: -140 });
    await dragToken(playerPage, tokenId, { dx: -170, dy: 110 });

    // Read the GM's position *now*. Once the player reconnects, the event
    // from their applied change reaches this still-subscribed page and moves
    // the token locally — so a reading taken later is the player's position,
    // not the GM's, and the final assertion would be comparing the server
    // against itself.
    const gmIntent = await tokenPosition(gmPage, tokenId);
    const playerIntent = await tokenPosition(playerPage, tokenId);
    // Not merely "a token is there": each drag must actually have moved it.
    // A drag that silently did nothing leaves the local and server positions
    // equal, which makes every convergence assertion below pass by
    // comparing the starting position with itself.
    expect(gmIntent, "the GM's drag must move their own view").toBeTruthy();
    expect(playerIntent, "the player's drag must move their own view").toBeTruthy();
    expect(gmIntent!.x, "the GM's drag must actually move the token").not.toBe(before!.x);
    expect(playerIntent!.x, "the player's drag must actually move the token").not.toBe(
      before!.x,
    );
    expect(
      (await serverTokenPosition(gmPage, sceneId, tokenId))!.x,
      "neither edit may be written through while offline",
    ).toBe(before!.x);

    // The player comes back first, and wins for as long as nothing outranks
    // them.
    playerLink.restore();
    await waitForOnline(playerPage, playerLink);
    await expect
      .poll(() => serverTokenPosition(playerPage, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "the player's queued edit should be applied on their reconnect",
      })
      .toBeCloseTo(playerIntent!.x, 0);
    expect(
      playerLink.reconcileRequests(),
      "the player should have submitted their queue on reconnect",
    ).toBeGreaterThanOrEqual(1);
    await expect(playerPage.getByTestId("reconcile-report")).toBeVisible({
      timeout: 30_000,
    });

    // Then the Game Master, whose edit outranks it (FR-040).
    gmLink.restore();
    await waitForOnline(gmPage, gmLink);
    await expect
      .poll(() => serverTokenPosition(gmPage, sceneId, tokenId).then((p) => p?.x), {
        timeout: 90_000,
        message: "a Game Master reconnecting later still wins",
      })
      .toBeCloseTo(gmIntent!.x, 0);

    // Convergence, which is the easy half.
    await expect
      .poll(() => tokenPosition(playerPage, tokenId).then((p) => p?.x), {
        timeout: 60_000,
        message: "the player's own view should follow the server",
      })
      .toBeCloseTo(gmIntent!.x, 0);

    // And the half that matters: the player is told, rather than left
    // believing an edit stands that does not.
    await expect(
      playerPage.getByTestId("reconcile-superseded"),
    ).toBeVisible({ timeout: 60_000 });
    await expect(playerPage.getByTestId("reconcile-superseded")).toContainText(
      "the Game Master",
    );

    await playerPage.context().close();
    await gmContext.close();
  });

  test("a queued change against deleted content is discarded with an explanation (T085)", async ({
    page,
  }) => {
    const link = severableLink(page);
    await link.install();

    await register(page, "e2eoffdel");
    const worldId = await createWorldAndPlay(page, `E2E Offline Gone ${uniqueSuffix()}`);
    await waitForEngineReady(page);
    const sceneId = await firstSceneId(page, worldId);
    const tokenId = await createToken(page);

    link.cut();
    await waitForOffline(page, link);
    await dragCanvas(page, { dx: 0, dy: 0 }, { dx: 150, dy: -90 });

    // Deleted while this client was away. HTTP still works — only the
    // subscription was severed — so the test can act as the other client
    // that removed it.
    const deleted = await page.evaluate(
      async ({ token }) => {
        const csrf = document.cookie
          .split(";")
          .map((p) => p.trim())
          .find((p) => p.startsWith("csrf_token="))
          ?.slice("csrf_token=".length);
        const res = await fetch("/api/graphql", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "content-type": "application/json",
            ...(csrf ? { "x-csrf-token": csrf } : {}),
          },
          body: JSON.stringify({
            query: `mutation ($tokenId: UUID!) { deleteToken(tokenId: $tokenId) }`,
            variables: { tokenId: token },
          }),
        });
        return res.ok;
      },
      { token: tokenId },
    );
    expect(deleted, "the token should have been deleted server-side").toBe(true);

    link.restore();
    await waitForOnline(page, link);

    // Discarded with a reason, never resurrected. Recreating something
    // someone deliberately removed is the failure FR-035a's create/delete
    // restriction exists to avoid.
    await expect(page.getByTestId("reconcile-report")).toBeVisible({ timeout: 90_000 });
    await expect(
      page.locator('[data-testid="reconcile-rejected"] [data-reason="GONE_AWAY"]'),
    ).toBeVisible({ timeout: 30_000 });

    expect(
      await serverTokenPosition(page, sceneId, tokenId),
      "a deleted token must not come back",
    ).toBeNull();
  });
});

