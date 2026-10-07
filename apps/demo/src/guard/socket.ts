/**
 * The only socket the demo has: one that ends in the page.
 *
 * The client opens `graphql-ws` connections to `/api/ws` for world events and
 * for its claim on the play field. This stands where `WebSocket` stood and
 * speaks that protocol itself, so the subscription client, the event sync and
 * the engine's own subscriber run exactly as they do against a server.
 */
import { now, subscribeToEvents } from "../backend/events";
import { openSubscription, type OperationRequest } from "../backend/execute";
import {
  NOT_IN_DEMO_CODE,
  notInDemoMessage,
  reportNotInDemo,
} from "../backend/notInDemo";

type Listener = ((event: never) => void) | null;

/**
 * Subscriptions that are true to leave silent: with one person in the world,
 * nobody else arrives, signals, or edits a sheet.
 */
const QUIET = new Set([
  "peerSignals",
  "playersOnline",
  "worldActorSystemDataUpdated",
]);

export function installSocketGuard(base: string): void {
  const RealWebSocket = window.WebSocket;

  class DemoSocket extends EventTarget {
    static readonly CONNECTING = 0;
    static readonly OPEN = 1;
    static readonly CLOSING = 2;
    static readonly CLOSED = 3;
    readonly CONNECTING = 0;
    readonly OPEN = 1;
    readonly CLOSING = 2;
    readonly CLOSED = 3;

    readonly url: string;
    readonly protocol = "graphql-transport-ws";
    readonly extensions = "";
    readonly bufferedAmount = 0;
    binaryType: BinaryType = "blob";
    readyState = 0;
    onopen: Listener = null;
    onmessage: Listener = null;
    onclose: Listener = null;
    onerror: Listener = null;

    private readonly subscriptions = new Map<string, () => void>();

    constructor(url: string | URL, protocols?: string | string[]) {
      super();
      const target = new URL(String(url), window.location.href);
      this.url = target.href;

      // The dev server's own module-reload socket is the page's tooling, not
      // the app's; a built demo has none.
      if (import.meta.env.DEV && target.pathname.startsWith(base)) {
        return new RealWebSocket(url, protocols) as unknown as DemoSocket;
      }

      if (
        target.host !== window.location.host ||
        target.pathname !== "/api/ws"
      ) {
        reportNotInDemo(`socket ${target.pathname}`);
        setTimeout(() => this.finish(1008, "not part of the demo"), 0);
        return;
      }
      setTimeout(() => {
        if (this.readyState !== 0) return;
        this.readyState = 1;
        this.fire("open", new Event("open"));
      }, 0);
    }

    send(data: string): void {
      if (this.readyState !== 1) return;
      const message = JSON.parse(data) as {
        type: string;
        id?: string;
        payload?: OperationRequest;
      };
      switch (message.type) {
        case "connection_init":
          this.deliver({ type: "connection_ack" });
          break;
        case "ping":
          this.deliver({ type: "pong" });
          break;
        case "subscribe":
          if (message.id && message.payload) {
            this.subscribe(message.id, message.payload);
          }
          break;
        case "complete":
          if (message.id) this.unsubscribe(message.id);
          break;
      }
    }

    close(code = 1000, reason = ""): void {
      if (this.readyState >= 2) return;
      this.readyState = 2;
      setTimeout(() => this.finish(code, reason), 0);
    }

    private subscribe(id: string, request: OperationRequest): void {
      const opened = openSubscription(request);
      if ("errors" in opened) {
        this.deliver({ id, type: "error", payload: opened.errors });
        return;
      }
      const next = (value: unknown) =>
        opened.shape(value).then((payload) => {
          if (this.subscriptions.has(id)) {
            this.deliver({ id, type: "next", payload });
          }
        });

      if (opened.field === "worldEventsCreated") {
        // One at a time, so two events shaped at once cannot swap places.
        let last: Promise<unknown> = Promise.resolve();
        this.subscriptions.set(
          id,
          subscribeToEvents((event) => {
            last = last.then(() => next(event));
          }),
        );
        return;
      }
      if (opened.field === "playField") {
        // Subscribing is claiming, and there is nobody to lose the claim to.
        this.subscriptions.set(id, () => {});
        void next({
          clientId: opened.args.clientId,
          worldId: opened.args.worldId,
          claimedAt: now(),
          isMine: true,
        });
        return;
      }
      if (QUIET.has(opened.field)) {
        this.subscriptions.set(id, () => {});
        return;
      }
      reportNotInDemo(opened.field);
      this.deliver({
        id,
        type: "error",
        payload: [
          {
            message: notInDemoMessage(opened.field),
            extensions: { code: NOT_IN_DEMO_CODE },
          },
        ],
      });
    }

    private unsubscribe(id: string): void {
      this.subscriptions.get(id)?.();
      this.subscriptions.delete(id);
    }

    private deliver(message: unknown): void {
      // Never inside the caller's own `send`: a server's answer is a later
      // turn of the loop, and the client is written for that.
      setTimeout(() => {
        if (this.readyState !== 1) return;
        this.fire(
          "message",
          new MessageEvent("message", { data: JSON.stringify(message) }),
        );
      }, 0);
    }

    private finish(code: number, reason: string): void {
      if (this.readyState === 3) return;
      for (const stop of this.subscriptions.values()) stop();
      this.subscriptions.clear();
      this.readyState = 3;
      this.fire(
        "close",
        new CloseEvent("close", { code, reason, wasClean: code === 1000 }),
      );
    }

    private fire(type: "open" | "message" | "close" | "error", event: Event) {
      const handler = this[`on${type}`] as ((event: Event) => void) | null;
      handler?.call(this, event);
      this.dispatchEvent(event);
    }
  }

  window.WebSocket = DemoSocket as unknown as typeof WebSocket;
}
