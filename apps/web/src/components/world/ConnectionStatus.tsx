/**
 * What the connection is doing, in the idiom players already expect from an
 * online game (spec 028 US7, T097, FR-063, SC-022).
 *
 * # What this has to say, and what it must not
 *
 * A player who has lost the server needs three things, and only three. Which
 * state they are in. That something is being done about it. And — the one
 * that decides whether they carry on playing — what is happening to the work
 * they do in the meantime. An indicator that only announced the failure would
 * leave someone guessing whether to stop moving tokens, which is exactly the
 * opposite of the point: their edits are being kept, and they may continue.
 *
 * It must never block. Every state here renders as a small, non-blocking
 * badge over an already-loaded scene, and none of them is a dead end
 * requiring manual action: the transport retries indefinitely underneath
 * (FR-009a), so there is no "reconnect" button to offer and nothing that
 * could leave a user stranded with an indicator they cannot dismiss.
 *
 * # Why the message appears before the outage is confirmed
 *
 * SC-022 wants the user told which state they are in within five seconds, and
 * a dropped socket is not yet an outage — the first retry usually succeeds in
 * about a second. So `reconnecting` shows immediately and quietly, carrying
 * the attempt count so the retries are visible, and only escalates to the
 * disconnected wording once retrying has stopped looking temporary. The user
 * is told something true at every moment, and the loud version is reserved
 * for when it is warranted.
 *
 * # The third state
 *
 * `server-isolated` is the one players will not have seen elsewhere: the
 * server is gone and the whole table is still here, so play continues with
 * the Game Master's client adjudicating. What it says is chosen carefully.
 * Play *is* continuing, so the badge is not an alarm — but nothing agreed
 * here is final, and saying so is the difference between a group that
 * understands a later correction and a group that experiences it as work
 * being deleted (FR-062).
 */

import type { LiveSyncState } from "@/engine/world/sync";

export interface ConnectionStatusProps {
  state: LiveSyncState;
}

/** Background colour per state. Quiet while it might be nothing. */
function toneFor(state: LiveSyncState): string {
  switch (state.status) {
    case "server-isolated":
      // Not an alarm: the table is playing. Distinct from both the healthy
      // case and the plain outage, because it is neither.
      return "rgba(30, 64, 120, 0.85)";
    case "disconnected":
      return "rgba(90, 24, 24, 0.85)";
    default:
      return "rgba(0, 0, 0, 0.75)";
  }
}

/** The line the user reads. */
function messageFor(state: LiveSyncState): string {
  switch (state.status) {
    case "connecting":
      return "Connecting…";
    case "reconnecting":
      return `Reconnecting… (attempt ${state.attempt})`;
    case "server-isolated":
      return state.peers === 1
        ? "Playing with 1 other person directly — changes are provisional until the server is back"
        : `Playing with ${state.peers} others directly — changes are provisional until the server is back`;
    case "disconnected":
      // What matters here is not "something is wrong" — the user can see that
      // — but that their work is being kept and that they may carry on.
      return state.browserOffline
        ? "Offline — your changes are saved here and will sync when you reconnect"
        : "Can't reach the server — your changes are saved here and will sync when it's back";
    default:
      return "";
  }
}

/**
 * The second line: what is being done about it.
 *
 * Present in every state that is not healthy, because "we are still trying"
 * is the half of SC-022 that an attempt count alone does not convey — a
 * number that stops going up looks the same as a number nobody is updating.
 */
function effortFor(state: LiveSyncState): string | null {
  switch (state.status) {
    case "connecting":
      return null;
    case "reconnecting":
      return "Still trying";
    case "disconnected":
      return state.attempt > 0
        ? `Still trying — attempt ${state.attempt}`
        : "Still trying";
    case "server-isolated":
      return state.attempt > 0
        ? `Still looking for the server — attempt ${state.attempt}`
        : "Still looking for the server";
    default:
      return null;
  }
}

export function ConnectionStatus({ state }: ConnectionStatusProps) {
  // A healthy connection says nothing at all. An indicator that is always
  // there is one nobody reads when it changes.
  if (state.status === "live") return null;

  const effort = effortFor(state);

  return (
    <div
      // The test id predates the third state and the extraction of this
      // component, and is kept exactly: it is what the offline end-to-end
      // suite looks for, and `data-sync-status` is how that suite tells the
      // states apart.
      data-testid="live-sync-reconnecting-indicator"
      data-sync-status={state.status}
      role="status"
      aria-live="polite"
      style={{
        position: "absolute",
        top: "1rem",
        // Clear of the dock's icon rail (3rem) on the right, and left of the
        // peer disclosure indicator that sits in the corner.
        right: "4rem",
        zIndex: 1000,
        maxWidth: "22rem",
        padding: "0.4rem 0.75rem",
        borderRadius: "0.375rem",
        background: toneFor(state),
        color: "white",
        fontSize: "0.8rem",
      }}
    >
      <div>{messageFor(state)}</div>
      {effort ? (
        <div
          data-testid="live-sync-retry-effort"
          style={{ opacity: 0.75, fontSize: "0.7rem", marginTop: "0.15rem" }}
        >
          {effort}
        </div>
      ) : null}
    </div>
  );
}
