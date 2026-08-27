import { expect, test } from "@playwright/test";
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

    // Drive real events. Scene creation broadcasts on the same world_events
    // bus every client watches, so this exercises the production fan-out
    // rather than a synthetic broadcast.
    for (let n = 0; n < EVENTS_EXPECTED; n += 1) {
      await graphql(
        page,
        `
          mutation ($worldId: UUID!, $name: String!) {
            createScene(worldId: $worldId, name: $name) {
              sceneId
            }
          }
        `,
        { worldId, name: `torture-${n}-${uniqueSuffix()}` },
      );
    }

    // Settle window scales with tier: the server writes one notification per
    // subscriber, so 100 legitimately needs longer than 5.
    await page.waitForTimeout(3_000 + SESSIONS * 120);

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

    console.log(
      `[torture] tier=${SESSIONS} starved=${starved.length} partial=${partial} ` +
        `full=${result.received.filter((c) => c >= EVENTS_EXPECTED).length} ` +
        `errors=${result.errors.length}`,
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

    // Partial delivery is reported, not failed: the settle window is a guess
    // and asserting exact counts would make this a flaky timing test instead
    // of a load test. Growth in this number across tiers is the signal.
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
