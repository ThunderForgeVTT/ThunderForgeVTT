import { expect, test } from "@playwright/test";
import { registerAndCreateWorld, uniqueSuffix } from "../fixtures/helpers";

/**
 * Many-to-one torture test: N writers, one world, one listener.
 *
 * # The shape this covers that `session-storm` does not
 *
 * `session-storm` is **one-to-many**: a single publisher sends, and N
 * subscribed sockets must each receive everything. It strains fan-out — the
 * broadcast, the per-world router, the NOTIFY behind them.
 *
 * This is the other direction. N writers all mutate the *same world* at once
 * while one socket listens, and every write must arrive exactly once. What
 * comes under strain here is different in kind: the connection pool, the
 * per-world event sequence, and whatever ordering guarantee the router
 * actually provides when writes arrive concurrently rather than in turn.
 *
 * A table is far more likely to hit this than the fan-out case. Six players
 * dragging tokens during a fight is many-to-one; one GM narrating to six
 * players is one-to-many, and the second is the easier problem.
 *
 * # Why the writes are fired from the page, in parallel
 *
 * The mutation is authenticated by session cookie and guarded by CSRF, both
 * of which the page has and the test process does not. Firing from inside the
 * page gets that for free and exercises the path a real client takes.
 *
 * Browsers cap HTTP/1.1 at six concurrent connections per host, so N writes
 * do not all leave at the same instant — they leave in waves of six. That is
 * a real limit of this harness and worth stating plainly rather than
 * pretending otherwise: this measures sustained concurrent write pressure,
 * not a simultaneous thundering herd. The server-side contention it creates
 * — pool checkout, transaction interleaving, sequence assignment — is the
 * same either way, and it is the part that breaks.
 *
 * # What "exactly once" means here
 *
 * Every write must reach the listener, and no write may arrive twice.
 * Losing one is the obvious failure. Duplicating one is the quieter failure
 * and the more damaging: a chat line shown twice is cosmetic, but the same
 * mechanism carries token movement, where a replayed event is a token that
 * jumps back.
 */

/** Concurrent writers. Shares the tier variable with `session-storm`. */
const WRITERS = Number(process.env.TORTURE_SESSIONS ?? "5");

/** Messages each writer sends. Enough to interleave, few enough to stay cheap. */
const WRITES_EACH = 3;

const TOTAL_WRITES = WRITERS * WRITES_EACH;

test(`${WRITERS} writers into one world, every write seen exactly once`, async ({
  page,
}) => {
  // Generous by design: this looks for a breaking point, not a latency
  // budget. A slow pass is still a pass.
  test.setTimeout(15 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Torture Writes ${uniqueSuffix()}`,
  );

  const result = await page.evaluate(
    async ({ world, writers, writesEach }) => {
      const csrfToken = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);

      const url = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`;
      const seen: string[] = [];
      const errors: string[] = [];

      // One listener, subscribed and acknowledged *before* any write goes
      // out. Subscribing after would make a lost event indistinguishable
      // from an event that was published before anyone was listening.
      const socket = new WebSocket(url, "graphql-transport-ws");
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(
          () => reject(new Error("listener did not connect")),
          60_000,
        );
        socket.addEventListener("error", () => {
          clearTimeout(timer);
          reject(new Error("listener socket errored"));
        });
        socket.addEventListener("message", (event) => {
          const message = JSON.parse(String(event.data));
          if (message.type === "connection_ack") {
            socket.send(
              JSON.stringify({
                id: "listener",
                type: "subscribe",
                payload: {
                  query: `subscription ($worldId: UUID!) {
                    worldEventsCreated(worldId: $worldId) { id eventCode }
                  }`,
                  variables: { worldId: world },
                },
              }),
            );
            clearTimeout(timer);
            resolve();
            return;
          }
          if (message.type === "next" && message.id === "listener") {
            const id = message.payload?.data?.worldEventsCreated?.id;
            if (id !== undefined && id !== null) seen.push(String(id));
          }
          if (message.type === "error") {
            errors.push(JSON.stringify(message.payload));
          }
        });
        socket.addEventListener("open", () => {
          socket.send(JSON.stringify({ type: "connection_init" }));
        });
      });

      // A beat, so the subscription is registered server-side before the
      // first write. Without it the race this test exists to measure is
      // contaminated by a different one.
      await new Promise((resolve) => setTimeout(resolve, 1_000));

      const send = async (writer: number, index: number) => {
        const res = await fetch("/api/graphql", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
            ...(csrfToken ? { "x-csrf-token": csrfToken } : {}),
          },
          body: JSON.stringify({
            query: `mutation ($input: SendChatMessageInput!) {
              sendChatMessage(input: $input) { id }
            }`,
            variables: {
              input: { worldId: world, body: `w${writer}-${index}` },
            },
          }),
        });
        const text = await res.text();
        let parsed: { errors?: unknown } = {};
        try {
          parsed = JSON.parse(text);
        } catch {
          errors.push(
            `writer ${writer} got unparseable response: ${text.slice(0, 200)}`,
          );
          return;
        }
        if (parsed.errors) {
          errors.push(
            `writer ${writer} write ${index}: ${JSON.stringify(parsed.errors)}`,
          );
        }
      };

      // Every writer runs its own sequence concurrently with the others.
      // Within a writer the sends are ordered, which is what a single client
      // does; across writers they interleave, which is the point.
      await Promise.all(
        Array.from({ length: writers }, (_, writer) =>
          (async () => {
            for (let index = 0; index < writesEach; index += 1) {
              await send(writer, index);
            }
          })(),
        ),
      );

      // Let delivery catch up. The writes are acknowledged over HTTP before
      // the event reaches the socket, so ending here would measure the
      // notification's latency rather than whether it arrives at all.
      const deadline = Date.now() + 120_000;
      const expected = writers * writesEach;
      while (seen.length < expected && Date.now() < deadline) {
        await new Promise((resolve) => setTimeout(resolve, 250));
      }

      socket.close();
      return { seen, errors };
    },
    { world: worldId, writers: WRITERS, writesEach: WRITES_EACH },
  );

  const unique = new Set(result.seen);
  const duplicates = result.seen.length - unique.size;

  // session-storm's line so both tiers read the same way in a log.
  console.log(
    `[torture] writers=${WRITERS} writes=${TOTAL_WRITES} delivered=${unique.size} ` +
      `duplicates=${duplicates} errors=${result.errors.length}`,
  );

  expect(
    result.errors,
    "no write may be refused: a rejected mutation means the server buckled under " +
      "concurrent writes rather than merely slowing down",
  ).toEqual([]);

  expect(
    duplicates,
    "an event delivered twice is the quieter failure and the worse one — the same " +
      "path carries token movement, where a replayed event is a token that jumps back",
  ).toBe(0);

  expect(
    unique.size,
    "every write must reach the listener; a missing one is an edit a player made " +
      "that nobody else ever saw",
  ).toBe(TOTAL_WRITES);
});
