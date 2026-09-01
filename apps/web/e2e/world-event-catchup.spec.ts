import { expect, test } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";

/**
 * Events that happen while the socket is down are recovered on reconnect.
 *
 * # The gap this closes
 *
 * Live delivery is at-most-once and always will be: a socket that is down
 * receives nothing. `graphql-ws` reconnects and resumes, and until the
 * catch-up existed that resumption silently assumed nothing had happened in
 * the meantime. The reconnect handler did refetch scene *content* — walls,
 * lights, shapes, tokens — but anything that exists only as an event (a scene
 * launched, a chat message posted, combat advanced) was simply never seen by
 * that tab again.
 *
 * # Why the test severs only the WebSocket
 *
 * `context.setOffline(true)` also breaks Vite's dynamic module fetches and
 * crashes the page with an unrelated error before the drop is observed — the
 * same reason `live-sync.spec.ts` routes the socket rather than going offline.
 * Severing just `/api/ws` leaves HTTP working, which is what lets the test act
 * as "another client" during the outage using the page's own session.
 *
 * # What is asserted, and why it is the honest assertion
 *
 * That the client asks `worldEventsSince` on reconnect **and that the answer
 * contains the event it missed**. Asserting a rendered side effect would be
 * stronger still, but it would be asserting the chat panel's refetch rather
 * than the recovery mechanism — and the mechanism is what did not exist.
 */

test.describe("World event catch-up on reconnect", () => {
  test.setTimeout(180_000);

  test("an event that happens while the socket is down is replayed on reconnect", async ({
    page,
  }) => {
    // Keep a handle on the app's *own* socket, rather than proxying /api/ws.
    //
    // This used to `page.routeWebSocket` the connection and sever the proxy.
    // The proxy itself is sound — Playwright replays the `graphql-transport-ws`
    // subprotocol and the socket is opened in-page so the session cookie rides
    // along — but it puts every frame of the handshake through a per-message
    // round trip into the page, and this spec cold-loads `/play` with
    // `page.goto`, so the ack has to win against the engine's wasm blocking the
    // main thread. When it lost, the tab never completed a handshake at all and
    // the failure surfaced 100 lines later as "the reconnect never finished",
    // which it was not.
    //
    // Recording the sockets instead leaves the handshake entirely un-proxied,
    // and severs the page's real connections rather than whichever one the
    // route happened to see last. It
    // also drops a leak: registering `ws.onClose` suppressed Playwright's own
    // forwarding of the page close to the server, so each severed connection's
    // `worldEventsCreated` subscription stayed alive for the rest of the run.
    await page.addInitScript(() => {
      const Native = window.WebSocket;
      const sockets: WebSocket[] = [];
      (window as unknown as { __e2eSockets: WebSocket[] }).__e2eSockets = sockets;
      class RecordingWebSocket extends Native {
        constructor(url: string | URL, protocols?: string | string[]) {
          super(url, protocols);
          if (String(url).includes("/api/ws")) {
            sockets.push(this);
          }
        }
      }
      window.WebSocket = RecordingWebSocket as unknown as typeof WebSocket;
    });

    // Every catch-up this page performs, recorded from before anything is
    // severed. See the note at the assertion for why this cannot be a
    // `waitForResponse` set up at the moment of interest.
    const catchUps: {
      askedFrom: number;
      events?: { id?: number }[];
      truncated?: boolean;
      latestId?: number;
    }[] = [];
    page.on("response", (response) => {
      if (!response.url().includes("/api/graphql")) return;
      const post = response.request().postData() ?? "";
      // The client's own query, not this test's helper below — the helper
      // asks for `latestId` alone and would otherwise be counted as a
      // catch-up the application never performed.
      if (!post.includes("WorldEventsSince") || !post.includes("truncated")) {
        return;
      }
      let askedFrom = -1;
      try {
        askedFrom = JSON.parse(post).variables?.afterId ?? -1;
      } catch {
        askedFrom = -1;
      }
      void response
        .json()
        .then((body) => {
          const payload = (
            body as {
              data?: {
                worldEventsSince?: {
                  events?: { id?: number }[];
                  truncated?: boolean;
                  latestId?: number;
                };
              };
            }
          ).data?.worldEventsSince;
          if (payload) catchUps.push({ ...payload, askedFrom });
        })
        .catch(() => {
          // A body that is gone by the time we ask undercounts, which can
          // only make the poll below time out — never pass spuriously.
        });
    });

    const worldId = await registerAndCreateWorld(
      page,
      `E2E Catchup ${uniqueSuffix()}`,
      "e2ecatchup",
    );

    // Into play, so the world page opens its subscriptions and the reconnect
    // handler is mounted.
    await page.goto(`/world/${worldId}/play`);
    const indicator = page.getByTestId("live-sync-reconnecting-indicator");

    // `ConnectionStatus` renders nothing at all once the socket is live, so
    // "hidden" is also exactly what a page that has not mounted it yet looks
    // like — and straight after `goto` that is the likelier reading. Waiting
    // for the play view to be up first makes this assertion about the socket
    // rather than about React not having got there, so a connection that
    // never comes up fails here, where it happened, instead of surviving to
    // the reconnect assertion 100 lines below and being blamed on it.
    //
    // Not "wait for the indicator to appear, then disappear": the banner is
    // only rendered while the socket is *not* live, and a connection that
    // completes promptly never renders it at all. Requiring it to appear
    // would demand the outage this assertion exists to rule out.
    await expect(page.locator("canvas")).toBeVisible({ timeout: 30_000 });
    await expect(indicator).toBeHidden({ timeout: 30_000 });

    // Let the page's subscriptions actually open before relying on live
    // delivery. The socket being `live` is not the same thing: the world page
    // mounts several panels, each of which opens its own subscription as it
    // renders, and an event published in that window reaches nobody. Measured
    // by inspecting the WebSocket frames — the first delivery arrived several
    // seconds after the indicator cleared.
    await page.waitForTimeout(6_000);

    // Establish a cursor: an event received *live*, so the client has a real
    // `lastSeenId` rather than zero. Without this the catch-up would ask from
    // the beginning of the world, replay everything, and the assertions below
    // would pass while proving nothing about the cursor.
    const before = await sendChat(page, worldId, "before the outage");
    expect(before, "the pre-outage message should have been recorded").toBeGreaterThan(0);
    await page.waitForTimeout(3_000);

    // --- sever the socket, and hold it down ---
    //
    // Closing alone is not an outage. `graphql-ws` retries immediately, and
    // now that the indicator reports the connection truthfully rather than
    // sitting at "connecting" forever, the gap is far too short to post an
    // event into. Blocking the endpoint over CDP keeps *new* connections from
    // succeeding, so the close below produces a real, controlled window.
    // Scoped to `/api/ws` so Vite's own module fetches keep working — a full
    // `setOffline` breaks the lazy route chunks this app loads, which is why
    // these specs never used it.
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Network.enable");
    await cdp.send("Network.setBlockedURLs", { urls: ["*/api/ws*"] });

    // Every open socket, not the last one.
    //
    // A world page holds *two* connections to `/api/ws`. The world-event
    // client in `engine/world/sync/subscriptionClient.ts` is the one whose
    // state the indicator reports; `engine/bevy/index.ts` opens a second,
    // private client for peer-transfer signalling, and it opens later —
    // when the engine starts, after the page has loaded. So `at(-1)` was
    // reliably the *signalling* socket, and closing it left the sync client
    // connected and the indicator correctly saying nothing. The test was
    // severing a connection it was not asserting about.
    //
    // Closing all of them is also the more honest reading of the scenario:
    // the outage this spec describes is a tab that cannot hear the server,
    // not one particular socket going away.
    const severed = await page.evaluate(() => {
      const sockets = (window as unknown as { __e2eSockets: WebSocket[] })
        .__e2eSockets;
      const open = sockets.filter((s) => s.readyState === WebSocket.OPEN);
      for (const socket of open) {
        // 4499 is graphql-ws's "Terminated", deliberately excluded from its
        // fatal close codes — the client retries this one rather than giving
        // up, which is the outage this test is about. A plain 1000 reads as a
        // clean, intentional shutdown and would not be retried.
        socket.close(4499, "e2e sever");
      }
      return { open: open.length, recorded: sockets.length };
    });
    expect(
      severed.open,
      `the page should have an active /api/ws connection (recorded ${severed.recorded})`,
    ).toBeGreaterThan(0);
    await expect(indicator).toBeVisible({ timeout: 20_000 });

    // --- something happens while this tab cannot hear it ---
    //
    // Over HTTP, which the route above leaves working. This is a real world
    // event written to the durable record; the tab is simply not listening.
    const missedId = await sendChat(page, worldId, "posted during the outage");
    expect(missedId, "the missed message should have been recorded").toBeGreaterThan(
      before,
    );

    // Let it back in. `graphql-ws` is already retrying with backoff, so the
    // next attempt after this succeeds and the client resumes on its own.
    await cdp.send("Network.setBlockedURLs", { urls: [] });
    await expect(indicator).toBeHidden({ timeout: 60_000 });

    // The collector was installed before the socket was severed, on purpose:
    // graphql-ws backs off for about a second, so the reconnect — and with it
    // the catch-up — can fire while the test is still posting the message it
    // means to miss. Waiting for the response *after* that point is a race
    // the test loses roughly half the time, and losing it looks exactly like
    // the feature not working.
    await expect
      .poll(() => catchUps.length, {
        timeout: 30_000,
        message: "the client should ask worldEventsSince after reconnecting",
      })
      .toBeGreaterThan(0);

    const result = catchUps[catchUps.length - 1];
    expect(result, "catch-up returned no data").toBeTruthy();
    expect(
      result?.truncated,
      "a one-event gap must not be reported as too large to replay",
    ).toBe(false);

    // The point of the whole feature: the event this tab could not hear is in
    // the replay.
    const replayedIds = (result?.events ?? []).map((e) => e.id);
    console.log(
      `[catchup] missed id=${missedId}; replayed ${JSON.stringify(replayedIds)}`,
    );
    expect(
      replayedIds,
      "the event that happened during the outage must be replayed",
    ).toContain(missedId);

    // And it asked from where it actually was, rather than from the
    // beginning of the world — the cursor is the whole point, and a catch-up
    // that always replayed everything would satisfy the assertion above while
    // being useless.
    console.log(`[catchup] asked from ${result.askedFrom}`);
    expect(
      result.askedFrom,
      "the client must ask from the cursor it had, not from zero — a catch-up \
       that always replayed the whole world would satisfy the assertion above \
       while proving nothing",
    ).toBeGreaterThanOrEqual(before);
    expect(
      replayedIds,
      "an event the client already processed must not be replayed",
    ).not.toContain(before);
  });
});

/**
 * Post a chat message and return the `world_events` id it produced.
 *
 * The mutation returns the message, not the event, so the event id is read
 * back from the catch-up query with a cursor of 0 — which is also a small
 * check that the query works at all before the test leans on it.
 */
async function sendChat(
  page: import("@playwright/test").Page,
  worldId: string,
  body: string,
): Promise<number> {
  const sent = await graphql<{
    data?: { sendChatMessage?: { id: string } };
    errors?: { message: string }[];
  }>(
    page,
    `mutation ($input: SendChatMessageInput!) {
       sendChatMessage(input: $input) { id }
     }`,
    { input: { worldId, body } },
  );
  expect(
    sent.errors,
    `sendChatMessage failed: ${JSON.stringify(sent.errors)}`,
  ).toBe(undefined);

  const events = await graphql<{
    data?: { worldEventsSince?: { latestId?: number } };
    errors?: { message: string }[];
  }>(
    page,
    `query ($worldId: UUID!, $afterId: Int!) {
       worldEventsSince(worldId: $worldId, afterId: $afterId) { latestId }
     }`,
    { worldId, afterId: 0 },
  );
  expect(
    events.errors,
    `worldEventsSince failed: ${JSON.stringify(events.errors)}`,
  ).toBe(undefined);

  return events.data?.worldEventsSince?.latestId ?? 0;
}
