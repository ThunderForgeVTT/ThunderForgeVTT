/**
 * Which window of this account is at the table (spec 036 US3a, ADR-073).
 *
 * # Why a service and not component state
 *
 * The claim outlives any one component: it is taken as the play field mounts
 * and released when that page goes away, and a companion surface elsewhere in
 * the app needs to be able to ask who holds it without the play field being
 * rendered. `peerTransfer.ts` is a service for the same reason and says so.
 *
 * # Subscribing is claiming
 *
 * There is no "claim" mutation. Opening the `playField` subscription takes the
 * table; closing it releases, with no timeout and nothing on the server to
 * reap — see `src/server/src/graphql/mutations_play_field.rs`. So the lifetime
 * of the subscription *is* the lifetime of the claim, and everything here is
 * about starting one, stopping it, and telling anybody who cares what the
 * server last said.
 */

import { subscribeToPlayField } from "../engine/world/sync/subscriptionClient";

/**
 * This page load's identity, generated once.
 *
 * The same notion the engine already uses for peer signalling
 * (`crypto.randomUUID()` per mount): opaque, never persisted, and meaningless
 * to anything but the registry holding it for as long as the socket is open.
 * A reload is deliberately a different client — it is a different window as
 * far as anybody watching is concerned.
 */
export const CLIENT_ID = crypto.randomUUID();

export type PlayFieldStatus =
  /** This client holds the table. */
  | "holding"
  /** Another window of this account holds it. */
  | "companion"
  /** Nobody holds it — nothing to take it from. */
  | "unclaimed"
  /** Not asked yet, or the subscription has not answered. */
  | "unknown";

export interface PlayFieldState {
  status: PlayFieldStatus;
  /** Which client holds it, when somebody does. */
  holderClientId: string | null;
  /** Set when the subscription itself failed — refused, or disconnected. */
  error: string | null;
}

const INITIAL: PlayFieldState = {
  status: "unknown",
  holderClientId: null,
  error: null,
};

let state: PlayFieldState = INITIAL;
let current: (() => void) | null = null;
const listeners = new Set<(next: PlayFieldState) => void>();

function disposeCurrent(): void {
  current?.();
  current = null;
}

function publish(next: PlayFieldState): void {
  state = next;
  for (const listener of listeners) {
    listener(state);
  }
}

export function getPlayFieldState(): PlayFieldState {
  return state;
}

/** Watch the claim. Returns an unsubscribe. */
export function subscribeToPlayFieldState(
  listener: (next: PlayFieldState) => void,
): () => void {
  listeners.add(listener);
  listener(state);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Take the play field for this world, and hold it until the returned function
 * is called.
 *
 * Always succeeds for a member: the newest claim wins, and a window that is
 * displaced learns so on this same stream rather than by polling. Refusing
 * the new window instead would reproduce the defect ADR-073 exists to remove
 * one level up — "you are already playing somewhere else" is the same dead
 * end as "you have been signed out", and a person cannot always reach the
 * other window to release it.
 */
export function claimPlayField(worldId: string): () => void {
  // At most one live claim per page load. "Take the table back" calls this
  // again while the first subscription is still open, and two subscriptions
  // for one client id would leave a socket open that nothing will ever close
  // — the registry is safe either way, because a displaced guard releases
  // nothing, but the leak is ours.
  disposeCurrent();

  const dispose = subscribeToPlayField(worldId, CLIENT_ID, {
    next: (claim) => {
      if (!claim) {
        publish({ status: "unclaimed", holderClientId: null, error: null });
        return;
      }
      publish({
        status: claim.isMine ? "holding" : "companion",
        holderClientId: claim.clientId,
        error: null,
      });
    },
    error: (message) => {
      // A refusal — not a member, not signed in — and a dropped socket arrive
      // the same way. Neither means this client holds the table, and saying
      // "holding" through a broken stream is the one answer that would be
      // actively harmful.
      publish({ status: "unknown", holderClientId: null, error: message });
    },
  });

  current = dispose;
  return () => {
    // Only the caller who started *this* claim may end it. A stale disposer
    // from a claim that has already been replaced must not close the live
    // subscription — the same identity guard the registry applies on the
    // server, for the same reason.
    if (current === dispose) {
      disposeCurrent();
      publish(INITIAL);
    }
  };
}

/** Reset — for tests, which must not inherit another test's claim. */
export function resetPlayFieldStateForTests(): void {
  disposeCurrent();
  state = INITIAL;
  listeners.clear();
}
