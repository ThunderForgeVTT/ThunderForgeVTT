/**
 * Spec 036 FR-009, the half that was missing: **the client is told.**
 *
 * > An ended session MUST be refused on its next request, and its client MUST
 * > be told to sign in again rather than left showing state it can no longer
 * > refresh.
 *
 * The first half was true — `resolve_authenticated_user` refuses a revoked or
 * expired session on its very next request. The second was not. Nothing in the
 * app reacted to a 401: session state was read once when the app mounted and
 * never revalidated, so a client whose session had been ended elsewhere kept
 * rendering everything it had, until some component happened to make a call
 * *and* handle the failure — which most do by showing an error string beside
 * the thing that failed.
 *
 * That matters more now than it did, because ending a session from another
 * device is a thing a person can actually do: the session list on
 * `/settings/security` exists to be used, and "I ended that laptop's session"
 * should not leave the laptop looking signed in.
 *
 * # Why the transport and not the router
 *
 * Because routing is not where interaction happens. A play field sits on one
 * route for hours and makes hundreds of requests; a route-change hook would
 * miss every one of them, fire pointlessly on navigations that call nothing,
 * and — worst — would have to run *before* rendering, which is exactly the
 * shape that breaks the offline play field specs 028 and 036 US3c protect.
 *
 * Every request in this app goes through `graphqlClient`. Reacting there sees
 * everything, needs no per-route wiring, costs no extra requests, and cannot
 * break offline: it only ever reacts to an answer that actually arrived.
 *
 * # Why a 401 and not an error message
 *
 * The status is the server's own verdict. A message can be reworded; a 401
 * from `require_authenticated_user` means precisely "this session is not
 * accepted", which is the condition FR-009 is about.
 */

/** Notified when the server refuses this client's session. */
type Listener = () => void;

const listeners = new Set<Listener>();

/**
 * Whether we have already reacted to a refusal.
 *
 * A page that has lost its session usually loses it for several in-flight
 * requests at once — a dashboard fires half a dozen queries on mount — and
 * each would otherwise announce the same sign-out. The first wins; the rest
 * are the same event arriving again.
 *
 * Reset by `resetSessionExpiry` when a session is established, so signing back
 * in re-arms it.
 */
let alreadyExpired = false;

/**
 * Subscribe to session refusal. Returns an unsubscribe function.
 *
 * Handlers must be idempotent regardless, because `BroadcastChannel` and the
 * `storage` event can both carry one cross-tab sign-out.
 */
export function onSessionExpired(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/**
 * Called by the transport when a request comes back 401.
 *
 * Deliberately does no navigation and clears no state itself: this module
 * knows a fact, and what to do about it belongs to whoever owns the session.
 * A transport that redirected would be a transport with an opinion about
 * routing, and would fire during tests and background refreshes alike.
 */
export function reportSessionRefused(): void {
  if (alreadyExpired) return;
  alreadyExpired = true;
  for (const listener of listeners) {
    try {
      listener();
    } catch {
      // One bad listener must not stop the others being told that the
      // session is gone.
    }
  }
}

/** Re-arm after a session is established. */
export function resetSessionExpiry(): void {
  alreadyExpired = false;
}

/** For tests: whether a refusal has been reported since the last reset. */
export function sessionRefusedAlready(): boolean {
  return alreadyExpired;
}
