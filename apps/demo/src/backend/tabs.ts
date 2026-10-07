/**
 * Spec 081 R6: one world across a visitor's tabs (contracts/demo-tabs.md).
 *
 * The tab granted the lock holds the world: it runs every operation, its own
 * and the other tabs', and posts each event it releases. Every other tab is a
 * guest that asks the holder over the channel. When the holder closes, the
 * lock passes to a waiting guest, which reads the world as the holder last
 * saved it and says so; the guests resend whatever was still unanswered.
 *
 * Without locks or a channel, every tab holds its own world, as before.
 */
import type { ExecutionResult } from "graphql";

import type { Viewer } from "../seed/world";
import type { WorldEvent } from "./events";
import type { OperationRequest } from "./execute";

export const LOCK_NAME = "thunderforge-demo-world";
export const CHANNEL_NAME = "thunderforge-demo";

export type TabMessage =
  | {
      kind: "request";
      id: string;
      viewer: Viewer;
      query: string;
      variables?: Record<string, unknown>;
    }
  | { kind: "answer"; id: string; result: ExecutionResult }
  | { kind: "event"; event: WorldEvent }
  | { kind: "holder" };

/** The part of `navigator.locks` used here. */
export interface Locks {
  request(
    name: string,
    options: { ifAvailable?: boolean },
    callback: (lock: unknown) => Promise<unknown> | unknown,
  ): Promise<unknown>;
}

/** The part of `BroadcastChannel` used here. */
export interface Channel {
  postMessage(message: TabMessage): void;
  addEventListener(
    type: "message",
    listener: (message: { data: TabMessage }) => void,
  ): void;
}

export interface TabsOptions {
  locks: Locks | undefined;
  channel: Channel | undefined;
  /** Who this tab renders for. */
  viewer: () => Viewer;
  /** Runs one operation on the world as `viewer`; only ever on the holder. */
  run: (
    viewer: Viewer,
    operation: OperationRequest,
  ) => Promise<ExecutionResult>;
  /** Reads the world as the last holder saved it, on taking over. */
  takeOver: () => void;
  /** Hands this tab's subscribers an event the holder released. */
  deliver: (event: WorldEvent) => void;
  /** Calls `post` with every event this tab releases while it holds. */
  onReleased: (post: (event: WorldEvent) => void) => void;
}

export interface Tabs {
  /** Resolves once the tab knows whether it holds the world. */
  ready: Promise<void>;
  holds: () => boolean;
  /** Runs an operation, here if this tab holds the world, else on the holder. */
  ask: (operation: OperationRequest) => Promise<ExecutionResult>;
}

/** Never settles: the lock is held until the tab closes. */
const forever = () => new Promise<never>(() => {});

export function connectTabs(options: TabsOptions): Tabs {
  const { locks, channel, viewer, run } = options;
  if (!locks || !channel) {
    return {
      ready: Promise.resolve(),
      holds: () => true,
      ask: (operation) => run(viewer(), operation),
    };
  }

  let holding = false;
  const open = new Map<
    string,
    {
      message: TabMessage;
      resolve: (result: ExecutionResult) => void;
    }
  >();

  const hold = () => {
    holding = true;
    options.onReleased((event) =>
      channel.postMessage({ kind: "event", event }),
    );
  };

  channel.addEventListener("message", ({ data }) => {
    switch (data.kind) {
      case "request":
        if (!holding) return;
        void run(data.viewer, {
          query: data.query,
          variables: data.variables,
        }).then((result) =>
          channel.postMessage({
            kind: "answer",
            id: data.id,
            // As the response carries it: a structured clone of a
            // `GraphQLError` keeps no `toJSON`, and its message would read
            // as `{}` on the guest.
            result: JSON.parse(JSON.stringify(result)) as ExecutionResult,
          }),
        );
        return;
      case "answer": {
        const waiting = open.get(data.id);
        if (!waiting) return;
        open.delete(data.id);
        waiting.resolve(data.result);
        return;
      }
      case "event":
        if (!holding) options.deliver(data.event);
        return;
      case "holder":
        // A request the last holder took and never answered went with it.
        for (const { message } of open.values()) channel.postMessage(message);
        return;
    }
  });

  const ready = new Promise<void>((resolve) => {
    void locks.request(LOCK_NAME, { ifAvailable: true }, (lock) => {
      if (lock) {
        hold();
        resolve();
        return forever();
      }
      resolve();
      // A guest waits its turn, and takes over when the holder goes.
      void locks.request(LOCK_NAME, {}, () => {
        options.takeOver();
        hold();
        channel.postMessage({ kind: "holder" });
        // What this tab asked while it waited is its own to answer now.
        for (const [id, { message, resolve: answer }] of open) {
          if (message.kind !== "request") continue;
          open.delete(id);
          void run(message.viewer, {
            query: message.query,
            variables: message.variables,
          }).then(answer);
        }
        return forever();
      });
      return undefined;
    });
  });

  return {
    ready,
    holds: () => holding,
    ask: async (operation) => {
      await ready;
      if (holding) return run(viewer(), operation);
      const message: TabMessage = {
        kind: "request",
        id: crypto.randomUUID(),
        viewer: viewer(),
        query: operation.query,
        variables: operation.variables as Record<string, unknown> | undefined,
      };
      return new Promise<ExecutionResult>((resolve) => {
        open.set(message.id, { message, resolve });
        channel.postMessage(message);
      });
    },
  };
}

/**
 * One operation at a time, each as its own viewer: a handler that awaits
 * (the dice do) must not find another tab's viewer when it resumes.
 */
export function oneAtATime<T>(
  task: (viewer: Viewer, operation: OperationRequest) => Promise<T>,
): (viewer: Viewer, operation: OperationRequest) => Promise<T> {
  let last: Promise<unknown> = Promise.resolve();
  return (viewer, operation) => {
    const next = last.then(() => task(viewer, operation));
    last = next.catch(() => undefined);
    return next;
  };
}
