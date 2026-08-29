import { expect, test, type BrowserContext } from "@playwright/test";

/**
 * Many tables, many real users: the topology an actual deployment has.
 *
 * # What the other two torture tests do not cover
 *
 * `session-storm` opens N sockets from **one** browser page, authenticated
 * as **one** user, subscribed to **one** world. That is a perfectly good
 * measurement of fan-out, and it is not a measurement of concurrent *users* —
 * a distinction worth keeping straight, because "100 sessions" and "100
 * users" are very different claims and only one of them is supported by
 * that test. `write-storm` is the same single user writing concurrently.
 *
 * This one uses distinct registered users, each with their own session
 * cookie, their own browser context, their own subscription, and a real role
 * — one Game Master per table, the rest players who joined by invite. It is
 * the shape a deployment actually has: many small tables, not one enormous
 * room.
 *
 * # The property worth proving, and why it is here rather than in a unit test
 *
 * Every user must receive everything published at *their* table, and
 * **nothing** published at anyone else's. `WorldRouter` already proves that
 * in isolation — `a_subscriber_is_never_woken_by_another_world` — but a unit
 * test proves it about a map, not about the running system: the subscription
 * resolver, the authorization check, the NOTIFY payload and the socket all
 * sit between that map and a player, and any of them could leak.
 *
 * It is also the claim the whole per-world topology was built to earn. The
 * router's own docs record the measurement: under the old global channel, an
 * event in a five-player world was cloned to every connected client in the
 * system — 200x wasted delivery at 1,000 connections, 20,000x at 100,000.
 * This is that argument, end to end, with real users.
 *
 * # Cost, and why the tiers mean something different here
 *
 * One user needs one browser context, because a session cookie is per
 * context — N users cannot share a page. Contexts here never load the
 * application, so no engine is instantiated and each is cheap, but they are
 * not free, and registration is a real Argon2id hash per user by design.
 *
 * So the tier is read as **total people**, split into tables of
 * `PLAYERS_PER_TABLE`. Tier 100 is ~17 tables of 6 — which is a far more
 * demanding and far more meaningful shape than 100 sockets in one room.
 */

/** Total distinct users, from the tier. */
const PEOPLE = Number(process.env.TORTURE_SESSIONS ?? "5");

/** A table: one GM and five players, which is a common shape. */
const PLAYERS_PER_TABLE = 6;

// At least two, always. One table cannot demonstrate isolation — there is
// nothing to be isolated from — so a single-table run would assert a
// property it had no way to violate.
const TABLES = Math.max(2, Math.ceil(PEOPLE / PLAYERS_PER_TABLE));

/** Messages each Game Master publishes to their own table. */
const MESSAGES_PER_TABLE = 3;

interface Person {
  context: BrowserContext;
  /** Index of the table this person belongs to. */
  table: number;
  isGm: boolean;
}

/**
 * Registrations already made, so the next one can wait for room.
 *
 * `auth_middleware` allows **15 requests per 60 seconds per IP** to
 * `/authentication/register` (and login), which is a deliberate protection
 * and not something a load test should route around. Registering a hundred
 * people in a burst trips it, and the failure looks like "could not
 * register" rather than "you are going too fast" — which is how it was
 * found.
 *
 * So this paces itself to the real limit. That makes a large tier slow to
 * set up, and the slowness is honest: it is the same ceiling anyone
 * onboarding a group from one office would hit.
 */
const registrationTimes: number[] = [];
const AUTH_WINDOW_MS = 60_000;
const AUTH_PER_WINDOW = 15;

async function waitForRegistrationSlot(): Promise<void> {
  for (;;) {
    const now = Date.now();
    while (
      registrationTimes.length > 0 &&
      now - registrationTimes[0] >= AUTH_WINDOW_MS
    ) {
      registrationTimes.shift();
    }
    // One below the limit: the window is enforced server-side against its
    // own clock, and sitting exactly on the boundary turns a slow moment
    // into a failed run.
    if (registrationTimes.length < AUTH_PER_WINDOW - 1) {
      registrationTimes.push(Date.now());
      return;
    }
    const waitFor = AUTH_WINDOW_MS - (now - registrationTimes[0]) + 250;
    await new Promise((resolve) => setTimeout(resolve, waitFor));
  }
}

/** Register a fresh user in this context and leave it holding their session. */
async function registerInContext(
  context: BrowserContext,
  baseURL: string,
  username: string,
): Promise<void> {
  await waitForRegistrationSlot();
  const page = await context.newPage();
  // A real page load, because registration sets a session cookie the
  // WebSocket upgrade later depends on, and cookies belong to an origin.
  await page.goto(baseURL);
  const ok = await page.evaluate(
    async ({ name }) => {
      const res = await fetch("/api/authentication/register", {
        method: "POST",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          username: name,
          email: `${name}@example.test`,
          password: "Sup3r-Secret-Passphrase!",
          password_confirmation: "Sup3r-Secret-Passphrase!",
        }),
      });
      return res.ok;
    },
    { name: username },
  );
  if (!ok) throw new Error(`could not register ${username}`);
}

/** Run one GraphQL operation as whoever this context is signed in as. */
async function gql<T>(
  context: BrowserContext,
  query: string,
  variables: Record<string, unknown>,
): Promise<T> {
  const page = context.pages()[0];
  return page.evaluate(
    async ({ query, variables }) => {
      const csrf = document.cookie
        .split(";")
        .map((part) => part.trim())
        .find((part) => part.startsWith("csrf_token="))
        ?.slice("csrf_token=".length);
      const res = await fetch("/api/graphql", {
        method: "POST",
        credentials: "same-origin",
        headers: {
          "Content-Type": "application/json",
          ...(csrf ? { "x-csrf-token": csrf } : {}),
        },
        body: JSON.stringify({ query, variables }),
      });
      const body = await res.json();
      if (body.errors) throw new Error(JSON.stringify(body.errors));
      return body.data;
    },
    { query, variables },
  ) as Promise<T>;
}

test(`${TABLES} tables of ${PLAYERS_PER_TABLE}, nobody hears another table`, async ({
  browser,
  baseURL,
}) => {
  // Generous by design: this looks for a breaking point, not a latency
  // budget. Registration alone is a real Argon2id hash per person.
  test.setTimeout(30 * 60_000);

  const people: Person[] = [];
  const worldByTable: string[] = [];
  const suffix = Date.now().toString(36);

  try {
    for (let table = 0; table < TABLES; table += 1) {
      // The Game Master registers, makes the world, and mints one invite the
      // whole table joins with.
      const gmContext = await browser.newContext();
      await registerInContext(gmContext, baseURL!, `tgm${table}x${suffix}`);
      people.push({ context: gmContext, table, isGm: true });

      const created = await gql<{ createWorld: { id: string } }>(
        gmContext,
        `mutation ($input: GraphQLCreateWorldInput!) {
          createWorld(input: $input) { id }
        }`,
        { input: { name: `Torture Table ${table} ${suffix}` } },
      );
      const worldId = created.createWorld.id;
      worldByTable.push(worldId);

      // Sized for the table, with room to spare. There is no unlimited
      // option — the server refuses `maxUses: 0` outright, which is the
      // right call for a shareable link and worth knowing: an invite that
      // never expires and never runs out is a standing door into a world.
      const invite = await gql<{
        generateInviteCode: { inviteCode: string };
      }>(
        gmContext,
        `mutation ($input: GenerateInviteCodeInput!) {
          generateInviteCode(input: $input) { inviteCode }
        }`,
        { input: { worldId, maxUses: PLAYERS_PER_TABLE * 2 } },
      );

      for (let seat = 1; seat < PLAYERS_PER_TABLE; seat += 1) {
        const playerContext = await browser.newContext();
        await registerInContext(
          playerContext,
          baseURL!,
          `tp${table}s${seat}x${suffix}`,
        );
        await gql(
          playerContext,
          `mutation ($input: JoinWorldInput!) { joinWorld(input: $input) { worldId } }`,
          { input: { inviteCode: invite.generateInviteCode.inviteCode } },
        );
        people.push({ context: playerContext, table, isGm: false });
      }
    }

    // Everybody subscribes to their own table, and every subscription is
    // acknowledged before a single message is published. Subscribing after
    // would make a missing event indistinguishable from one sent before
    // anyone was listening.
    await Promise.all(
      people.map(({ context, table }) =>
        context.pages()[0].evaluate(
          async ({ world }) => {
            const w = window as unknown as {
              __heard?: { worldId: string; eventCode: number }[];
            };
            w.__heard = [];
            const url = `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/api/ws`;
            const socket = new WebSocket(url, "graphql-transport-ws");
            await new Promise<void>((resolve, reject) => {
              const timer = setTimeout(
                () => reject(new Error("subscription never acknowledged")),
                60_000,
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
                          worldEventsCreated(worldId: $worldId) {
                            id
                            worldId
                            eventCode
                          }
                        }`,
                        variables: { worldId: world },
                      },
                    }),
                  );
                  clearTimeout(timer);
                  resolve();
                }
                if (message.type === "next" && message.id === "s") {
                  const event = message.payload?.data?.worldEventsCreated;
                  if (event?.worldId != null) {
                    w.__heard!.push({
                      worldId: String(event.worldId),
                      eventCode: Number(event.eventCode),
                    });
                  }
                }
              });
              socket.addEventListener("error", () => {
                clearTimeout(timer);
                reject(new Error("socket errored"));
              });
              socket.addEventListener("open", () => {
                socket.send(JSON.stringify({ type: "connection_init" }));
              });
            });
          },
          { world: worldByTable[table] },
        ),
      ),
    );

    // A beat, so every subscription is registered server-side.
    await new Promise((resolve) => setTimeout(resolve, 2_000));

    // Each Game Master speaks only to their own table, all at once.
    await Promise.all(
      people
        .filter((person) => person.isGm)
        .map(async (gm) => {
          for (let n = 0; n < MESSAGES_PER_TABLE; n += 1) {
            await gql(
              gm.context,
              `mutation ($input: SendChatMessageInput!) {
                sendChatMessage(input: $input) { id }
              }`,
              {
                input: {
                  worldId: worldByTable[gm.table],
                  body: `table ${gm.table} message ${n}`,
                },
              },
            );
          }
        }),
    );

    // Delivery is asynchronous, so wait for it rather than measuring its
    // latency by accident.
    //
    // A world carries more than chat: joining one emits a member-joined
    // event, minting an invite emits another, and every person at the table
    // legitimately hears those. So "more events than messages" is not
    // leakage — an earlier version of this test asserted exactly that and
    // failed on a single table, where leakage is not even possible. What
    // isolation actually means is that every event a person receives is
    // stamped with *their own* world.
    const CHAT = 17;
    const deadline = Date.now() + 180_000;
    let reports: { own: number; chat: number; foreign: number }[] = [];
    while (Date.now() < deadline) {
      reports = await Promise.all(
        people.map((person, index) =>
          person.context.pages()[0].evaluate(
            ({ mine, chat }) => {
              const heard =
                (
                  window as unknown as {
                    __heard: { worldId: string; eventCode: number }[];
                  }
                ).__heard ?? [];
              return {
                own: heard.filter((e) => e.worldId === mine).length,
                chat: heard.filter(
                  (e) => e.worldId === mine && e.eventCode === chat,
                ).length,
                foreign: heard.filter((e) => e.worldId !== mine).length,
              };
            },
            { mine: worldByTable[people[index].table], chat: CHAT },
          ),
        ),
      );
      if (reports.every((r) => r.chat >= MESSAGES_PER_TABLE)) break;
      await new Promise((resolve) => setTimeout(resolve, 500));
    }

    const shortfall = reports.filter((r) => r.chat < MESSAGES_PER_TABLE).length;
    const leaked = reports.reduce((sum, r) => sum + r.foreign, 0);

    // eslint-disable-next-line no-console -- the run's own summary, matching
    // the other torture specs so every tier reads the same way in a log.
    console.log(
      `[torture] tables=${TABLES} people=${people.length} ` +
        `expected=${MESSAGES_PER_TABLE} shortfall=${shortfall} leaked=${leaked}`,
    );

    expect(
      shortfall,
      "every person must hear every message said at their own table",
    ).toBe(0);

    // The one that matters most, and the reason this test uses real users
    // rather than sockets. An event stamped with another world reaching a
    // player is a privacy breach, not a performance problem — and it is the
    // exact failure the per-world router exists to make impossible.
    expect(leaked, "no event from another table may reach a player").toBe(0);
  } finally {
    await Promise.all(people.map((person) => person.context.close()));
  }
});
