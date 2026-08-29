import { expect, test, type BrowserContext, type Page } from "@playwright/test";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "../fixtures/helpers";

/**
 * A thousand tables playing at once.
 *
 * # Why this shape, and not more people on one world
 *
 * `fanout-storm` puts a thousand subscribers on a single world and passes
 * without the server noticing — which is what the fan-out measurements
 * predict, because publishing writes the world's ring once and every listener
 * reads from it. The marginal cost of the thousandth subscriber is about a
 * nanosecond. That path is not where a real deployment runs out.
 *
 * The untested path is *breadth*. The delivery loop polls
 * `ORDER BY id ASC LIMIT 256` across **every** world with no per-world
 * fairness, so a thousand worlds share one 256-row window every 100ms, and
 * each world holds its own broadcast channel in a map that has to stay
 * navigable. Nothing here had ever been run against more than a handful of
 * simultaneous worlds.
 *
 * So: `TORTURE_WORLDS` tables of `TORTURE_PLAYERS_PER_WORLD` players each,
 * every player subscribed to their own table and nobody else's.
 *
 * # The assertion is exactness, not arrival
 *
 * One event is published per world, and every socket must receive **exactly
 * one**. Fewer is delivery loss. *More* is a leak — a subscriber hearing a
 * table that is not theirs — and at this width that is the failure worth
 * catching, because a per-world routing mistake that is invisible with five
 * worlds is obvious with a thousand.
 *
 * # Why one account owns every world
 *
 * Membership resolves through `worlds.created_by` for a world's owner, so a
 * single account subscribing to worlds it created is a legitimate member of
 * all of them. A thousand real registrations would spend the run on the auth
 * rate limiter and measure that instead.
 */

/** Sockets per browser context, under Chromium's per-host WebSocket cap. */
const SOCKETS_PER_CONTEXT = 200;

const WORLDS = Number(process.env.TORTURE_WORLDS ?? "10");
const PLAYERS_PER_WORLD = Number(process.env.TORTURE_PLAYERS_PER_WORLD ?? "10");
const SUBSCRIBERS = WORLDS * PLAYERS_PER_WORLD;

/** Concurrency for the setup mutations. Enough to be quick, not a storm. */
const SETUP_IN_FLIGHT = 24;

interface Shard {
  page: Page;
  context: BrowserContext | null;
  /** One world id per socket this shard opens, in order. */
  worlds: string[];
}

/** Run `jobs` with bounded concurrency, preserving order of results. */
async function pooled<T>(
  jobs: (() => Promise<T>)[],
  inFlight: number,
): Promise<T[]> {
  const results: T[] = new Array(jobs.length);
  let next = 0;
  const workers = Array.from(
    { length: Math.min(inFlight, jobs.length) },
    async () => {
      for (;;) {
        const index = next;
        next += 1;
        if (index >= jobs.length) return;
        results[index] = await jobs[index]();
      }
    },
  );
  await Promise.all(workers);
  return results;
}

/**
 * Open one subscription per entry in `worlds`, inside this page.
 *
 * Each socket subscribes to its own world, so a socket receiving anything at
 * all from another world is a routing failure the counters will show.
 */
async function openSockets(page: Page, worlds: string[]): Promise<number> {
  return page.evaluate(
    async ({ worlds }) => {
      const received: number[] = new Array(worlds.length).fill(0);
      const errors: string[] = [];
      const sockets: WebSocket[] = [];
      const url = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`;

      const ready = await Promise.all(
        worlds.map((worldId, i) => {
          return new Promise<boolean>((resolve) => {
            const socket = new WebSocket(url, "graphql-transport-ws");
            sockets.push(socket);
            const giveUp = setTimeout(() => resolve(false), 120_000);

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

      (window as unknown as { __worldstorm: unknown }).__worldstorm = {
        sockets,
        received,
        errors,
      };
      return ready.filter(Boolean).length;
    },
    { worlds },
  );
}

async function receivedCounts(shards: Shard[]): Promise<number[]> {
  const perShard = await Promise.all(
    shards.map((shard) =>
      shard.page.evaluate(
        () =>
          (window as unknown as { __worldstorm: { received: number[] } })
            .__worldstorm.received,
      ),
    ),
  );
  return perShard.flat();
}

test(`${WORLDS} worlds of ${PLAYERS_PER_WORLD} all hear their own table and no other`, async ({
  page,
  browser,
}) => {
  test.setTimeout(45 * 60_000);

  const suffix = uniqueSuffix();
  await registerAndCreateWorld(page, `World Storm ${suffix}`, "torture");

  // Every world this account creates, it also owns — which is what makes a
  // single session a legitimate subscriber to all of them.
  const worldIds = await pooled(
    Array.from({ length: WORLDS }, (_, w) => async () => {
      const created = await graphql<{
        data?: { createWorld?: { id: string } };
        errors?: { message: string }[];
      }>(
        page,
        `
          mutation ($input: GraphQLCreateWorldInput!) {
            createWorld(input: $input) {
              id
            }
          }
        `,
        { input: { name: `Storm ${w} ${suffix}` } },
      );
      const id = created.data?.createWorld?.id;
      if (!id) {
        throw new Error(
          `world ${w} was not created: ${JSON.stringify(created.errors ?? created)}`,
        );
      }
      return id;
    }),
    SETUP_IN_FLIGHT,
  );

   
  console.log(`[torture] created ${worldIds.length} worlds`);

  // One socket per player per world, laid out flat and then cut into shards.
  const assignments: string[] = [];
  for (const worldId of worldIds) {
    for (let p = 0; p < PLAYERS_PER_WORLD; p += 1) assignments.push(worldId);
  }

  const storageState = await page.context().storageState();
  const shards: Shard[] = [];
  for (
    let start = 0;
    start < assignments.length;
    start += SOCKETS_PER_CONTEXT
  ) {
    const slice = assignments.slice(start, start + SOCKETS_PER_CONTEXT);
    if (start === 0) {
      shards.push({ page, context: null, worlds: slice });
      continue;
    }
    const context = await browser.newContext({ storageState });
    const shardPage = await context.newPage();
    await shardPage.goto("/");
    shards.push({ page: shardPage, context, worlds: slice });
  }

  try {
    const connected = (
      await Promise.all(
        shards.map((shard) => openSockets(shard.page, shard.worlds)),
      )
    ).reduce((total, n) => total + n, 0);

    expect(
      connected,
      `only ${connected}/${SUBSCRIBERS} sockets subscribed across ${shards.length} context(s)`,
    ).toBe(SUBSCRIBERS);

     
    console.log(
      `[torture] ${connected} sockets live across ${shards.length} contexts`,
    );

    // Exactly one event per world. Every socket should see its own and
    // nothing else.
    await pooled(
      worldIds.map((worldId, w) => async () => {
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
          { input: { worldId, body: `storm ${w} ${suffix}` } },
        );
        if (res.errors?.length || !res.data?.sendChatMessage?.id) {
          throw new Error(
            `world ${w} emitted no event: ${JSON.stringify(res.errors ?? res)}`,
          );
        }
      }),
      SETUP_IN_FLIGHT,
    );

    await expect
      .poll(
        async () => (await receivedCounts(shards)).filter((c) => c >= 1).length,
        {
          message: "every subscriber should receive their own world's event",
          timeout: 10 * 60_000,
          intervals: [2_000],
        },
      )
      .toBe(SUBSCRIBERS);

    const counts = await receivedCounts(shards);
    const distribution: Record<number, number> = {};
    for (const c of counts) distribution[c] = (distribution[c] ?? 0) + 1;
    const starved = counts.filter((c) => c === 0).length;
    const leaked = counts.filter((c) => c > 1).length;

     
    console.log(
      `[torture] worlds=${WORLDS} playersPerWorld=${PLAYERS_PER_WORLD} ` +
        `subscribers=${SUBSCRIBERS} shards=${shards.length} ` +
        `starved=${starved} leaked=${leaked} ` +
        `distribution=${JSON.stringify(distribution)}`,
    );

    expect(starved, "no subscriber may be starved of their own table").toBe(0);
    expect(
      leaked,
      "a subscriber receiving more than its own world's single event is hearing another table",
    ).toBe(0);
  } finally {
    for (const shard of shards) {
      if (shard.context) await shard.context.close();
    }
  }
});
