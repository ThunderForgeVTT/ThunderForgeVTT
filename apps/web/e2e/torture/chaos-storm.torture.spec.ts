import { expect, test } from "@playwright/test";
import { registerAndCreateWorld, uniqueSuffix } from "../fixtures/helpers";

/**
 * Churn storm: subscribers arriving and leaving while events are flowing.
 *
 * # Why this one exists, specifically
 *
 * The other three torture tests all measure a *stable* system under load —
 * N sockets that connect, receive, and are counted. Nothing in them
 * subscribes and unsubscribes while the router is delivering, which is the
 * state a real deployment spends most of its time in: people opening a
 * second tab, closing a laptop, walking through a tunnel, and every one of
 * them at once after a deploy.
 *
 * That gap was not theoretical. `WorldRouter::reap` counted the channels it
 * released as `len()` before minus `len()` after — two independently
 * observed lengths of a concurrent map. Reaping runs on a timer, so a single
 * subscription arriving mid-reap made the second length the larger, and the
 * unsigned subtraction panicked the tokio worker. In this session that
 * killed the live-event backplane mid-run: `world_events` rows kept being
 * written, `pg_notify` kept firing, and no client received anything, with
 * nothing in the client logs to say why. It was found by accident, from a
 * test failing for what looked like an unrelated reason.
 *
 * A unit test now covers the arithmetic. This covers the situation: real
 * sockets, real subscribe and unsubscribe traffic, real publishing, against
 * the running server.
 *
 * # The three things asserted, in order of what they would have caught
 *
 * 1. **The stable cohort loses nothing.** Sockets that never leave must
 *    receive every message published while they were connected. Churn around
 *    them is not their problem.
 * 2. **The server is still healthy afterwards.** `/api/readyz` must answer.
 *    A panicked worker does not necessarily stop the process — that is
 *    exactly what made the original bug so quiet.
 * 3. **Delivery still works afterwards.** A final canary message, published
 *    after the storm, must reach the stable cohort. This is the assertion
 *    the original failure would have tripped: the server was up, answering
 *    HTTP, and no longer delivering a single event.
 */

/** Sockets that stay for the whole run and must miss nothing. */
const STABLE = 5;

/** How many sockets churn, scaled by the tier. */
const CHURN = Math.max(4, Number(process.env.TORTURE_SESSIONS ?? "5"));

/** Connect/disconnect rounds for the churning cohort. */
const ROUNDS = 6;

/** Messages published during the storm. */
const MESSAGES = 12;

test(`${CHURN} sockets churning through ${ROUNDS} rounds while ${MESSAGES} messages fly`, async ({
  page,
}) => {
  // Generous by design: this looks for a breaking point, not a latency
  // budget.
  test.setTimeout(20 * 60_000);

  const worldId = await registerAndCreateWorld(
    page,
    `Torture Chaos ${uniqueSuffix()}`,
  );

  const result = await page.evaluate(
    async ({ world, stable, churn, rounds, messages }) => {
      const url = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`;
      const csrf = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);

      const socketErrors: string[] = [];

      /** Open a socket already subscribed to this world, counting arrivals. */
      const open = async (received: { count: number }) => {
        const socket = new WebSocket(url, "graphql-transport-ws");
        await new Promise<void>((resolve, reject) => {
          const timer = setTimeout(
            () => reject(new Error("subscription never acknowledged")),
            30_000,
          );
          socket.addEventListener("message", (event) => {
            const message = JSON.parse(String(event.data));
            if (message.type === "connection_ack") {
              socket.send(
                JSON.stringify({
                  id: "s",
                  type: "subscribe",
                  payload: {
                    query: `subscription ($worldId: UUID!) {
                      worldEventsCreated(worldId: $worldId) { id }
                    }`,
                    variables: { worldId: world },
                  },
                }),
              );
              clearTimeout(timer);
              resolve();
              return;
            }
            if (message.type === "next" && message.id === "s") {
              received.count += 1;
            }
            if (message.type === "error") {
              socketErrors.push(JSON.stringify(message.payload));
            }
          });
          socket.addEventListener("error", () => {
            clearTimeout(timer);
            reject(new Error("socket errored while connecting"));
          });
          socket.addEventListener("open", () => {
            socket.send(JSON.stringify({ type: "connection_init" }));
          });
        });
        return socket;
      };

      const publish = async (body: string) => {
        const res = await fetch("/api/graphql", {
          method: "POST",
          credentials: "same-origin",
          headers: {
            "Content-Type": "application/json",
            ...(csrf ? { "x-csrf-token": csrf } : {}),
          },
          body: JSON.stringify({
            query: `mutation ($input: SendChatMessageInput!) {
              sendChatMessage(input: $input) { id }
            }`,
            variables: { input: { worldId: world, body } },
          }),
        });
        const parsed = await res.json();
        if (parsed.errors) {
          socketErrors.push(`publish failed: ${JSON.stringify(parsed.errors)}`);
        }
      };

      // The cohort that never leaves. Connected and acknowledged before
      // anything is published, so a missed message is a real loss rather
      // than one sent before anyone was listening.
      const stableCounts = Array.from({ length: stable }, () => ({ count: 0 }));
      await Promise.all(stableCounts.map((counter) => open(counter)));
      await new Promise((resolve) => setTimeout(resolve, 1_000));

      // The storm: churn and publishing at the same time, deliberately
      // overlapping. Reaping is on a timer server-side, so the interesting
      // moment is a subscribe landing while a release is in flight — which
      // cannot be arranged from here, only made likely by doing both
      // continuously.
      const churning = (async () => {
        for (let round = 0; round < rounds; round += 1) {
          const sockets = await Promise.all(
            Array.from({ length: churn }, () => open({ count: 0 })),
          );
          // Held briefly, then all dropped at once — the shape of a deploy,
          // or a wifi blip across a table.
          await new Promise((resolve) => setTimeout(resolve, 300));
          for (const socket of sockets) socket.close();
          await new Promise((resolve) => setTimeout(resolve, 200));
        }
      })();

      const publishing = (async () => {
        for (let n = 0; n < messages; n += 1) {
          await publish(`storm ${n}`);
          await new Promise((resolve) => setTimeout(resolve, 150));
        }
      })();

      await Promise.all([churning, publishing]);

      // Let the stable cohort catch up on anything still in flight.
      const deadline = Date.now() + 60_000;
      while (
        stableCounts.some((counter) => counter.count < messages) &&
        Date.now() < deadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 250));
      }

      const duringStorm = stableCounts.map((counter) => counter.count);

      // The canary. Published after all churn has stopped, on a server that
      // has just been through it. If the backplane died during the storm the
      // process is still up and still answering HTTP — this is the only
      // thing that notices.
      const before = stableCounts.map((counter) => counter.count);
      await publish("canary");
      const canaryDeadline = Date.now() + 30_000;
      while (
        stableCounts.some((counter, index) => counter.count <= before[index]) &&
        Date.now() < canaryDeadline
      ) {
        await new Promise((resolve) => setTimeout(resolve, 250));
      }
      const canaryHeard = stableCounts.filter(
        (counter, index) => counter.count > before[index],
      ).length;

      const ready = await fetch("/api/readyz").then((res) => res.status);

      return { duringStorm, canaryHeard, ready, socketErrors };
    },
    {
      world: worldId,
      stable: STABLE,
      churn: CHURN,
      rounds: ROUNDS,
      messages: MESSAGES,
    },
  );

  const starved = result.duringStorm.filter((count) => count < MESSAGES).length;

  // the other torture specs so every tier reads the same way in a log.
  console.log(
    `[torture] churn=${CHURN}x${ROUNDS} messages=${MESSAGES} ` +
      `starved=${starved} canaryHeard=${result.canaryHeard}/${STABLE} ` +
      `readyz=${result.ready} errors=${result.socketErrors.length}`,
  );

  expect(
    result.socketErrors,
    "no subscription or publish may fail while others come and go",
  ).toEqual([]);

  expect(
    starved,
    "a socket that never left must receive every message published while it was there",
  ).toBe(0);

  expect(result.ready, "the server must still be ready after the storm").toBe(
    200,
  );

  // The one the original bug would have tripped, and the reason this test is
  // shaped around a canary rather than ending at the last storm message: a
  // panicked worker leaves a server that is up, answering HTTP, and quietly
  // delivering nothing.
  expect(
    result.canaryHeard,
    "delivery must still work after the storm — a server that stopped notifying " +
      "still answers /readyz, which is exactly what made this failure so quiet",
  ).toBe(STABLE);
});
