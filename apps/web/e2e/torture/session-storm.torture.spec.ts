import { expect, test, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "../fixtures/helpers";

/**
 * Session-count torture test.
 *
 * # Why not N browser contexts
 *
 * The obvious reading of "100 connected sessions" is 100 Playwright contexts.
 * That would not work, and worse, would not measure the thing we care about:
 * each context instantiates the engine, so at the current bundle size 100 of
 * them is roughly 21GB of WASM before a single scene loads. The run would
 * thrash or OOM, and whatever number came out would describe the test machine
 * rather than the server.
 *
 * What actually comes under strain as a table grows is the *transport and
 * fan-out* — the `graphql-ws` subscription, the `world_events` broadcast, and
 * the Postgres NOTIFY behind it. None of that needs a canvas.
 *
 * # Why the sockets live inside the page
 *
 * The subscription is authenticated by session cookie on the WebSocket
 * upgrade. Node's native `WebSocket` cannot set headers, so driving this from
 * the test process would mean adding a `ws` dependency purely to hand-roll
 * cookie plumbing that the browser does for free — and it would exercise a
 * different code path from the one users hit. Opening the sockets inside the
 * page gets authentication for nothing and tests the real path. Browsers cap
 * *HTTP/1.1* connections at six per host; WebSockets are not subject to that,
 * so N sockets from one page is fine.
 *
 * # Sizing
 *
 * `TORTURE_SESSIONS` selects the tier: 5, 10, 25, 50, 100. Five is cheap
 * enough to run often; 100 is deliberately far past what one GM could ever
 * assemble, so passing it says the ceiling is somewhere we will not reach by
 * accident.
 *
 * # Ephemerality
 *
 * Driven by `scripts/torture.mjs`, which stands up an isolated Postgres and
 * RustFS on tmpfs. Nothing here touches a database anyone cares about.
 */

const SESSIONS = Number(process.env.TORTURE_SESSIONS ?? "5");

/** One world-scoped chat message — the storm's unit of work. */
async function sendChat(page: Page, worldId: string, body: string) {
  const res = await graphql<{
    data?: { sendChatMessage?: { id: string } };
    errors?: { message: string }[];
  }>(
    page,
    `
      mutation ($input: SendChatMessageInput!) {
        sendChatMessage(input: $input) {
          id
        }
      }
    `,
    { input: { worldId, body } },
  );
  // Assert the stimulus actually happened. Not doing this is what made the
  // first run look like total fan-out starvation when `createScene` simply
  // emitted nothing — a storm that silently fails to fire is indistinguishable
  // from a fan-out that silently fails to deliver, and only one of those is
  // worth investigating.
  if (res.errors?.length || !res.data?.sendChatMessage?.id) {
    throw new Error(
      `chat mutation failed, so no event was emitted: ${JSON.stringify(res.errors ?? res)}`,
    );
  }
  return res;
}
/** Events every subscriber must see before fan-out counts as healthy. */
const EVENTS_EXPECTED = 3;

interface StormResult {
  connected: number;
  received: number[];
  errors: string[];
}

test.describe(`Session storm — ${SESSIONS} concurrent subscribers`, () => {
  // Generous by design: this looks for a breaking point, not a latency
  // budget. A tier that needs longer than this is itself the finding.
  test.setTimeout(SESSIONS * 2_000 + 180_000);

  test(`${SESSIONS} sessions subscribe to one world and none are starved of events`, async ({
    page,
  }) => {
    const worldId = await registerAndCreateWorld(
      page,
      `Torture ${SESSIONS} ${uniqueSuffix()}`,
      "torture",
    );

    // Open N subscriptions inside the page and leave them running. Returns
    // once every socket has acknowledged its subscription, so the mutation
    // storm below cannot start before anyone is listening — a race that
    // would otherwise look like dropped events.
    const connected = await page.evaluate(
      async ({ worldId, sessions }) => {
        const w = window as unknown as { __torture?: unknown };
        const sockets: WebSocket[] = [];
        const received: number[] = new Array(sessions).fill(0);
        const errors: string[] = [];
        const url = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`;

        const ready = await Promise.all(
          Array.from({ length: sessions }, (_, i) => {
            return new Promise<boolean>((resolve) => {
              // `graphql-transport-ws` is the subprotocol `graphql-ws`
              // speaks; hand-rolling the three messages avoids bundling a
              // client into page scope just to send them.
              const socket = new WebSocket(url, "graphql-transport-ws");
              sockets.push(socket);
              const giveUp = setTimeout(() => resolve(false), 45_000);

              socket.onopen = () =>
                socket.send(JSON.stringify({ type: "connection_init" }));

              socket.onmessage = (event) => {
                const msg = JSON.parse(event.data as string);
                if (msg.type === "connection_ack") {
                  socket.send(
                    JSON.stringify({
                      id: String(i),
                      type: "subscribe",
                      payload: {
                        query: `subscription($worldId: UUID!) {
                          worldEventsCreated(worldId: $worldId) { id }
                        }`,
                        variables: { worldId },
                      },
                    }),
                  );
                  clearTimeout(giveUp);
                  resolve(true);
                } else if (msg.type === "next") {
                  received[i] += 1;
                } else if (msg.type === "error") {
                  errors.push(`socket ${i}: ${JSON.stringify(msg.payload)}`);
                }
              };

              socket.onerror = () => {
                errors.push(`socket ${i}: transport error`);
                clearTimeout(giveUp);
                resolve(false);
              };
            });
          }),
        );

        // Park state on window so a later evaluate can read counts without
        // re-opening anything.
        (w as { __torture: unknown }).__torture = { sockets, received, errors };
        return ready.filter(Boolean).length;
      },
      { worldId, sessions: SESSIONS },
    );

    expect(
      connected,
      `only ${connected}/${SESSIONS} sockets completed the subscribe handshake`,
    ).toBe(SESSIONS);

    // Prove delivery is live before measuring anything.
    //
    // A socket acknowledges `connection_init` before the server has processed
    // its `subscribe`, so a fixed sleep here is a guess — and the first run
    // that used one produced distribution={"2":5}: every subscriber missing
    // exactly the same single event. Uniform, so a registration race rather
    // than fan-out loss, but indistinguishable from real loss in the summary.
    //
    // So instead of sleeping longer, send one warm-up event and wait until
    // every subscriber has actually received something. Then reset the
    // counters and start the real storm. That removes the guess entirely,
    // and — importantly — does not hide the failure this test looks for: if
    // fan-out later drops messages under burst, the distribution goes ragged
    // and the assertion still catches it.
    await sendChat(page, worldId, `warmup ${uniqueSuffix()}`);

    await expect
      .poll(
        async () =>
          page.evaluate(() => {
            const t = (
              window as unknown as { __torture: { received: number[] } }
            ).__torture;
            return t.received.filter((c) => c > 0).length;
          }),
        {
          message: "every subscriber should receive the warm-up event",
          timeout: 30_000,
        },
      )
      .toBe(SESSIONS);

    // Counters zeroed: only the storm below is measured.
    await page.evaluate(() => {
      const t = (window as unknown as { __torture: { received: number[] } })
        .__torture;
      t.received.fill(0);
    });

    // Drive real events with chat messages.
    //
    // Deliberately NOT scene creation, which was tried first and produced a
    // clean five-out-of-five "starvation" that was entirely self-inflicted:
    // `createScene` writes no `world_events` row at all, so the storm drove
    // nothing and every subscriber correctly reported zero. Chat is
    // world-scoped, needs no scene or token fixture, and broadcasts on the
    // same bus every client watches.
    for (let n = 0; n < EVENTS_EXPECTED; n += 1) {
      await sendChat(page, worldId, `torture ${n} ${uniqueSuffix()}`);
      // Spaced, not bursted. TORTURE_EVENT_SPACING_MS lets a diagnosis
      // distinguish two very different failures that look identical in the
      // totals: events being coalesced or dropped under burst (spacing fixes
      // it) versus only the first event after subscribe ever arriving
      // (spacing changes nothing).
      const spacing = Number(process.env.TORTURE_EVENT_SPACING_MS ?? "0");
      if (spacing > 0) await page.waitForTimeout(spacing);
    }

    // Settle window scales with tier: the server writes one notification per
    // subscriber, so 100 legitimately needs longer than 5.
    // Wait for delivery to actually settle rather than guessing at it: poll
    // until counts stop moving, with a floor so a genuinely slow tier still
    // gets time. A fixed sleep here cannot distinguish "not delivered" from
    // "not delivered yet", and those mean opposite things.
    let previousTotal = -1;
    for (let stableRounds = 0; stableRounds < 3; ) {
      await page.waitForTimeout(1_000);
      const total = await page.evaluate(() => {
        const t = (window as unknown as { __torture: { received: number[] } })
          .__torture;
        return t.received.reduce((a, b) => a + b, 0);
      });
      stableRounds = total === previousTotal ? stableRounds + 1 : 0;
      previousTotal = total;
    }

    const result = await page.evaluate<StormResult>(() => {
      const t = (
        window as unknown as {
          __torture: { received: number[]; errors: string[] };
        }
      ).__torture;
      return {
        connected: t.received.length,
        received: t.received,
        errors: t.errors,
      };
    });

    const starved = result.received
      .map((count, i) => ({ count, i }))
      .filter((s) => s.count === 0);
    const partial = result.received.filter(
      (c) => c > 0 && c < EVENTS_EXPECTED,
    ).length;

    // The distribution, not just the tally. A uniform shortfall (everyone
    // missing exactly the same count) means a startup race on the first
    // events; a ragged one means fan-out is genuinely losing messages under
    // burst. Those need completely different fixes, and the summary counts
    // alone cannot tell them apart.
    const histogram = result.received.reduce<Record<number, number>>(
      (acc, c) => ({ ...acc, [c]: (acc[c] ?? 0) + 1 }),
      {},
    );
    console.log(
      `[torture] tier=${SESSIONS} expected=${EVENTS_EXPECTED} ` +
        `starved=${starved.length} partial=${partial} ` +
        `full=${result.received.filter((c) => c >= EVENTS_EXPECTED).length} ` +
        `errors=${result.errors.length} ` +
        `distribution=${JSON.stringify(histogram)}`,
    );

    // Starvation is the failure that matters. A subscriber receiving nothing
    // while its peers receive everything means fan-out is silently dropping
    // clients under load — far worse than being slow, because nothing
    // surfaces it at runtime.
    expect(
      starved.map((s) => s.i),
      `${starved.length}/${SESSIONS} subscribers received no events at all`,
    ).toEqual([]);

    expect(result.errors, "transport errors during the storm").toEqual([]);

    // Partial delivery is reported, not failed.
    //
    // KNOWN, UNRESOLVED as of 2026-08-26: at tier 5 a burst of three events
    // delivers two per subscriber (distribution={"2":5}); the same three
    // spaced 1500ms apart deliver all three ({"3":5}). Set
    // TORTURE_EVENT_SPACING_MS to reproduce the difference.
    //
    // One cause of this was found and fixed — `poll_new_events_with_conn`
    // selected events `DESC` and skipped `id <= cursor` in the loop, so the
    // first (newest) row advanced the cursor past every older row in the same
    // batch. That took burst delivery from 1/3 to 2/3. The remaining shortfall
    // is not understood, and is deliberately left visible here rather than
    // absorbed by a longer settle: it is a real difference between bursted
    // and spaced events, and the assertion below still guards the failure
    // that matters most (total starvation).
    if (partial > 0) {
      console.warn(
        `[torture] ${partial}/${SESSIONS} received a partial event set — investigate if this grows with tier`,
      );
    }

    // Cleanup. The containers are discarded anyway, but keeping this means
    // the test stays safe to point at a persistent database, which someone
    // will eventually do.
    await graphql(
      page,
      `
        mutation ($id: UUID!) {
          deleteWorld(id: $id) {
            id
          }
        }
      `,
      {
        id: worldId,
      },
    ).catch(() => {
      // A failed cleanup must never turn a passing load test red.
    });
  });
});
