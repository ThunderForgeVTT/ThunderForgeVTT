import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "../fixtures/helpers";

/**
 * Fan-out at a scale one page cannot reach.
 *
 * # Why this exists alongside `session-storm`
 *
 * `session-storm` opens every subscription inside a single page, on the stated
 * reasoning that "browsers cap HTTP/1.1 connections at six per host;
 * WebSockets are not subject to that, so N sockets from one page is fine".
 * The first half is true and the second is not. Chromium enforces a separate
 * per-host cap on WebSockets, and a tier-1000 run measured it exactly:
 *
 *     only 254/1000 sockets completed the subscribe handshake
 *
 * That is the browser refusing to open a 256th socket to one host, not the
 * server refusing to serve one. A test that stops there reports a client
 * limit as though it were a capacity finding, which is the most misleading
 * shape a load test can take.
 *
 * So this spec shards its sockets across browser contexts, each of which gets
 * its own renderer and therefore its own allowance. `session-storm` remains
 * the right test up to ~200; this one is for the sizes past it.
 *
 * # What it measures, and what it deliberately does not
 *
 * The transport and fan-out only: the `graphql-ws` subscription, the per-world
 * broadcast, and the poll loop feeding it. No engine, no canvas, no WASM —
 * instantiating the bundle a thousand times would measure this machine's RAM
 * and nothing about the server.
 *
 * Every subscriber authenticates as the same account. That is deliberate: the
 * question here is what the fan-out path costs per *connection*, and creating
 * a thousand real accounts would spend the run on the registration rate
 * limiter instead.
 */

/** Sockets per context. Comfortably under Chromium's per-host WebSocket cap. */
const SOCKETS_PER_CONTEXT = 200;

/** How many subscribers to open in total. */
const SUBSCRIBERS = Number(process.env.TORTURE_SESSIONS ?? "5");

/** Events published once everybody is listening. */
const EVENTS = 3;

interface Shard {
  page: Page;
  context: BrowserContext | null;
  sockets: number;
}

/** Open `count` subscriptions inside one page and leave them running. */
async function openSockets(
  page: Page,
  worldId: string,
  count: number,
): Promise<number> {
  return page.evaluate(
    async ({ worldId, count }) => {
      const sockets: WebSocket[] = [];
      const received: number[] = new Array(count).fill(0);
      const errors: string[] = [];
      const url = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`;

      const ready = await Promise.all(
        Array.from({ length: count }, (_, i) => {
          return new Promise<boolean>((resolve) => {
            const socket = new WebSocket(url, "graphql-transport-ws");
            sockets.push(socket);
            const giveUp = setTimeout(() => resolve(false), 60_000);

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

      (window as unknown as { __fanout: unknown }).__fanout = {
        sockets,
        received,
        errors,
      };
      return ready.filter(Boolean).length;
    },
    { worldId, count },
  );
}

/** Every subscriber's event count, across every shard. */
async function receivedCounts(shards: Shard[]): Promise<number[]> {
  const perShard = await Promise.all(
    shards.map((shard) =>
      shard.page.evaluate(
        () =>
          (window as unknown as { __fanout: { received: number[] } }).__fanout
            .received,
      ),
    ),
  );
  return perShard.flat();
}

/** Zero the counters after the warm-up, before the measured storm. */
async function resetCounts(shards: Shard[]): Promise<void> {
  await Promise.all(
    shards.map((shard) =>
      shard.page.evaluate(() => {
        const t = (window as unknown as { __fanout: { received: number[] } })
          .__fanout;
        t.received.fill(0);
      }),
    ),
  );
}

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
  // A storm that silently fails to fire is indistinguishable from a fan-out
  // that silently fails to deliver, and only one of those is worth chasing.
  if (res.errors?.length || !res.data?.sendChatMessage?.id) {
    throw new Error(
      `chat mutation failed, so no event was emitted: ${JSON.stringify(res.errors ?? res)}`,
    );
  }
}

test(`${SUBSCRIBERS} subscribers across shards all hear every event`, async ({
  page,
  browser,
}) => {
  test.setTimeout(15 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Fanout ${SUBSCRIBERS} ${uniqueSuffix()}`,
    "torture",
  );

  // Every shard reuses this session, so no shard pays for its own
  // registration — and the rate limiter never enters the picture.
  const storageState = await page.context().storageState();

  const shardSizes: number[] = [];
  for (let left = SUBSCRIBERS; left > 0; left -= SOCKETS_PER_CONTEXT) {
    shardSizes.push(Math.min(SOCKETS_PER_CONTEXT, left));
  }

  const shards: Shard[] = [];
  for (const [index, size] of shardSizes.entries()) {
    if (index === 0) {
      shards.push({ page, context: null, sockets: size });
      continue;
    }
    const context = await browser.newContext({ storageState });
    const shardPage = await context.newPage();
    // Any same-origin document will do; the sockets are opened by script and
    // the page itself is never rendered against.
    await shardPage.goto("/");
    shards.push({ page: shardPage, context, sockets: size });
  }

  try {
    const connected = (
      await Promise.all(
        shards.map((shard) => openSockets(shard.page, worldId, shard.sockets)),
      )
    ).reduce((total, n) => total + n, 0);

    expect(
      connected,
      `only ${connected}/${SUBSCRIBERS} sockets completed the subscribe handshake ` +
        `across ${shards.length} context(s)`,
    ).toBe(SUBSCRIBERS);

    // Prove delivery is live before measuring. A socket acknowledges
    // `connection_init` before the server has processed its `subscribe`, so
    // any fixed sleep here is a guess — and a guess that is too short reports
    // a registration race as fan-out loss.
    await sendChat(page, worldId, `warmup ${uniqueSuffix()}`);
    await expect
      .poll(
        async () => (await receivedCounts(shards)).filter((c) => c > 0).length,
        {
          message: "every subscriber should receive the warm-up event",
          timeout: 120_000,
        },
      )
      .toBe(SUBSCRIBERS);

    await resetCounts(shards);

    for (let i = 0; i < EVENTS; i += 1) {
      await sendChat(page, worldId, `storm ${i} ${uniqueSuffix()}`);
    }

    await expect
      .poll(
        async () =>
          (await receivedCounts(shards)).filter((c) => c >= EVENTS).length,
        {
          message: `every subscriber should receive all ${EVENTS} storm events`,
          timeout: 180_000,
        },
      )
      .toBe(SUBSCRIBERS);

    const counts = await receivedCounts(shards);
    const distribution: Record<number, number> = {};
    for (const c of counts) {
      distribution[c] = (distribution[c] ?? 0) + 1;
    }
    const starved = counts.filter((c) => c === 0).length;
    const full = counts.filter((c) => c >= EVENTS).length;

     
    console.log(
      `[torture] fanout=${SUBSCRIBERS} shards=${shards.length} ` +
        `expected=${EVENTS} starved=${starved} full=${full} ` +
        `distribution=${JSON.stringify(distribution)}`,
    );

    expect(starved, "no subscriber may be starved").toBe(0);
    expect(full, "every subscriber must receive every event").toBe(SUBSCRIBERS);
  } finally {
    for (const shard of shards) {
      if (shard.context) await shard.context.close();
    }
  }
});
