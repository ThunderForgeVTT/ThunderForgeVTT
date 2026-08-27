import { expect, test, type WebSocketRoute } from "@playwright/test";
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
    // Forward /api/ws transparently, keeping a handle so it can be severed.
    let activeClientWs: WebSocketRoute | null = null;
    await page.routeWebSocket(/\/api\/ws/, (ws) => {
      const server = ws.connectToServer();
      ws.onMessage((message) => server.send(message));
      server.onMessage((message) => ws.send(message));
      activeClientWs = ws;
      ws.onClose(() => {
        if (activeClientWs === ws) activeClientWs = null;
      });
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

    // --- sever the socket ---
    expect(
      activeClientWs,
      "the page should have an active /api/ws connection",
    ).not.toBeNull();
    activeClientWs?.close();
    await expect(indicator).toBeVisible({ timeout: 20_000 });

    // --- something happens while this tab cannot hear it ---
    //
    // Over HTTP, which the route above leaves working. This is a real world
    // event written to the durable record; the tab is simply not listening.
    const missedId = await sendChat(page, worldId, "posted during the outage");
    expect(missedId, "the missed message should have been recorded").toBeGreaterThan(
      before,
    );

    // graphql-ws retries on its own; the route handler passes the next
    // attempt straight through to the real server.
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
