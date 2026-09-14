import { expect, test, type BrowserContext, type Page } from "./fixtures/test";
import { openAdminPage } from "./fixtures/admin";
import {
  graphql,
  registerAndCreateWorld,
  uniqueSuffix,
} from "./fixtures/helpers";
import { pauseWorldAsOperator } from "./fixtures/playPause";

/**
 * Spec 051 research R1, the enforcing half: the server's own five-second
 * stream check ends a paused world's live streams, whatever the client does.
 *
 * # Why there is no page here
 *
 * `play-pause.spec.ts` withholds world event 28 from a real page and watches
 * it leave. The page left in tens of milliseconds, which is faster than any
 * `LIVENESS_POLL` tick can be: something other than the tick got there first
 * (see the note on that test). So it proves the page leaves; it cannot prove
 * the tick.
 *
 * This spec is the client an attacker would write. It speaks
 * `graphql-transport-ws` to `/api/ws` from the test process, carrying the
 * member's real session cookie on the upgrade (which is how the server
 * authenticates the socket), and it has no logic at all: it does not leave,
 * does not resubscribe, does not make HTTP requests, and does not act on
 * event 28. Anything that ends its streams is the server.
 *
 * # What each stream proves
 *
 * - `playField` and `peerSignals` never carry world event 28. The only way
 *   they can learn of the pause is the tick in `until_stream_must_end`.
 * - `worldEventsCreated` does carry event 28, and a client ignoring it is
 *   still cut off: the same tick yields the error and completes the stream.
 * - A stream on another world, on the same socket, keeps running past two
 *   ticks and still delivers.
 *
 * # How the tick is told apart from any other road
 *
 * `until_stream_must_end` starts its `interval` when the stream is first
 * polled, and discards the immediate first tick, so its first check is
 * `LIVENESS_POLL` after the stream opened and every later one is a multiple
 * of it. An error delivered by the tick therefore arrives at least five
 * seconds after the subscribe was sent, whatever moment the pause landed in.
 * Each run shape pauses at a different point in that window, so the delay
 * after the pause changes while the delay after opening stays pinned to the
 * tick.
 */

/** `LIVENESS_POLL` in `src/server/src/graphql/session_lifetime.rs`. */
const LIVENESS_POLL_MS = 5_000;

/**
 * The most a paused stream may run after the pause, before the error arrives.
 *
 * Worst case is a pause committed just after a tick: the next check is one
 * full `LIVENESS_POLL` later, then the tick's own query (a pool checkout and
 * a two-row lookup on `spawn_blocking`, tens of milliseconds), then the frame
 * crossing the Vite proxy to this process. 1.5 s over the poll is room for a
 * loaded shard without being loose enough to hide a missed tick, which would
 * land near ten seconds.
 */
const TICK_BUDGET_MS = LIVENESS_POLL_MS + 1_500;

/**
 * How early an error may appear relative to the tick schedule and still be
 * called the tick's. The interval starts server-side after the subscribe
 * frame is read and the resolver has run, so a genuine tick can only be
 * *later* than `subscribedAt + LIVENESS_POLL`; the slack is for timer
 * granularity only.
 */
const TICK_EARLY_SLACK_MS = 100;

/** A pause answered faster than this after being sent was not a tick. */
const NOT_INSTANT_MS = 200;

const WORLD_PLAY_PAUSED = "WORLD_PLAY_PAUSED";

type GqlError = { message: string; extensions?: Record<string, unknown> };

interface StreamItem {
  at: number;
  data: Record<string, unknown> | null;
  errors: GqlError[] | null;
}

interface StreamRecord {
  id: string;
  field: string;
  subscribedAt: number;
  items: StreamItem[];
  completedAt: number | null;
  /** A protocol-level `error` message, which is not what the tick sends. */
  protocolError: { at: number; payload: unknown } | null;
}

/**
 * A `graphql-transport-ws` client with nothing in it but the protocol.
 *
 * Hand-rolled rather than `graphql-ws`'s `createClient`, because that client
 * retries, resubscribes and hides frames — each of which is a road this spec
 * exists to rule out. Every frame is recorded with the moment it arrived.
 */
class RawSocket {
  private readonly streams = new Map<string, StreamRecord>();
  private nextId = 1;
  closedAt: number | null = null;

  private constructor(private readonly ws: WebSocket) {}

  static async open(url: string, cookie: string): Promise<RawSocket> {
    // Node's WebSocket (undici) accepts headers on the upgrade, which is
    // where the session cookie has to be: the auth middleware reads it from
    // the HTTP request, not from `connection_init`.
    const ws = new WebSocket(url, {
      protocols: ["graphql-transport-ws"],
      headers: { cookie },
    } as unknown as string[]);
    const socket = new RawSocket(ws);
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("no connection_ack within 15 s")),
        15_000,
      );
      ws.addEventListener("open", () => {
        ws.send(JSON.stringify({ type: "connection_init", payload: {} }));
      });
      ws.addEventListener("error", (event) => {
        clearTimeout(timer);
        reject(
          new Error(`socket error: ${String((event as ErrorEvent).message)}`),
        );
      });
      ws.addEventListener("close", (event) => {
        socket.closedAt = Date.now();
        clearTimeout(timer);
        reject(
          new Error(`socket closed before ack: ${event.code} ${event.reason}`),
        );
      });
      ws.addEventListener("message", (event) => {
        const at = Date.now();
        const message = JSON.parse(String(event.data)) as {
          type: string;
          id?: string;
          payload?: unknown;
        };
        socket.receive(at, message);
        if (message.type === "connection_ack") {
          clearTimeout(timer);
          resolve();
        }
      });
    });
    return socket;
  }

  private receive(
    at: number,
    message: { type: string; id?: string; payload?: unknown },
  ): void {
    if (message.type === "ping") {
      this.ws.send(JSON.stringify({ type: "pong" }));
      return;
    }
    const stream = message.id ? this.streams.get(message.id) : undefined;
    if (!stream) return;
    switch (message.type) {
      case "next": {
        const payload = message.payload as {
          data?: Record<string, unknown> | null;
          errors?: GqlError[];
        };
        stream.items.push({
          at,
          data: payload.data ?? null,
          errors: payload.errors?.length ? payload.errors : null,
        });
        break;
      }
      case "error":
        stream.protocolError = { at, payload: message.payload };
        stream.completedAt ??= at;
        break;
      case "complete":
        stream.completedAt ??= at;
        break;
    }
  }

  subscribe(
    field: string,
    query: string,
    variables: Record<string, unknown>,
  ): StreamRecord {
    const id = String(this.nextId++);
    const record: StreamRecord = {
      id,
      field,
      subscribedAt: Date.now(),
      items: [],
      completedAt: null,
      protocolError: null,
    };
    this.streams.set(id, record);
    this.ws.send(
      JSON.stringify({ id, type: "subscribe", payload: { query, variables } }),
    );
    return record;
  }

  get isOpen(): boolean {
    return this.ws.readyState === WebSocket.OPEN;
  }

  close(): void {
    this.ws.close(1000);
  }
}

const PLAY_FIELD = `
  subscription PlayField($worldId: UUID!, $clientId: String!) {
    playField(worldId: $worldId, clientId: $clientId) { clientId worldId isMine }
  }
`;
const PEER_SIGNALS = `
  subscription PeerSignals($worldId: UUID!, $sessionId: String!) {
    peerSignals(worldId: $worldId, sessionId: $sessionId) { fromSessionId payload }
  }
`;
const WORLD_EVENTS = `
  subscription WorldEvents($worldId: String!) {
    worldEventsCreated(worldId: $worldId) { id worldId eventCode }
  }
`;
const PLAYERS_ONLINE = `
  subscription PlayersOnline($worldId: String!) {
    playersOnline(worldId: $worldId) { worldId }
  }
`;

function subscribeField(
  socket: RawSocket,
  field: "playField" | "peerSignals" | "worldEventsCreated" | "playersOnline",
  worldId: string,
  clientId: string,
): StreamRecord {
  switch (field) {
    case "playField":
      return socket.subscribe(field, PLAY_FIELD, { worldId, clientId });
    case "peerSignals":
      return socket.subscribe(field, PEER_SIGNALS, {
        worldId,
        sessionId: clientId,
      });
    case "worldEventsCreated":
      return socket.subscribe(field, WORLD_EVENTS, { worldId });
    case "playersOnline":
      return socket.subscribe(field, PLAYERS_ONLINE, { worldId });
  }
}

function errorItems(stream: StreamRecord): StreamItem[] {
  return stream.items.filter((item) => item.errors);
}

function describeStream(stream: StreamRecord): string {
  return JSON.stringify({
    field: stream.field,
    subscribedAt: stream.subscribedAt,
    completedAt: stream.completedAt,
    protocolError: stream.protocolError,
    items: stream.items,
  });
}

async function until(
  condition: () => boolean,
  timeoutMs: number,
  what: string,
): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  while (!condition()) {
    if (Date.now() > deadline) {
      throw new Error(`timed out after ${timeoutMs} ms waiting for ${what}`);
    }
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

async function sleepUntil(moment: number): Promise<void> {
  const ms = moment - Date.now();
  if (ms > 0) await new Promise((resolve) => setTimeout(resolve, ms));
}

async function createWorld(page: Page, name: string): Promise<string> {
  const answer = await graphql<{
    data?: { createWorld?: { id: string } };
    errors?: unknown;
  }>(
    page,
    `
      mutation CW($input: GraphQLCreateWorldInput!) {
        createWorld(input: $input) {
          id
        }
      }
    `,
    { input: { name } },
  );
  const id = answer.data?.createWorld?.id;
  if (!id) throw new Error(`createWorld: ${JSON.stringify(answer.errors)}`);
  return id;
}

/** Record a world event on `worldId`, by launching a fresh scene in it. */
async function recordAWorldEvent(page: Page, worldId: string): Promise<void> {
  const created = await graphql<{
    data?: { createScene?: { sceneId: string } };
    errors?: unknown;
  }>(
    page,
    `
      mutation CS($input: GraphQLCreateSceneInput!) {
        createScene(input: $input) {
          sceneId
        }
      }
    `,
    { input: { worldId, name: `Control scene ${uniqueSuffix()}` } },
  );
  const sceneId = created.data?.createScene?.sceneId;
  if (!sceneId)
    throw new Error(`createScene: ${JSON.stringify(created.errors)}`);
  const launched = await graphql<{
    data?: { launchScene?: { id: string } };
    errors?: unknown;
  }>(
    page,
    `
      mutation LS($worldId: UUID!, $sceneId: UUID!) {
        launchScene(worldId: $worldId, sceneId: $sceneId) {
          id
        }
      }
    `,
    { worldId, sceneId },
  );
  if (!launched.data?.launchScene) {
    throw new Error(`launchScene: ${JSON.stringify(launched.errors)}`);
  }
}

async function cookieHeader(
  context: BrowserContext,
  baseURL: string,
): Promise<string> {
  const cookies = await context.cookies(baseURL);
  const header = cookies.map((c) => `${c.name}=${c.value}`).join("; ");
  if (cookies.length === 0) {
    throw new Error("the member's context holds no cookies");
  }
  return header;
}

/**
 * The shapes the pause is made in: how long after the paused world's streams
 * opened. One well inside the first tick window, one close to its end. The
 * delay after the pause should differ by about three seconds between them;
 * the delay after opening should sit on the tick in both.
 */
const SHAPES = [
  { name: "paused 1 s after the streams opened", pauseAfterOpenMs: 1_000 },
  { name: "paused 3.5 s after the streams opened", pauseAfterOpenMs: 3_500 },
] as const;

test.describe("spec 051 R1: the five-second stream check ends a paused world's streams on its own", () => {
  for (const shape of SHAPES) {
    test(`a client that ignores the pause is cut off by the tick (${shape.name})`, async ({
      browser,
      baseURL,
    }) => {
      test.setTimeout(300_000);
      if (!baseURL) throw new Error("no baseURL configured");

      const suffix = uniqueSuffix();
      const memberContext = await browser.newContext();
      const memberPage = await memberContext.newPage();
      const pausedWorldName = `E2E Stream Poll ${suffix}`;
      const pausedWorldId = await registerAndCreateWorld(
        memberPage,
        pausedWorldName,
        "e2epoll",
      );
      const controlWorldId = await createWorld(
        memberPage,
        `E2E Stream Poll Control ${suffix}`,
      );
      // No page logic anywhere near the streams: the tab is left blank, and
      // the GraphQL helper talks through the context's cookie jar.
      await memberPage.goto("about:blank");

      const adminPage = await openAdminPage(browser);
      const wsUrl = `${baseURL.replace(/^http/, "ws")}/api/ws`;
      const socket = await RawSocket.open(
        wsUrl,
        await cookieHeader(memberContext, baseURL),
      );

      try {
        const clientId = `e2e-poll-${suffix}`;

        // The control world first, so it has been open longest and crosses
        // the most ticks by the end.
        const controlEvents = subscribeField(
          socket,
          "worldEventsCreated",
          controlWorldId,
          clientId,
        );
        const controlPresence = subscribeField(
          socket,
          "playersOnline",
          controlWorldId,
          clientId,
        );

        // The paused world. `playField` answers with the claim straight away,
        // which is how this knows the stream opened rather than being refused.
        const playField = subscribeField(
          socket,
          "playField",
          pausedWorldId,
          clientId,
        );
        await until(
          () => playField.items.length > 0 || playField.completedAt !== null,
          15_000,
          "playField's first claim",
        );
        expect(
          playField.items[0]?.data?.playField,
          `playField must open and claim: ${describeStream(playField)}`,
        ).toMatchObject({ worldId: pausedWorldId, isMine: true });

        // `peerSignals` admits only the play field's holder, which this socket
        // now is.
        const peerSignals = subscribeField(
          socket,
          "peerSignals",
          pausedWorldId,
          clientId,
        );
        const worldEvents = subscribeField(
          socket,
          "worldEventsCreated",
          pausedWorldId,
          clientId,
        );
        const pausedStreams = [playField, peerSignals, worldEvents];
        const lastOpenedAt = Math.max(
          ...pausedStreams.map((s) => s.subscribedAt),
        );

        // Give refusals a moment to show: an open stream says nothing, a
        // refused one says so and completes.
        await new Promise((resolve) => setTimeout(resolve, 300));
        for (const stream of [
          ...pausedStreams,
          controlEvents,
          controlPresence,
        ]) {
          expect(
            stream.completedAt,
            `${stream.field} must be open before the pause: ${describeStream(stream)}`,
          ).toBeNull();
          expect(errorItems(stream)).toEqual([]);
        }

        // ---- The pause, at a chosen point in the tick window ----
        await sleepUntil(lastOpenedAt + shape.pauseAfterOpenMs);
        const pauseSentAt = Date.now();
        await pauseWorldAsOperator(
          adminPage,
          pausedWorldId,
          `Operator grounds ${suffix}: stream poll`,
        );
        const pauseConfirmedAt = Date.now();
        const itemsBeforePause = new Map(
          pausedStreams.map((s) => [s, s.items.length]),
        );

        await until(
          () => pausedStreams.every((s) => s.completedAt !== null),
          TICK_BUDGET_MS + 5_000,
          `every paused stream to complete: ${pausedStreams.map(describeStream).join("\n")}`,
        );

        for (const stream of pausedStreams) {
          const errors = errorItems(stream);
          const errorAt = errors[0]?.at ?? Number.NaN;
          const code = errors[0]?.errors?.[0]?.extensions?.code;
          const afterPause = errorAt - pauseSentAt;
          const afterConfirm = errorAt - pauseConfirmedAt;
          const intoStream = errorAt - stream.subscribedAt;
          const completedAfter =
            (stream.completedAt ?? Number.NaN) - pauseSentAt;
          console.log(
            `[stream-poll] ${stream.field}: error after ${afterPause} ms, ` +
              `completed after ${completedAfter} ms ` +
              `(${shape.name}; ${afterConfirm} ms after confirmation, ` +
              `${intoStream} ms after the stream opened, code ${String(code)})`,
          );

          const detail = describeStream(stream);
          expect(stream.protocolError, detail).toBeNull();
          expect(errors, `exactly one error item: ${detail}`).toHaveLength(1);
          expect(errors[0].errors, detail).toHaveLength(1);
          expect(code, detail).toBe(WORLD_PLAY_PAUSED);
          expect(errors[0].errors?.[0]?.extensions?.worldId, detail).toBe(
            pausedWorldId,
          );
          // The error is the stream's last word: nothing after it but the end.
          expect(
            stream.items.at(-1),
            `no data after the error: ${detail}`,
          ).toBe(errors[0]);
          expect(stream.completedAt ?? 0, detail).toBeGreaterThanOrEqual(
            errorAt,
          );

          // Within one tick of the pause.
          expect(afterPause, detail).toBeLessThanOrEqual(TICK_BUDGET_MS);
          // And by the tick: never before the first check could have run, and
          // never instantly after the pause.
          expect(
            intoStream,
            `the error came before the stream's first tick could run: ${detail}`,
          ).toBeGreaterThanOrEqual(LIVENESS_POLL_MS - TICK_EARLY_SLACK_MS);
          expect(
            afterPause,
            `the error was instant, so something other than the tick ended the stream: ${detail}`,
          ).toBeGreaterThan(NOT_INSTANT_MS);
          // Sitting on the tick schedule, not merely late: its offset from the
          // nearest multiple of LIVENESS_POLL is the query and the hop.
          const offTick = intoStream % LIVENESS_POLL_MS;
          expect(
            offTick,
            `the error is ${offTick} ms off the tick schedule: ${detail}`,
          ).toBeLessThan(1_000);
        }

        // The two streams that never carry event 28 received nothing between
        // the pause and the error: the tick is the only way they heard.
        for (const stream of [playField, peerSignals]) {
          const before = itemsBeforePause.get(stream) ?? 0;
          expect(
            stream.items.slice(before).filter((item) => !item.errors),
            `${stream.field} must carry no data after the pause: ${describeStream(stream)}`,
          ).toEqual([]);
        }

        // `worldEventsCreated` received event 28 first, and was cut off anyway.
        const pauseEvent = worldEvents.items.find(
          (item) =>
            (
              item.data?.worldEventsCreated as
                | { eventCode?: number }
                | undefined
            )?.eventCode === 28,
        );
        expect(
          pauseEvent,
          `worldEventsCreated must carry event 28: ${describeStream(worldEvents)}`,
        ).toBeTruthy();
        const worldEventsError = errorItems(worldEvents)[0];
        expect(pauseEvent?.at ?? Infinity).toBeLessThan(worldEventsError.at);
        console.log(
          `[stream-poll] worldEventsCreated: event 28 after ${
            (pauseEvent?.at ?? Number.NaN) - pauseSentAt
          } ms, ignored; the tick's error followed ${
            worldEventsError.at - (pauseEvent?.at ?? Number.NaN)
          } ms later`,
        );

        // ---- Re-opening on the same socket is refused at open ----
        expect(socket.isOpen, "the socket itself stays open").toBe(true);
        const reopened = (
          [
            "playField",
            "peerSignals",
            "worldEventsCreated",
            "playersOnline",
          ] as const
        ).map((field) =>
          subscribeField(socket, field, pausedWorldId, `${clientId}-again`),
        );
        await until(
          () => reopened.every((s) => s.completedAt !== null),
          10_000,
          `every re-opened subscription to be refused: ${reopened.map(describeStream).join("\n")}`,
        );
        for (const stream of reopened) {
          const detail = describeStream(stream);
          const refusedMs =
            (stream.completedAt ?? Number.NaN) - stream.subscribedAt;
          console.log(
            `[stream-poll] re-open ${stream.field}: refused in ${refusedMs} ms`,
          );
          expect(stream.protocolError, detail).toBeNull();
          expect(stream.items, detail).toHaveLength(1);
          expect(
            stream.items[0].data?.[stream.field] ?? null,
            detail,
          ).toBeNull();
          expect(stream.items[0].errors?.[0]?.extensions?.code, detail).toBe(
            WORLD_PLAY_PAUSED,
          );
          // Refused as it opens, not on a tick.
          expect(refusedMs, detail).toBeLessThan(LIVENESS_POLL_MS - 1_000);
        }

        // ---- The other world, on the same socket, plays on ----
        await sleepUntil(pauseConfirmedAt + 2 * LIVENESS_POLL_MS + 1_000);
        const controlRanMs = Date.now() - controlEvents.subscribedAt;
        for (const stream of [controlEvents, controlPresence]) {
          const detail = describeStream(stream);
          expect(
            stream.completedAt,
            `the control must still run: ${detail}`,
          ).toBeNull();
          expect(errorItems(stream), detail).toEqual([]);
          expect(stream.protocolError, detail).toBeNull();
        }
        // Not merely unended but delivering: an event recorded now arrives.
        const controlBefore = controlEvents.items.length;
        const eventSentAt = Date.now();
        await recordAWorldEvent(memberPage, controlWorldId);
        await until(
          () => controlEvents.items.length > controlBefore,
          10_000,
          `the control world's event: ${describeStream(controlEvents)}`,
        );
        const delivered = controlEvents.items[controlBefore];
        expect(delivered.errors).toBeNull();
        console.log(
          `[stream-poll] control worldEventsCreated on another world: running ` +
            `${controlRanMs} ms (${Math.floor(controlRanMs / LIVENESS_POLL_MS)} ticks), ` +
            `still delivered event ${String(
              (delivered.data?.worldEventsCreated as { eventCode?: number })
                ?.eventCode,
            )} ${delivered.at - eventSentAt} ms after it was made`,
        );
        expect(controlEvents.completedAt).toBeNull();
        expect(controlPresence.completedAt).toBeNull();
      } finally {
        socket.close();
        await adminPage.context().close();
        await memberContext.close();
      }
    });
  }
});
